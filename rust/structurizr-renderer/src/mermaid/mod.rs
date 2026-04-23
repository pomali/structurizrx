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
    let mut out = String::from("graph TD\n");
    let model = &workspace.model;

    if let Some(people) = &model.people {
        for p in people {
            let alias = safe_alias(&p.id);
            out.push_str(&format!("    {}[\"{}\\n[Person]\"]\n", alias, p.name));
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            let alias = safe_alias(&ss.id);
            out.push_str(&format!("    {}[\"{}\\n[Software System]\"]\n", alias, ss.name));
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

    out
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

fn safe_alias(id: &str) -> String {
    format!("elem{}", id.replace('-', "_").replace(' ', "_"))
}
