//! `structurizrx graph`: the universe graph (`structurizr_query::graph`) as
//! JSON or DOT.

use std::path::Path;

use anyhow::Result;
use structurizr_query::graph::{Graph, GraphLink, GraphNode};

pub fn run(file: &Path, format: &str) -> Result<()> {
    let mut workspace = crate::load_workspace(&file.to_path_buf())?;
    if let Err(e) = structurizr_query::generate_views(&mut workspace) {
        eprintln!("warning: view generation failed: {}", e);
    }
    let g = structurizr_query::graph(&workspace);

    match format.to_lowercase().as_str() {
        "dot" | "graphviz" => print!("{}", to_dot(&g)),
        _ => println!("{}", serde_json::to_string_pretty(&g)?),
    }

    Ok(())
}

fn shape_for(kind: &str) -> &'static str {
    match kind {
        "person" => "ellipse",
        "view" => "note",
        "decision" => "tab",
        _ => "box",
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn edge_style(class: &str) -> &'static str {
    match class {
        "containment" => "dashed",
        "membership" | "instance" | "documents" => "dotted",
        _ => "solid",
    }
}

fn to_dot(g: &Graph) -> String {
    let mut out = String::new();
    out.push_str("digraph universe {\n");
    out.push_str(&format!("  label=\"{}\";\n", escape(&g.workspace_name)));
    out.push_str("  node [fontname=\"sans-serif\"];\n");
    out.push_str("  edge [fontname=\"sans-serif\"];\n");

    for n in &g.nodes {
        out.push_str(&node_line(n));
    }
    for l in &g.links {
        out.push_str(&link_line(l));
    }

    out.push_str("}\n");
    out
}

fn node_line(n: &GraphNode) -> String {
    format!(
        "  \"{}\" [label=\"{}\", shape={}];\n",
        escape(&n.id),
        escape(&n.name),
        shape_for(n.kind),
    )
}

fn link_line(l: &GraphLink) -> String {
    let style = edge_style(l.class);
    match &l.description {
        Some(desc) if !desc.is_empty() => format!(
            "  \"{}\" -> \"{}\" [label=\"{}\", style={}];\n",
            escape(&l.source_id),
            escape(&l.target_id),
            escape(desc),
            style,
        ),
        _ => format!(
            "  \"{}\" -> \"{}\" [style={}];\n",
            escape(&l.source_id),
            escape(&l.target_id),
            style,
        ),
    }
}
