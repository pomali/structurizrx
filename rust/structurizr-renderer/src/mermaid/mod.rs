use structurizr_model::*;

use crate::diagram::{Diagram, DiagramFormat};
use crate::exporter::DiagramExporter;

/// Mermaid diagram exporter.
pub struct MermaidExporter;

impl DiagramExporter for MermaidExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram> {
        let mut diagrams = Vec::new();

        if let Some(sc_views) = &workspace.views.system_context_views {
            for v in sc_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemContext".to_string());
                let content = render_mermaid(workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::Mermaid));
            }
        }

        if let Some(sl_views) = &workspace.views.system_landscape_views {
            for v in sl_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemLandscape".to_string());
                let content = render_mermaid(workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::Mermaid));
            }
        }

        diagrams
    }
}

fn render_mermaid(workspace: &Workspace) -> String {
    let styles = get_styles(workspace);
    let mut out = String::from("graph TD\n");
    let model = &workspace.model;

    // Track (alias → css_class) mappings to emit at the end.
    let mut class_assignments: Vec<(String, String)> = Vec::new();
    // Collect unique class definitions: class_name → "fill:…,color:…"
    let mut class_defs: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    if let Some(people) = &model.people {
        for p in people {
            let alias = safe_alias(&p.id);
            out.push_str(&format!("    {}[\"{}\\n[Person]\"]\n", alias, p.name));
            if let Some((cls, def)) = build_mermaid_class(p.tags.as_deref(), "Person", styles) {
                class_defs.entry(cls.clone()).or_insert(def);
                class_assignments.push((alias, cls));
            }
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            let alias = safe_alias(&ss.id);
            out.push_str(&format!("    {}[\"{}\\n[Software System]\"]\n", alias, ss.name));
            if let Some((cls, def)) = build_mermaid_class(ss.tags.as_deref(), "Software System", styles) {
                class_defs.entry(cls.clone()).or_insert(def);
                class_assignments.push((alias, cls));
            }
        }
    }

    // Relationships
    if let Some(people) = &model.people {
        for p in people {
            if let Some(rels) = &p.relationships {
                for rel in rels {
                    emit_mermaid_rel(rel, &mut out);
                }
            }
        }
    }
    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if let Some(rels) = &ss.relationships {
                for rel in rels {
                    emit_mermaid_rel(rel, &mut out);
                }
            }
            if let Some(containers) = &ss.containers {
                for c in containers {
                    if let Some(rels) = &c.relationships {
                        for rel in rels {
                            emit_mermaid_rel(rel, &mut out);
                        }
                    }
                }
            }
        }
    }

    // Emit classDef lines
    let mut defs: Vec<(&String, &String)> = class_defs.iter().collect();
    defs.sort_by_key(|(k, _)| k.as_str());
    for (cls, def) in defs {
        out.push_str(&format!("    classDef {} {}\n", cls, def));
    }

    // Emit class assignment lines
    for (alias, cls) in &class_assignments {
        out.push_str(&format!("    class {} {}\n", alias, cls));
    }

    out
}

/// Build a Mermaid class name + definition for a node, if any matching styles exist.
/// Returns `(class_name, "fill:…,color:…,stroke:…")` or `None` if no styles apply.
fn build_mermaid_class(
    tags: Option<&str>,
    default_type_tag: &str,
    styles: Option<&Styles>,
) -> Option<(String, String)> {
    let styles = styles?;
    let element_styles = styles.elements.as_ref()?;

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
    let mut last_matching_tag = String::new();

    for tag in tags_str.split(',').map(|t| t.trim()) {
        for style in element_styles {
            if style.tag.eq_ignore_ascii_case(tag) {
                if let Some(b) = &style.background { bg = Some(b.clone()); }
                if let Some(c) = &style.color      { fg = Some(c.clone()); }
                if let Some(s) = &style.stroke     { stroke = Some(s.clone()); }
                last_matching_tag = tag.to_string();
            }
        }
    }

    if bg.is_none() && fg.is_none() && stroke.is_none() {
        return None;
    }

    // Use the last matched tag as the class name (sanitized)
    let cls = sanitize_mermaid_class(&last_matching_tag);
    let mut parts: Vec<String> = Vec::new();
    if let Some(b) = bg     { parts.push(format!("fill:{}", b)); }
    if let Some(f) = fg     { parts.push(format!("color:{}", f)); }
    if let Some(s) = stroke { parts.push(format!("stroke:{}", s)); }
    let def = parts.join(",");

    Some((cls, def))
}

fn sanitize_mermaid_class(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

fn emit_mermaid_rel(rel: &Relationship, out: &mut String) {
    let src = safe_alias(&rel.source_id);
    let dst = safe_alias(&rel.destination_id);
    let desc = rel.description.as_deref().unwrap_or("");
    if desc.is_empty() {
        out.push_str(&format!("    {} --> {}\n", src, dst));
    } else {
        out.push_str(&format!("    {} -->|\"{}\"|{}\n", src, desc, dst));
    }
}

/// Extract the workspace-level element styles, if any.
fn get_styles(workspace: &Workspace) -> Option<&Styles> {
    workspace.views.configuration.as_ref()?.styles.as_ref()
}

fn safe_alias(id: &str) -> String {
    format!("elem{}", id.replace('-', "_").replace(' ', "_"))
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
    fn mermaid_exporter_basic() {
        let workspace = basic_workspace_with_landscape();
        let exporter = MermaidExporter;
        let diagrams = exporter.export_workspace(&workspace);
        assert_eq!(diagrams.len(), 1);
        assert!(diagrams[0].content.starts_with("graph TD"));
        assert!(diagrams[0].content.contains("Alice"));
        assert!(diagrams[0].content.contains("MySystem"));
    }

    #[test]
    fn mermaid_exporter_respects_element_styles() {
        let mut workspace = basic_workspace_with_landscape();
        workspace.views.configuration = Some(ViewConfiguration {
            styles: Some(Styles {
                elements: Some(vec![ElementStyle {
                    tag: "Person".to_string(),
                    background: Some("#CC0000".to_string()),
                    color: Some("#FFFFFF".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let exporter = MermaidExporter;
        let diagrams = exporter.export_workspace(&workspace);
        let md = &diagrams[0].content;
        assert!(md.contains("classDef"), "should emit classDef");
        assert!(md.contains("#CC0000"), "fill colour should appear in classDef");
        assert!(md.contains("#FFFFFF"), "text colour should appear in classDef");
        assert!(md.contains("class "), "should assign class to nodes");
    }
}
