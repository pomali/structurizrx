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
    let mut out = format!("digraph {} {{\n", safe_id(graph_name));
    out.push_str("    rankdir=TB;\n");
    out.push_str("    node [shape=box];\n\n");

    let model = &workspace.model;

    if let Some(people) = &model.people {
        for p in people {
            let alias = safe_alias(&p.id);
            out.push_str(&format!("    {} [label=\"{}\\n[Person]\"];\n", alias, p.name));
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            let alias = safe_alias(&ss.id);
            out.push_str(&format!("    {} [label=\"{}\\n[Software System]\"];\n", alias, ss.name));
            if let Some(containers) = &ss.containers {
                for c in containers {
                    let calias = safe_alias(&c.id);
                    let tech = c.technology.as_deref().unwrap_or("");
                    out.push_str(&format!(
                        "    {} [label=\"{}\\n[Container: {}]\"];\n",
                        calias, c.name, tech
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
                    emit_dot_rel(rel, &mut out);
                }
            }
        }
    }
    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if let Some(rels) = &ss.relationships {
                for rel in rels {
                    emit_dot_rel(rel, &mut out);
                }
            }
            if let Some(containers) = &ss.containers {
                for c in containers {
                    if let Some(rels) = &c.relationships {
                        for rel in rels {
                            emit_dot_rel(rel, &mut out);
                        }
                    }
                }
            }
        }
    }

    out.push_str("}\n");
    out
}

fn emit_dot_rel(rel: &Relationship, out: &mut String) {
    let src = safe_alias(&rel.source_id);
    let dst = safe_alias(&rel.destination_id);
    let desc = rel.description.as_deref().unwrap_or("");
    if desc.is_empty() {
        out.push_str(&format!("    {} -> {};\n", src, dst));
    } else {
        out.push_str(&format!("    {} -> {} [label=\"{}\"];\n", src, dst, desc));
    }
}

fn safe_alias(id: &str) -> String {
    format!("n{}", id.replace('-', "_").replace(' ', "_"))
}

fn safe_id(s: &str) -> String {
    s.replace('-', "_").replace(' ', "_")
}
