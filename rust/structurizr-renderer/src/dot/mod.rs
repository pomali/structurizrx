use structurizr_model::*;

use crate::diagram::{Diagram, DiagramFormat};
use crate::exporter::DiagramExporter;

/// DOT/Graphviz diagram exporter.
pub struct DotExporter;

impl DiagramExporter for DotExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram> {
        let mut diagrams = Vec::new();

        if let Some(sc_views) = &workspace.views.system_context_views {
            for v in sc_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemContext".to_string());
                let content = render_dot(workspace, &key);
                diagrams.push(Diagram::new(key, content, DiagramFormat::Dot));
            }
        }

        if let Some(sl_views) = &workspace.views.system_landscape_views {
            for v in sl_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemLandscape".to_string());
                let content = render_dot(workspace, &key);
                diagrams.push(Diagram::new(key, content, DiagramFormat::Dot));
            }
        }

        diagrams
    }
}

fn render_dot(workspace: &Workspace, graph_name: &str) -> String {
    let styles = get_styles(workspace);
    let mut out = format!("digraph {} {{\n", safe_id(graph_name));
    out.push_str("    rankdir=TB;\n");
    out.push_str("    node [shape=box];\n\n");

    let model = &workspace.model;

    if let Some(people) = &model.people {
        for p in people {
            let alias = safe_alias(&p.id);
            let attrs = node_style_attrs(p.tags.as_deref(), "Person", styles);
            out.push_str(&format!(
                "    {} [label=\"{}\\n[Person]\"{}];\n",
                alias, p.name, attrs
            ));
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            let alias = safe_alias(&ss.id);
            let attrs = node_style_attrs(ss.tags.as_deref(), "Software System", styles);
            out.push_str(&format!(
                "    {} [label=\"{}\\n[Software System]\"{}];\n",
                alias, ss.name, attrs
            ));
            if let Some(containers) = &ss.containers {
                for c in containers {
                    let calias = safe_alias(&c.id);
                    let tech = c.technology.as_deref().unwrap_or("");
                    let cattrs = node_style_attrs(c.tags.as_deref(), "Container", styles);
                    out.push_str(&format!(
                        "    {} [label=\"{}\\n[Container: {}]\"{}];\n",
                        calias, c.name, tech, cattrs
                    ));
                }
            }
        }
    }

    out.push('\n');

    if let Some(people) = &model.people {
        for p in people {
            if let Some(rels) = &p.relationships {
                for rel in rels {
                    emit_dot_rel(rel, &mut out, styles);
                }
            }
        }
    }
    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if let Some(rels) = &ss.relationships {
                for rel in rels {
                    emit_dot_rel(rel, &mut out, styles);
                }
            }
            if let Some(containers) = &ss.containers {
                for c in containers {
                    if let Some(rels) = &c.relationships {
                        for rel in rels {
                            emit_dot_rel(rel, &mut out, styles);
                        }
                    }
                }
            }
        }
    }

    out.push_str("}\n");
    out
}

fn emit_dot_rel(rel: &Relationship, out: &mut String, styles: Option<&Styles>) {
    let src = safe_alias(&rel.source_id);
    let dst = safe_alias(&rel.destination_id);
    let desc = rel.description.as_deref().unwrap_or("");
    let edge_attrs = rel_style_attrs(rel.tags.as_deref(), styles);
    if desc.is_empty() {
        out.push_str(&format!("    {} -> {}{};\n", src, dst, edge_attrs));
    } else {
        out.push_str(&format!(
            "    {} -> {} [label=\"{}\"{}];\n",
            src, dst, desc, edge_attrs_without_brackets(&edge_attrs)
        ));
    }
}

/// Build a DOT attribute string (e.g. `, style=filled, fillcolor="#ff0000"`)
/// based on element styles for the given tag list.
fn node_style_attrs(tags: Option<&str>, default_type_tag: &str, styles: Option<&Styles>) -> String {
    let Some(styles) = styles else { return String::new() };
    let Some(element_styles) = &styles.elements else { return String::new() };

    let owned;
    let tags_str: &str = match tags {
        Some(t) => t,
        None => {
            owned = format!("Element,{}", default_type_tag);
            &owned
        }
    };

    let mut bg: Option<String> = None;
    let mut fg: Option<String> = None;
    let mut stroke: Option<String> = None;

    for tag in tags_str.split(',').map(|t| t.trim()) {
        for style in element_styles {
            if style.tag.eq_ignore_ascii_case(tag) {
                if let Some(b) = &style.background { bg = Some(b.clone()); }
                if let Some(c) = &style.color     { fg = Some(c.clone()); }
                if let Some(s) = &style.stroke    { stroke = Some(s.clone()); }
            }
        }
    }

    let mut attrs = String::new();
    if bg.is_some() || fg.is_some() || stroke.is_some() {
        attrs.push_str(", style=filled");
        if let Some(b) = bg    { attrs.push_str(&format!(", fillcolor=\"{}\"", b)); }
        if let Some(f) = fg    { attrs.push_str(&format!(", fontcolor=\"{}\"", f)); }
        if let Some(s) = stroke { attrs.push_str(&format!(", color=\"{}\"", s)); }
    }
    attrs
}

/// Build DOT edge attribute extras from relationship styles.
fn rel_style_attrs(tags: Option<&str>, styles: Option<&Styles>) -> String {
    let Some(styles) = styles else { return String::new() };
    let Some(rel_styles) = &styles.relationships else { return String::new() };

    let tags_str = tags.unwrap_or("Relationship");
    let mut color: Option<String> = None;
    let mut dashed: Option<bool> = None;

    for tag in tags_str.split(',').map(|t| t.trim()) {
        for style in rel_styles {
            if style.tag.eq_ignore_ascii_case(tag) {
                if let Some(c) = &style.color   { color = Some(c.clone()); }
                if let Some(d) = style.dashed    { dashed = Some(d); }
            }
        }
    }

    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = color  { parts.push(format!("color=\"{}\"", c)); }
    if let Some(d) = dashed {
        if d { parts.push("style=dashed".to_string()); } else { parts.push("style=solid".to_string()); }
    }
    if parts.is_empty() { return String::new(); }
    format!(" [{}]", parts.join(", "))
}

/// When we already have a `label` attribute we only need the extra attrs inside `[…]`.
fn edge_attrs_without_brackets(attrs: &str) -> String {
    let trimmed = attrs.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner.is_empty() {
            return String::new();
        }
        return format!(", {}", inner);
    }
    attrs.to_string()
}

/// Extract the workspace-level element styles, if any.
fn get_styles(workspace: &Workspace) -> Option<&Styles> {
    workspace.views.configuration.as_ref()?.styles.as_ref()
}

fn safe_alias(id: &str) -> String {
    format!("n{}", id.replace('-', "_").replace(' ', "_"))
}

fn safe_id(s: &str) -> String {
    s.replace('-', "_").replace(' ', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{
        ElementStyle, Person, SoftwareSystem, Styles, SystemLandscapeView, ViewConfiguration,
        Workspace,
    };

    fn basic_workspace_with_landscape() -> Workspace {
        let mut ws = Workspace::default();
        ws.name = "Test".to_string();
        ws.model.people = Some(vec![Person {
            id: "1".to_string(),
            name: "Alice".to_string(),
            ..Default::default()
        }]);
        ws.model.software_systems = Some(vec![SoftwareSystem {
            id: "2".to_string(),
            name: "MySystem".to_string(),
            ..Default::default()
        }]);
        ws.views.system_landscape_views = Some(vec![SystemLandscapeView {
            key: Some("Landscape".to_string()),
            ..Default::default()
        }]);
        ws
    }

    #[test]
    fn dot_exporter_basic() {
        let workspace = basic_workspace_with_landscape();
        let exporter = DotExporter;
        let diagrams = exporter.export_workspace(&workspace);
        assert_eq!(diagrams.len(), 1);
        assert!(diagrams[0].content.contains("digraph"));
        assert!(diagrams[0].content.contains("Alice"));
        assert!(diagrams[0].content.contains("MySystem"));
    }

    #[test]
    fn dot_exporter_respects_element_styles() {
        let mut workspace = basic_workspace_with_landscape();
        workspace.views.configuration = Some(ViewConfiguration {
            styles: Some(Styles {
                elements: Some(vec![ElementStyle {
                    tag: "Person".to_string(),
                    background: Some("#AA0000".to_string()),
                    color: Some("#FFFFFF".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let exporter = DotExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let dot = &diagrams[0].content;
        assert!(dot.contains("fillcolor=\"#AA0000\""), "fillcolor should appear in DOT output");
        assert!(dot.contains("fontcolor=\"#FFFFFF\""), "fontcolor should appear in DOT output");
        assert!(dot.contains("style=filled"), "style=filled must be present");
    }
}
