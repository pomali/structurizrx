mod clusters;
mod diff;
mod edit;
mod graph;
mod lint;
mod locate;
mod mcp;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use structurizr_dsl::{parse_file, ParseError};
use structurizr_model::{validation, ViewSet, Workspace};
use structurizr_renderer::{
    dot::DotExporter, exporter::DiagramExporter, mermaid::MermaidExporter,
    plantuml::PlantUmlExporter, svg::SvgExporter,
};

#[derive(Parser)]
#[command(name = "structurizrx", version, about = "Structurizr DSL toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and validate a .dsl or .json workspace file
    Validate {
        file: PathBuf,
        /// Also fail on lint findings (placeholders, uncertain items,
        /// orphans, unbound ports)
        #[arg(long)]
        strict: bool,
        /// Emit machine-readable JSON (parse/validation errors and lint
        /// findings with stable codes) instead of text
        #[arg(long)]
        json: bool,
    },
    /// Render diagrams from a workspace file
    Render {
        file: PathBuf,
        /// Output format: svg, png, mermaid, plantuml, or dot
        #[arg(
            long,
            default_value = "plantuml",
            value_name = "svg|png|mermaid|plantuml|dot"
        )]
        format: String,
        #[arg(long, short, default_value = ".")]
        output: PathBuf,
    },
    /// Export workspace to JSON
    Export {
        file: PathBuf,
        #[arg(long, short, default_value = "workspace.json")]
        output: PathBuf,
    },
    /// Export a portable static HTML website for a workspace
    ExportSite {
        file: PathBuf,
        /// Directory to write the website artifact into
        #[arg(long, short, default_value = "site")]
        output: PathBuf,
    },
    /// Export a portable interactive workspace viewer with bundled assets
    ExportViewer {
        file: PathBuf,
        /// Directory to write the viewer artifact into
        #[arg(long, short, default_value = "viewer")]
        output: PathBuf,
    },
    /// Print a compact plain-text summary of the model, sized for LLM context
    Digest { file: PathBuf },
    /// Run a selector expression against a workspace (spec §6.2),
    /// e.g. `query ws.dsl "element.tag==Database"` or `query ws.dsl "->api->2"`
    Query {
        file: PathBuf,
        /// Selector expression, e.g. `element.status==idea && element.layer==domain`
        #[arg(allow_hyphen_values = true)]
        expression: String,
        /// Emit machine-readable JSON instead of a text listing
        #[arg(long)]
        json: bool,
    },
    /// Find where elements, ports, relationships, views and decisions are
    /// declared, by name path or from a viewer link carrying a selection,
    /// e.g. `locate ws.dsl "Shop/API" "Shop/API->Shop/DB"`
    Locate {
        file: PathBuf,
        /// Name paths (`Shop/API`, `Shop/API.http`, `Shop/API->Shop/DB "reads"`,
        /// `view:<key>`, `decision:<id>`) or viewer links (`http://…#view&sel=…`)
        #[arg(required = true)]
        references: Vec<String>,
        /// Emit machine-readable JSON instead of a text listing
        #[arg(long)]
        json: bool,
    },
    /// Compare two versions of a workspace as models — elements, relationships,
    /// views and decisions added, removed or changed, keyed by name path rather
    /// than id. Defaults to the last commit vs the file on disk
    Diff {
        file: PathBuf,
        /// Git revision to compare from (default: HEAD)
        #[arg(long)]
        from: Option<String>,
        /// Git revision to compare to (default: the file on disk)
        #[arg(long)]
        to: Option<String>,
        /// Compare against another workspace file instead of a git revision
        #[arg(long, value_name = "FILE")]
        against: Option<PathBuf>,
        /// Emit machine-readable JSON instead of a text report
        #[arg(long)]
        json: bool,
        /// Exit with status 1 when anything changed (for CI gates)
        #[arg(long)]
        fail_on_change: bool,
    },
    /// Lint the model: blocking findings (placeholders, uncertain items, orphans,
    /// unbound ports), hygiene warnings (missing description or technology,
    /// elements in no view, duplicate names, undescribed relationships) and
    /// dependency cycles. Exits 1 on blocking findings
    Lint {
        file: PathBuf,
        /// Also exit 1 on warnings and cycles, not just blocking findings
        #[arg(long)]
        strict: bool,
        /// Emit machine-readable JSON instead of a text listing
        #[arg(long)]
        json: bool,
    },
    /// Cluster analysis at one level: detected communities and how they agree
    /// with the declared structure, cycles, bridges, articulation points and
    /// coupling metrics — the same analysis as the viewer's clusters page
    Clusters {
        file: PathBuf,
        /// Level to project the model onto
        #[arg(
            long,
            default_value = "container",
            value_name = "system|container|component"
        )]
        level: String,
        /// Keep only elements carrying one of these tags (repeatable)
        #[arg(long)]
        include: Vec<String>,
        /// Drop elements carrying one of these tags (repeatable)
        #[arg(long)]
        exclude: Vec<String>,
        /// Do not roll descendant relationships up onto their ancestors
        #[arg(long)]
        no_implied: bool,
        /// Emit machine-readable JSON instead of a text report
        #[arg(long)]
        json: bool,
    },
    /// The whole workspace as one graph — every element, view and decision a
    /// node; every relationship, containment, instance and membership a link
    Graph {
        file: PathBuf,
        /// Output format: json (the viewer's universe-graph feed) or dot
        #[arg(long, default_value = "json", value_name = "json|dot")]
        format: String,
    },
    /// Add a statement to the DSL source — an element, relationship, view or
    /// body line — inside the block named by `--in` (an element identifier or
    /// name path, or `model` / `views`; default `model`). Comments and
    /// formatting are kept, and nothing is written unless the result parses
    Add {
        file: PathBuf,
        /// The statement, e.g. `cache = container "Cache" "Redis"` or `web -> api "calls"`
        #[arg(allow_hyphen_values = true)]
        statement: String,
        /// Block to add into: `model`, `views`, or an element reference such as `shop` or `Shop/API`
        #[arg(long = "in", value_name = "REF", default_value = "model")]
        parent: String,
        /// Emit machine-readable JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// Remove the statement declaring an element, relationship or port from
    /// the DSL source. Writes nothing if something still refers to it;
    /// `--cascade` also removes the relationships that do
    Remove {
        file: PathBuf,
        /// Identifier or name path (`api`, `Shop/API`, `Shop/Web App->Shop/API`)
        #[arg(allow_hyphen_values = true)]
        reference: String,
        /// Also remove every relationship touching the element or anything inside it
        #[arg(long)]
        cascade: bool,
        /// Emit machine-readable JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// Rename a DSL identifier everywhere it is used, across `!include`d
    /// files (element names in quotes are untouched)
    Rename {
        file: PathBuf,
        identifier: String,
        new_identifier: String,
        /// Emit machine-readable JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// Print the workspace as canonical DSL: stable ordering and layout, a
    /// `.json` workspace converted to DSL, a sketch promoted to a full
    /// `workspace { model { … } views { … } }`. Comments are not kept, so
    /// `--write` refuses to overwrite a file that has any unless `--force`
    Fmt {
        file: PathBuf,
        /// Write the result to this file instead of stdout
        #[arg(long, short, conflicts_with_all = ["write", "check"])]
        output: Option<PathBuf>,
        /// Rewrite the file in place
        #[arg(long, conflicts_with = "check")]
        write: bool,
        /// Exit 1 if the file is not already in canonical form (for CI)
        #[arg(long)]
        check: bool,
        /// With --write, overwrite even when comments, includes or imports would be lost
        #[arg(long, requires = "write")]
        force: bool,
    },
    /// Serve the read, check and edit commands (validate, digest, query,
    /// locate, lint, diff, render, docs, add, remove, rename, format) as MCP
    /// tools over stdio, for agent hosts such as Claude Desktop, Cursor or
    /// Codex
    Mcp,
    /// Print the DSL extension cheat sheet (llms.txt) — the format reference
    /// for LLM agents and humans authoring workspaces
    Docs,
    /// Serve a workspace or directory of workspaces in a local web browser
    Serve {
        /// Path to a .dsl/.json file or a directory containing workspace(s).
        /// Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// TCP port to listen on.
        #[arg(long, short, default_value_t = 3000)]
        port: u16,
        /// Open the browser automatically after starting the server.
        #[arg(long)]
        open: bool,
    },
    /// Run the DSL language server over stdio (for editor integration, e.g.
    /// the VS Code extension in editors/vscode)
    Lsp,
}

/// Why a workspace file could not be loaded: a DSL parse failure keeps its
/// structured diagnostics; anything else (unreadable file, bad JSON) is opaque.
enum LoadError {
    Parse(ParseError),
    Other(anyhow::Error),
}

fn load_workspace_detailed(path: &Path) -> std::result::Result<Workspace, LoadError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "json" {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))
            .map_err(LoadError::Other)?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from {}", path.display()))
            .map_err(LoadError::Other)
    } else {
        parse_file(path).map_err(LoadError::Parse)
    }
}

/// A parse error as `validate --json` reports it: stable code, the file it
/// is in (an `!include`d part resolved onto the entry file's directory),
/// line, column and message.
fn parse_error_json(err: &ParseError, entry: &Path) -> Vec<serde_json::Value> {
    err.diagnostics()
        .iter()
        .map(|d| {
            let file = ParseError::resolve_file(d, entry);
            serde_json::json!({
                "code": d.code,
                "file": file.display().to_string(),
                "line": d.line,
                "column": d.column,
                "message": d.message,
            })
        })
        .collect()
}

fn load_workspace(path: &PathBuf) -> Result<Workspace> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "json" {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let ws: Workspace = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from {}", path.display()))?;
        Ok(ws)
    } else {
        parse_file(path).with_context(|| format!("Failed to parse DSL from {}", path.display()))
    }
}

/// View-type name paired with how many views of that type are defined.
fn view_type_counts(views: &ViewSet) -> Vec<(&'static str, usize)> {
    vec![
        (
            "systemLandscape",
            views.system_landscape_views.as_ref().map_or(0, Vec::len),
        ),
        (
            "systemContext",
            views.system_context_views.as_ref().map_or(0, Vec::len),
        ),
        (
            "container",
            views.container_views.as_ref().map_or(0, Vec::len),
        ),
        (
            "component",
            views.component_views.as_ref().map_or(0, Vec::len),
        ),
        ("dynamic", views.dynamic_views.as_ref().map_or(0, Vec::len)),
        (
            "deployment",
            views.deployment_views.as_ref().map_or(0, Vec::len),
        ),
        (
            "filtered",
            views.filtered_views.as_ref().map_or(0, Vec::len),
        ),
        ("image", views.image_views.as_ref().map_or(0, Vec::len)),
        ("custom", views.custom_views.as_ref().map_or(0, Vec::len)),
    ]
}

/// View types each exporter's `export_workspace` actually renders. Kept in
/// sync manually with the `if let Some(..) = views.*` cases in each exporter;
/// anything not listed here is silently dropped by that exporter today.
fn handled_view_types(format: &str) -> &'static [&'static str] {
    match format.to_lowercase().as_str() {
        "svg" | "png" => &["systemLandscape", "systemContext", "container", "component"],
        "mermaid" => &["systemLandscape", "systemContext", "container", "component"],
        "dot" | "graphviz" => &["systemLandscape", "systemContext"],
        _ => &["systemLandscape", "systemContext", "container"], // plantuml
    }
}

/// Warn about views defined in the workspace that the chosen exporter has no
/// support for, so they don't just vanish without explanation.
fn warn_on_unsupported_views(views: &ViewSet, format: &str) {
    let handled = handled_view_types(format);
    let skipped: Vec<(&str, usize)> = view_type_counts(views)
        .into_iter()
        .filter(|(name, count)| *count > 0 && !handled.contains(name))
        .collect();
    if skipped.is_empty() {
        return;
    }
    let total: usize = skipped.iter().map(|(_, count)| count).sum();
    let breakdown: Vec<String> = skipped
        .iter()
        .map(|(name, count)| format!("{} {}", count, name))
        .collect();
    eprintln!(
        "Warning: {} view(s) skipped ({} exporter does not support: {})",
        total,
        format,
        breakdown.join(", ")
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { file, strict, json } => {
            let workspace = match load_workspace_detailed(&file) {
                Ok(ws) => ws,
                Err(LoadError::Parse(e)) => {
                    if json {
                        let out = serde_json::json!({
                            "valid": false,
                            "errors": parse_error_json(&e, &file),
                            "lint": [],
                        });
                        println!("{}", serde_json::to_string_pretty(&out)?);
                    } else {
                        // One line per error, each with its file, so an
                        // agent (or a person) fixes every mistake in one
                        // pass instead of one per run.
                        for d in e.diagnostics() {
                            let path = ParseError::resolve_file(&d, &file);
                            if d.line > 0 {
                                eprintln!(
                                    "{}:{}:{}: {}",
                                    path.display(),
                                    d.line,
                                    d.column,
                                    d.message
                                );
                            } else {
                                eprintln!("{}: {}", path.display(), d.message);
                            }
                        }
                        let n = e.errors().len();
                        eprintln!(
                            "✗ {} parse error{} in {}",
                            n,
                            if n == 1 { "" } else { "s" },
                            file.display()
                        );
                    }
                    std::process::exit(1);
                }
                Err(LoadError::Other(e)) if json => {
                    let out = serde_json::json!({
                        "valid": false,
                        "errors": [{ "code": "load", "message": format!("{:#}", e) }],
                        "lint": [],
                    });
                    println!("{}", serde_json::to_string_pretty(&out)?);
                    std::process::exit(1);
                }
                Err(LoadError::Other(e)) => return Err(e),
            };
            let errors = validation::validate(&workspace);
            let findings = structurizr_query::lint(&workspace);
            let failed = !errors.is_empty() || (strict && !findings.is_empty());

            if json {
                let out = serde_json::json!({
                    "valid": errors.is_empty(),
                    "errors": errors.iter().map(|e| serde_json::json!({
                        "code": e.code(),
                        "message": e.to_string(),
                    })).collect::<Vec<_>>(),
                    "lint": findings.iter().map(|f| serde_json::json!({
                        "code": f.code,
                        "elementId": f.element_id,
                        "name": f.name,
                        "message": f.message,
                    })).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                if !errors.is_empty() {
                    eprintln!("Validation errors:");
                    for e in &errors {
                        eprintln!("  - [{}] {}", e.code(), e);
                    }
                }
                if strict && !findings.is_empty() {
                    eprintln!("Lint findings:");
                    for f in &findings {
                        eprintln!("  - [{}] {} (element {})", f.code, f.message, f.element_id);
                    }
                }
                if !failed {
                    println!("✓ Workspace '{}' is valid", workspace.name);
                }
            }
            if failed {
                std::process::exit(1);
            }
        }
        Commands::Render {
            file,
            format,
            output,
        } => {
            let mut workspace = load_workspace(&file)?;
            let generated = structurizr_query::generate_views(&mut workspace)
                .map_err(|e| anyhow::anyhow!("view generation: {}", e))?;
            if !generated.is_empty() {
                println!("Generated views: {}", generated.join(", "));
            }
            warn_on_unsupported_views(&workspace.views, &format);
            std::fs::create_dir_all(&output)
                .with_context(|| format!("Cannot create output dir {}", output.display()))?;

            let format = format.to_lowercase();
            let diagrams: Vec<_> = match format.as_str() {
                "mermaid" => MermaidExporter.export_workspace(&workspace),
                "dot" | "graphviz" => DotExporter.export_workspace(&workspace),
                "svg" | "png" => SvgExporter.export_workspace(&workspace),
                _ => PlantUmlExporter.export_workspace(&workspace),
            };

            if diagrams.is_empty() {
                println!("No diagrams to render.");
            } else {
                for d in &diagrams {
                    let filename = if format == "png" {
                        output.join(format!("{}.png", d.key))
                    } else {
                        output.join(format!("{}.{}", d.key, d.extension()))
                    };
                    if format == "png" {
                        let bytes = structurizr_renderer::png::svg_to_png(&d.content)
                            .map_err(|e| anyhow::anyhow!("rasterizing {}: {}", d.key, e))?;
                        std::fs::write(&filename, bytes)
                            .with_context(|| format!("Cannot write {}", filename.display()))?;
                    } else {
                        std::fs::write(&filename, &d.content)
                            .with_context(|| format!("Cannot write {}", filename.display()))?;
                    }
                    println!("Written: {}", filename.display());
                }
            }
        }
        Commands::Export { file, output } => {
            let workspace = load_workspace(&file)?;
            let json = serde_json::to_string_pretty(&workspace)
                .context("Failed to serialize workspace to JSON")?;
            std::fs::write(&output, &json)
                .with_context(|| format!("Cannot write {}", output.display()))?;
            println!("Exported workspace to {}", output.display());
        }
        Commands::ExportSite { file, output } => {
            let mut workspace = load_workspace(&file)?;
            let generated = structurizr_query::generate_views(&mut workspace)
                .map_err(|e| anyhow::anyhow!("view generation: {}", e))?;
            if !generated.is_empty() {
                println!("Generated views: {}", generated.join(", "));
            }
            structurizr_web::static_site::export(&workspace, &output)?;
            println!("Exported static report to {}", output.display());
        }
        Commands::ExportViewer { file, output } => {
            let mut workspace = load_workspace(&file)?;
            let generated = structurizr_query::generate_views(&mut workspace)
                .map_err(|e| anyhow::anyhow!("view generation: {}", e))?;
            if !generated.is_empty() {
                println!("Generated views: {}", generated.join(", "));
            }
            structurizr_web::static_site::export_viewer(&workspace, &output)?;
            println!("Exported static viewer to {}", output.display());
        }
        Commands::Digest { file } => {
            let mut workspace = load_workspace(&file)?;
            // Materialize generated (`auto`) views so the digest lists the
            // effective view set, matching what render/serve produce.
            if let Err(e) = structurizr_query::generate_views(&mut workspace) {
                eprintln!("warning: view generation failed: {}", e);
            }
            print!("{}", structurizr_query::digest(&workspace));
        }
        Commands::Query {
            file,
            expression,
            json,
        } => {
            let workspace = load_workspace(&file)?;
            let selection = structurizr_query::query(&expression, &workspace)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let names = structurizr_query::element_names(&workspace);
            let paths = structurizr_query::element_paths(&workspace);
            let rels = structurizr_query::relationship_summaries(&workspace);
            if json {
                let out = serde_json::json!({
                    "elements": selection.elements.iter().map(|id| serde_json::json!({
                        "id": id,
                        "name": names.get(id),
                        "path": paths.get(id),
                    })).collect::<Vec<_>>(),
                    "relationships": selection.relationships.iter().map(|id| match rels.get(id) {
                        Some(r) => serde_json::to_value(r).unwrap_or_default(),
                        None => serde_json::json!({ "id": id }),
                    }).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                for id in &selection.elements {
                    match (names.get(id), paths.get(id)) {
                        (Some(name), Some(path)) => println!("element  {}  {}  {}", id, name, path),
                        (Some(name), None) => println!("element  {}  {}", id, name),
                        _ => println!("element  {}", id),
                    }
                }
                for id in &selection.relationships {
                    match rels.get(id) {
                        Some(r) => println!("relationship  {}  {}", id, r.line()),
                        None => println!("relationship  {}", id),
                    }
                }
                if selection.elements.is_empty() && selection.relationships.is_empty() {
                    eprintln!("(no matches)");
                }
            }
        }
        Commands::Locate {
            file,
            references,
            json,
        } => {
            if !locate::run(&file, &references, json)? {
                std::process::exit(1);
            }
        }
        Commands::Diff {
            file,
            from,
            to,
            against,
            json,
            fail_on_change,
        } => {
            let changed = diff::run(&diff::Args {
                file,
                from,
                to,
                against,
                json,
            })?;
            if changed && fail_on_change {
                std::process::exit(1);
            }
        }
        Commands::Lint { file, strict, json } => {
            if !lint::run(&file, strict, json)? {
                std::process::exit(1);
            }
        }
        Commands::Clusters {
            file,
            level,
            include,
            exclude,
            no_implied,
            json,
        } => {
            clusters::run(&clusters::Args {
                file,
                level,
                include,
                exclude,
                implied: !no_implied,
                json,
            })?;
        }
        Commands::Graph { file, format } => {
            graph::run(&file, &format)?;
        }
        Commands::Add {
            file,
            statement,
            parent,
            json,
        } => {
            edit::add(&file, &statement, &parent)?.print(json);
        }
        Commands::Remove {
            file,
            reference,
            cascade,
            json,
        } => {
            edit::remove(&file, &reference, cascade)?.print(json);
        }
        Commands::Rename {
            file,
            identifier,
            new_identifier,
            json,
        } => {
            edit::rename(&file, &identifier, &new_identifier)?.print(json);
        }
        Commands::Fmt {
            file,
            output,
            write,
            check,
            force,
        } => {
            let (text, losses) = edit::format(&file)?;
            if check {
                let current = std::fs::read_to_string(&file)
                    .with_context(|| format!("Failed to read {}", file.display()))?;
                if current != text {
                    eprintln!(
                        "{} is not in canonical form (run `structurizrx fmt --write`)",
                        file.display()
                    );
                    std::process::exit(1);
                }
            } else if write {
                let is_json = file
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"));
                if is_json {
                    anyhow::bail!(
                        "--write would put DSL into a .json file; use --output ws.dsl instead"
                    );
                }
                if !losses.is_empty() && !force {
                    anyhow::bail!(
                        "not rewriting {} because {}.\nUse --output to write elsewhere, or --force to accept the loss.",
                        file.display(),
                        losses.join("; ")
                    );
                }
                std::fs::write(&file, &text)
                    .with_context(|| format!("Cannot write {}", file.display()))?;
                println!("formatted {}", file.display());
            } else if let Some(out) = output {
                std::fs::write(&out, &text)
                    .with_context(|| format!("Cannot write {}", out.display()))?;
                for l in &losses {
                    eprintln!("note: {}", l);
                }
                println!("wrote {}", out.display());
            } else {
                print!("{}", text);
            }
        }
        Commands::Mcp => {
            mcp::run()?;
        }
        Commands::Docs => {
            print!("{}", include_str!("../../../llms.txt"));
        }
        Commands::Serve { path, port, open } => {
            structurizr_web::serve(structurizr_web::ServeOptions {
                path,
                port,
                open_browser: open,
            })
            .await?;
        }
        Commands::Lsp => {
            structurizr_lsp::run_stdio().await?;
        }
    }

    Ok(())
}
