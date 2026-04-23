use structurizr_model::*;

use crate::diagram::{Diagram, DiagramFormat};
use crate::exporter::DiagramExporter;

/// PlantUML C4 diagram exporter.
pub struct PlantUmlExporter;

impl DiagramExporter for PlantUmlExporter {
    fn export_workspace(&self, workspace: &Workspace) -> Vec<Diagram> {
        let mut diagrams = Vec::new();
        let views = &workspace.views;

        if let Some(sc_views) = &views.system_context_views {
            for v in sc_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemContext".to_string());
                let content = render_system_context(v, workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::PlantUml));
            }
        }

        if let Some(sl_views) = &views.system_landscape_views {
            for v in sl_views {
                let key = v.key.clone().unwrap_or_else(|| "SystemLandscape".to_string());
                let content = render_system_landscape(v, workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::PlantUml));
            }
        }

        if let Some(cv) = &views.container_views {
            for v in cv {
                let key = v.key.clone().unwrap_or_else(|| "Container".to_string());
                let content = render_container_view(v, workspace);
                diagrams.push(Diagram::new(key, content, DiagramFormat::PlantUml));
            }
        }

        diagrams
    }
}

fn render_system_context(view: &SystemContextView, workspace: &Workspace) -> String {
    let mut out = String::new();
    out.push_str("@startuml\n");
    out.push_str("!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Context.puml\n\n");

    if let Some(title) = &view.title {
        out.push_str(&format!("title {}\n\n", title));
    }

    let model = &workspace.model;

    if let Some(people) = &model.people {
        for p in people {
            let alias = safe_alias(&p.id);
            let desc = p.description.as_deref().unwrap_or("");
            out.push_str(&format!("Person({}, \"{}\", \"{}\")\n", alias, p.name, desc));
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            let alias = safe_alias(&ss.id);
            let desc = ss.description.as_deref().unwrap_or("");
            out.push_str(&format!("System({}, \"{}\", \"{}\")\n", alias, ss.name, desc));
        }
    }

    out.push('\n');
    emit_relationships(model, &mut out);
    out.push_str("\n@enduml\n");
    out
}

fn render_system_landscape(view: &SystemLandscapeView, workspace: &Workspace) -> String {
    let mut out = String::new();
    out.push_str("@startuml\n");
    out.push_str("!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Context.puml\n\n");

    if let Some(title) = &view.title {
        out.push_str(&format!("title {}\n\n", title));
    }

    let model = &workspace.model;

    if let Some(people) = &model.people {
        for p in people {
            let alias = safe_alias(&p.id);
            let desc = p.description.as_deref().unwrap_or("");
            out.push_str(&format!("Person({}, \"{}\", \"{}\")\n", alias, p.name, desc));
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            let alias = safe_alias(&ss.id);
            let desc = ss.description.as_deref().unwrap_or("");
            out.push_str(&format!("System({}, \"{}\", \"{}\")\n", alias, ss.name, desc));
        }
    }

    out.push('\n');
    emit_relationships(model, &mut out);
    out.push_str("\n@enduml\n");
    out
}

fn render_container_view(view: &ContainerView, workspace: &Workspace) -> String {
    let mut out = String::new();
    out.push_str("@startuml\n");
    out.push_str("!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Container.puml\n\n");

    if let Some(title) = &view.title {
        out.push_str(&format!("title {}\n\n", title));
    }

    let model = &workspace.model;

    if let Some(people) = &model.people {
        for p in people {
            let alias = safe_alias(&p.id);
            let desc = p.description.as_deref().unwrap_or("");
            out.push_str(&format!("Person({}, \"{}\", \"{}\")\n", alias, p.name, desc));
        }
    }

    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if ss.id == view.software_system_id {
                out.push_str(&format!("System_Boundary({}, \"{}\") {{\n", safe_alias(&ss.id), ss.name));
                if let Some(containers) = &ss.containers {
                    for c in containers {
                        let alias = safe_alias(&c.id);
                        let desc = c.description.as_deref().unwrap_or("");
                        let tech = c.technology.as_deref().unwrap_or("");
                        out.push_str(&format!(
                            "    Container({}, \"{}\", \"{}\", \"{}\")\n",
                            alias, c.name, tech, desc
                        ));
                    }
                }
                out.push_str("}\n");
            } else {
                let alias = safe_alias(&ss.id);
                let desc = ss.description.as_deref().unwrap_or("");
                out.push_str(&format!("System_Ext({}, \"{}\", \"{}\")\n", alias, ss.name, desc));
            }
        }
    }

    out.push('\n');
    emit_relationships(model, &mut out);
    out.push_str("\n@enduml\n");
    out
}

fn emit_relationships(model: &Model, out: &mut String) {
    if let Some(people) = &model.people {
        for p in people {
            if let Some(rels) = &p.relationships {
                for rel in rels {
                    emit_rel(rel, out);
                }
            }
        }
    }
    if let Some(systems) = &model.software_systems {
        for ss in systems {
            if let Some(rels) = &ss.relationships {
                for rel in rels {
                    emit_rel(rel, out);
                }
            }
            if let Some(containers) = &ss.containers {
                for c in containers {
                    if let Some(rels) = &c.relationships {
                        for rel in rels {
                            emit_rel(rel, out);
                        }
                    }
                    if let Some(components) = &c.components {
                        for comp in components {
                            if let Some(rels) = &comp.relationships {
                                for rel in rels {
                                    emit_rel(rel, out);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn emit_rel(rel: &Relationship, out: &mut String) {
    let src = safe_alias(&rel.source_id);
    let dst = safe_alias(&rel.destination_id);
    let desc = rel.description.as_deref().unwrap_or("");
    let tech = rel.technology.as_deref().unwrap_or("");
    if tech.is_empty() {
        out.push_str(&format!("Rel({}, {}, \"{}\")\n", src, dst, desc));
    } else {
        out.push_str(&format!("Rel({}, {}, \"{}\", \"{}\")\n", src, dst, desc, tech));
    }
}

fn safe_alias(id: &str) -> String {
    format!("elem_{}", id.replace('-', "_").replace(' ', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{Person, SoftwareSystem, Workspace};

    #[test]
    fn plantuml_exporter_basic() {
        let mut workspace = Workspace::default();
        workspace.name = "Test".to_string();

        let person = Person { id: "1".to_string(), name: "User".to_string(), ..Default::default() };
        let system = SoftwareSystem { id: "2".to_string(), name: "System".to_string(), ..Default::default() };

        workspace.model.people = Some(vec![person]);
        workspace.model.software_systems = Some(vec![system]);
        workspace.views.system_context_views = Some(vec![SystemContextView {
            software_system_id: "2".to_string(),
            key: Some("SystemContext".to_string()),
            ..Default::default()
        }]);

        let exporter = PlantUmlExporter;
        let diagrams = exporter.export_workspace(&workspace);
        assert_eq!(diagrams.len(), 1);
        assert!(diagrams[0].content.contains("@startuml"));
        assert!(diagrams[0].content.contains("Person("));
        assert!(diagrams[0].content.contains("System("));
    }
}
