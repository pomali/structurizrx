//! `structurizrx diff`: compare two versions of a workspace as models.
//! Implemented on `structurizr_query::diff` (the model comparison) and
//! `structurizr_web::git` (reading a revision with its `!include`s spliced
//! from the same revision).

use std::path::PathBuf;

use anyhow::{Context, Result};
use structurizr_model::Workspace;
use structurizr_query::diff::{Change, Diff};

pub struct Args {
    pub file: PathBuf,
    pub from: Option<String>,
    pub to: Option<String>,
    pub against: Option<PathBuf>,
    pub json: bool,
}

/// Load a workspace file directly (not from git), materializing generated
/// views the same way a revision read from git is — so a comparison never
/// reads a generated view as added or removed purely because one side went
/// through `generate_views` and the other did not.
fn load_and_materialize(path: &std::path::Path) -> Result<Workspace> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    structurizr_web::resolver::workspace_from_source(
        &content,
        structurizr_web::resolver::is_json(path),
        &path.display().to_string(),
    )
}

/// Print the comparison. Returns whether anything changed.
pub fn run(args: &Args) -> Result<bool> {
    let (before, after, from_label, to_label) = if let Some(against) = &args.against {
        if args.from.is_some() || args.to.is_some() {
            anyhow::bail!("--against cannot be combined with --from/--to");
        }
        let before = load_and_materialize(&args.file)?;
        let after = load_and_materialize(against)?;
        (
            before,
            after,
            args.file.display().to_string(),
            against.display().to_string(),
        )
    } else {
        if !structurizr_web::git::is_tracked(&args.file) {
            anyhow::bail!(
                "{} is not tracked by git; pass --against <file> to compare against another workspace file instead",
                args.file.display()
            );
        }
        let from = args.from.clone().unwrap_or_else(|| "HEAD".to_string());
        let to = args
            .to
            .clone()
            .unwrap_or_else(|| structurizr_web::git::WORKING.to_string());

        let load = |rev: &str| -> Result<Workspace> {
            if rev == structurizr_web::git::WORKING {
                load_and_materialize(&args.file)
            } else {
                let source = structurizr_web::git::read(&args.file, rev)
                    .with_context(|| format!("Failed to read revision {rev}"))?;
                structurizr_web::resolver::workspace_from_source(
                    &source,
                    structurizr_web::resolver::is_json(&args.file),
                    &format!("revision {rev}"),
                )
            }
        };

        let before = load(&from)?;
        let after = load(&to)?;
        (before, after, from, to)
    };

    let d = structurizr_query::diff(&before, &after);
    let changed = !d.summary.is_empty();

    if args.json {
        let out = serde_json::json!({
            "from": from_label,
            "to": to_label,
            "diff": d,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_text(&d, &from_label, &to_label);
    }

    Ok(changed)
}

fn print_text(d: &Diff, from: &str, to: &str) {
    if d.summary.is_empty() {
        println!("no changes ({from} -> {to})");
        return;
    }

    println!("{from} -> {to}");
    println!(
        "elements {}, relationships {}, views {}, decisions {}",
        counts_str(&d.summary.elements),
        counts_str(&d.summary.relationships),
        counts_str(&d.summary.views),
        counts_str(&d.summary.decisions),
    );
    // (each category reads e.g. "elements +2 −1 ~3" or "decisions =")

    if !d.workspace.is_empty() {
        println!();
        println!("Workspace:");
        for f in &d.workspace {
            println!("  ~ {}: {}", f.field, field_change_str(f));
        }
    }

    if !d.elements.is_empty() {
        println!();
        println!("Elements:");
        for c in &d.elements {
            match c.change {
                Change::Added => println!("  + {} ({})", c.path, c.kind),
                Change::Removed => println!("  − {} ({})", c.path, c.kind),
                Change::Modified => {
                    for f in &c.fields {
                        println!(
                            "  ~ {} ({}): {} {}",
                            c.path,
                            c.kind,
                            f.field,
                            field_change_str(f)
                        );
                    }
                }
            }
        }
    }

    if !d.relationships.is_empty() {
        println!();
        println!("Relationships:");
        for c in &d.relationships {
            let desc = c.description.as_deref().unwrap_or("");
            match c.change {
                Change::Added => println!("  + {} -> {} \"{}\"", c.source, c.destination, desc),
                Change::Removed => println!("  − {} -> {} \"{}\"", c.source, c.destination, desc),
                Change::Modified => {
                    for f in &c.fields {
                        println!(
                            "  ~ {} -> {} \"{}\": {} {}",
                            c.source,
                            c.destination,
                            desc,
                            f.field,
                            field_change_str(f)
                        );
                    }
                }
            }
        }
    }

    if !d.views.is_empty() {
        println!();
        println!("Views:");
        for c in &d.views {
            match c.change {
                Change::Added => println!("  + {} ({})", c.key, c.kind),
                Change::Removed => println!("  − {} ({})", c.key, c.kind),
                Change::Modified => {
                    for f in &c.fields {
                        println!(
                            "  ~ {} ({}): {} {}",
                            c.key,
                            c.kind,
                            f.field,
                            field_change_str(f)
                        );
                    }
                    if !c.elements_added.is_empty() {
                        println!(
                            "  ~ {} ({}): +{}",
                            c.key,
                            c.kind,
                            c.elements_added.join(", +")
                        );
                    }
                    if !c.elements_removed.is_empty() {
                        println!(
                            "  ~ {} ({}): -{}",
                            c.key,
                            c.kind,
                            c.elements_removed.join(", -")
                        );
                    }
                }
            }
        }
    }

    if !d.decisions.is_empty() {
        println!();
        println!("Decisions:");
        for c in &d.decisions {
            match c.change {
                Change::Added => println!("  + {} {}", c.id, c.title),
                Change::Removed => println!("  − {} {}", c.id, c.title),
                Change::Modified => {
                    for f in &c.fields {
                        println!(
                            "  ~ {} {}: {} {}",
                            c.id,
                            c.title,
                            f.field,
                            field_change_str(f)
                        );
                    }
                }
            }
        }
    }

    if !d.renames.is_empty() {
        println!();
        println!("Likely renames:");
        for r in &d.renames {
            println!("  {} -> {} ({})", r.from, r.to, r.reason);
        }
    }
}

fn counts_str(c: &structurizr_query::diff::CategoryCounts) -> String {
    if c.is_empty() {
        return "=".to_string();
    }
    let mut parts = Vec::new();
    if c.added > 0 {
        parts.push(format!("+{}", c.added));
    }
    if c.removed > 0 {
        parts.push(format!("−{}", c.removed));
    }
    if c.modified > 0 {
        parts.push(format!("~{}", c.modified));
    }
    parts.join(" ")
}

fn field_change_str(f: &structurizr_query::diff::FieldChange) -> String {
    format!(
        "\"{}\" -> \"{}\"",
        f.before.as_deref().unwrap_or(""),
        f.after.as_deref().unwrap_or("")
    )
}
