//! `structurizrx locate`: from a name path, or a viewer link carrying a
//! selection, to the DSL statements that declare it. Resolution and the JSON
//! shape are shared with the server's `/api/workspace/{name}/locate`
//! ([`structurizr_web::locate`]); this adds loading, link expansion and the
//! text listing.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use structurizr_query::reference::{parse_viewer_link, Catalog};
use structurizr_web::locate::{self, SourceLines};

/// Run the command; `Ok(false)` when any reference did not resolve.
pub fn run(file: &Path, inputs: &[String], json_output: bool) -> Result<bool> {
    let is_json = file
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    let (mut workspace, locations) = if is_json {
        (crate::load_workspace(&file.to_path_buf())?, None)
    } else {
        let parsed = structurizr_dsl::parse_file_detailed(file)
            .with_context(|| format!("Failed to parse DSL from {}", file.display()))?;
        (parsed.workspace, Some(parsed.locations))
    };
    // The viewer addresses generated views by key too: materialize them so
    // those keys resolve, and remember they have no statement of their own.
    let generated: HashSet<String> = structurizr_query::generate_views(&mut workspace)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let catalog = Catalog::new(&workspace);

    let references = expand_inputs(file, inputs);
    let located = locate::locate(&catalog, locations.as_ref(), &generated, &references);

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&locate::to_json(&located))?
        );
    } else {
        let mut sources = SourceLines::default();
        for item in &located.items {
            println!("{}  {}", item.target.kind(), locate::label(&item.target));
            for place in &item.places {
                if let Some(note) = &place.note {
                    println!("  {note}");
                }
                let loc = &place.location;
                let through = if loc.end_line > loc.line {
                    format!(" (to line {})", loc.end_line)
                } else {
                    String::new()
                };
                println!(
                    "  {}:{}:{}{}",
                    locate::display_file(loc),
                    loc.line,
                    loc.col,
                    through
                );
                if let Some(text) = loc.file.as_deref().and_then(|f| sources.line(f, loc.line)) {
                    println!("    {text}");
                }
            }
            if let Some(note) = &item.note {
                println!("  ({note})");
            }
        }
        for (reference, miss) in &located.misses {
            match locate::suggestion(miss) {
                Some(s) => eprintln!(
                    "error: {reference}: {} (did you mean '{s}'?)",
                    locate::miss_message(miss)
                ),
                None => eprintln!("error: {reference}: {}", locate::miss_message(miss)),
            }
        }
    }
    Ok(located.misses.is_empty() && !references.is_empty())
}

/// Replace each viewer link with the references it carries: its view, then
/// its selection.
fn expand_inputs(file: &Path, inputs: &[String]) -> Vec<String> {
    let served_as = structurizr_web::resolver::slug_from_path(file);
    let mut out = Vec::new();
    for input in inputs {
        let Some(link) = parse_viewer_link(input) else {
            out.push(input.clone());
            continue;
        };
        if let Some(slug) = link.workspace.as_deref().filter(|s| *s != served_as) {
            eprintln!(
                "warning: {input} links to workspace '{slug}', but {} is served as '{served_as}'",
                file.display()
            );
        }
        if link.view.is_none() && link.selection.is_empty() {
            eprintln!("warning: {input} names no view and carries no selection");
        }
        out.extend(link.view.map(|key| format!("view:{key}")));
        out.extend(link.selection);
    }
    out
}
