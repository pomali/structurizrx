//! Plain-text model digest (spec §9.1) and element/relationship naming,
//! shared by the CLI (`structurizrx digest`, `query`) and the web server.

use std::collections::HashMap;

use structurizr_model::{Port, Relationship, Workspace};

/// Map every static-model element id to its qualified name path
/// (`System/Container/Component`), the same paths the digest prints and
/// `reference.rs` resolves.
pub fn element_paths(ws: &Workspace) -> HashMap<String, String> {
    let mut paths = HashMap::new();
    for p in ws.model.people.iter().flatten() {
        paths.insert(p.id.clone(), p.name.clone());
    }
    for s in ws.model.software_systems.iter().flatten() {
        paths.insert(s.id.clone(), s.name.clone());
        for c in s.containers.iter().flatten() {
            let path = format!("{}/{}", s.name, c.name);
            paths.insert(c.id.clone(), path.clone());
            for comp in c.components.iter().flatten() {
                paths.insert(comp.id.clone(), format!("{}/{}", path, comp.name));
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        paths.insert(ce.id.clone(), ce.name.clone());
    }
    paths
}

/// A relationship described by name paths rather than ids.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipSummary {
    pub id: String,
    /// Source name path, with `.port` when the relationship leaves a port.
    pub source: String,
    /// Destination name path, with `.port` when it arrives at a port.
    pub destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduced: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retired: Option<String>,
}

impl RelationshipSummary {
    /// The digest's one-line form: `A -> B "desc" [kind, status:x]`.
    pub fn line(&self) -> String {
        let mut markers = Vec::new();
        if let Some(k) = &self.kind {
            markers.push(k.clone());
        }
        if let Some(s) = &self.status {
            markers.push(format!("status:{}", s));
        }
        if let Some(i) = &self.introduced {
            markers.push(format!("introduced:{}", i));
        }
        if let Some(r) = &self.retired {
            markers.push(format!("retired:{}", r));
        }
        let marker_s = if markers.is_empty() {
            String::new()
        } else {
            format!(" [{}]", markers.join(", "))
        };
        let desc = self
            .description
            .as_deref()
            .map(|d| format!(" \"{}\"", d))
            .unwrap_or_default();
        format!(
            "{} -> {}{}{}",
            self.source, self.destination, desc, marker_s
        )
    }
}

/// Every static-model relationship by id, described by name paths.
pub fn relationship_summaries(ws: &Workspace) -> HashMap<String, RelationshipSummary> {
    let paths = element_paths(ws);
    let mut port_names: HashMap<(String, String), String> = HashMap::new();
    let mut record_ports = |element_id: &str, ports: &Option<Vec<Port>>| {
        for p in ports.iter().flatten() {
            port_names.insert((element_id.to_string(), p.id.clone()), p.name.clone());
        }
    };
    for s in ws.model.software_systems.iter().flatten() {
        record_ports(&s.id, &s.ports);
        for c in s.containers.iter().flatten() {
            record_ports(&c.id, &c.ports);
            for comp in c.components.iter().flatten() {
                record_ports(&comp.id, &comp.ports);
            }
        }
    }

    let summarize = |r: &Relationship| -> RelationshipSummary {
        let mut src = paths
            .get(&r.source_id)
            .cloned()
            .unwrap_or_else(|| r.source_id.clone());
        let mut dst = paths
            .get(&r.destination_id)
            .cloned()
            .unwrap_or_else(|| r.destination_id.clone());
        if let Some(pid) = &r.source_port_id {
            if let Some(pname) = port_names.get(&(r.source_id.clone(), pid.clone())) {
                src = format!("{}.{}", src, pname);
            }
        }
        if let Some(pid) = &r.destination_port_id {
            if let Some(pname) = port_names.get(&(r.destination_id.clone(), pid.clone())) {
                dst = format!("{}.{}", dst, pname);
            }
        }
        RelationshipSummary {
            id: r.id.clone(),
            source: src,
            destination: dst,
            description: r.description.clone(),
            technology: r.technology.clone(),
            kind: r.kind.as_ref().map(|k| format!("{:?}", k).to_lowercase()),
            status: r.status.as_ref().map(|s| format!("{:?}", s).to_lowercase()),
            introduced: r.introduced.clone(),
            retired: r.retired.clone(),
        }
    };

    let mut out = HashMap::new();
    for r in all_relationships(ws) {
        out.insert(r.id.clone(), summarize(r));
    }
    out
}

/// Static-model relationships in model order (people, then systems depth
/// first, then custom elements).
pub fn all_relationships(ws: &Workspace) -> Vec<&Relationship> {
    let mut all = Vec::new();
    for p in ws.model.people.iter().flatten() {
        all.extend(p.relationships.iter().flatten());
    }
    for s in ws.model.software_systems.iter().flatten() {
        all.extend(s.relationships.iter().flatten());
        for c in s.containers.iter().flatten() {
            all.extend(c.relationships.iter().flatten());
            for comp in c.components.iter().flatten() {
                all.extend(comp.relationships.iter().flatten());
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        all.extend(ce.relationships.iter().flatten());
    }
    all
}

/// Render a compact, deterministic plain-text digest of the model (spec §9.1):
/// one line per element with its qualified name-path, ports, and markers; one
/// line per relationship as a name-path triple. Sized to paste into LLM context.
pub fn digest(ws: &Workspace) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "workspace: {}", ws.name);
    if let Some(desc) = &ws.description {
        let _ = writeln!(out, "description: {}", desc);
    }
    if let Some(ms) = &ws.milestones {
        let list: Vec<String> = ms
            .iter()
            .map(|m| match &m.date {
                Some(d) => format!("{}({})", m.name, d),
                None => m.name.clone(),
            })
            .collect();
        let _ = writeln!(out, "milestones: {}", list.join(", "));
    }
    if let Some(ps) = &ws.perspectives {
        let list: Vec<&str> = ps.iter().map(|p| p.name.as_str()).collect();
        let _ = writeln!(out, "perspectives: {}", list.join(", "));
    }
    let _ = writeln!(out);

    let paths = element_paths(ws);

    fn markers(
        status: &Option<structurizr_model::Status>,
        introduced: &Option<String>,
        retired: &Option<String>,
        technology: &Option<String>,
    ) -> String {
        let mut m = Vec::new();
        if let Some(t) = technology {
            m.push(t.clone());
        }
        if let Some(s) = status {
            m.push(format!("status:{}", format!("{:?}", s).to_lowercase()));
        }
        if let Some(i) = introduced {
            m.push(format!("introduced:{}", i));
        }
        if let Some(r) = retired {
            m.push(format!("retired:{}", r));
        }
        if m.is_empty() {
            String::new()
        } else {
            format!(" [{}]", m.join(", "))
        }
    }

    fn ports_suffix(ports: &Option<Vec<Port>>) -> String {
        match ports {
            Some(ps) if !ps.is_empty() => {
                let list: Vec<String> = ps
                    .iter()
                    .map(|p| {
                        let mut s = p.name.clone();
                        if let Some(proto) = &p.protocol {
                            s = format!("{}({})", s, proto);
                        }
                        s
                    })
                    .collect();
                format!(" ports: {}", list.join(", "))
            }
            _ => String::new(),
        }
    }

    for p in ws.model.people.iter().flatten() {
        let _ = writeln!(
            out,
            "person {}{}",
            p.name,
            markers(&p.status, &p.introduced, &p.retired, &None)
        );
    }
    for s in ws.model.software_systems.iter().flatten() {
        let _ = writeln!(
            out,
            "system {}{}{}",
            s.name,
            markers(&s.status, &s.introduced, &s.retired, &None),
            ports_suffix(&s.ports)
        );
        for c in s.containers.iter().flatten() {
            let path = &paths[&c.id];
            let _ = writeln!(
                out,
                "  container {}{}{}",
                path,
                markers(&c.status, &c.introduced, &c.retired, &c.technology),
                ports_suffix(&c.ports)
            );
            for comp in c.components.iter().flatten() {
                let cpath = &paths[&comp.id];
                let _ = writeln!(
                    out,
                    "    component {}{}{}",
                    cpath,
                    markers(
                        &comp.status,
                        &comp.introduced,
                        &comp.retired,
                        &comp.technology
                    ),
                    ports_suffix(&comp.ports)
                );
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        let _ = writeln!(out, "element {}", ce.name);
    }
    let _ = writeln!(out);

    // Relationships, in model order
    let summaries = relationship_summaries(ws);
    for r in all_relationships(ws) {
        let _ = writeln!(out, "rel {}", summaries[&r.id].line());
    }

    // Views: key, type, scope, and content size — so a reader can see which
    // views exist and what a model change would affect. Generated views carry
    // an `auto-` key prefix. Relationship counts include the implied
    // relationships the renderers lift onto visible ancestors.
    let scope_of = |id: &str| paths.get(id).cloned().unwrap_or_else(|| id.to_string());
    fn counts(
        ev: &Option<Vec<structurizr_model::ElementView>>,
        rv: &Option<Vec<structurizr_model::RelationshipView>>,
    ) -> String {
        format!(
            "{} elements, {} rels",
            ev.as_ref().map_or(0, |v| v.len()),
            rv.as_ref().map_or(0, |v| v.len())
        )
    }
    let mut view_lines: Vec<String> = Vec::new();
    for v in ws.views.system_landscape_views.iter().flatten() {
        view_lines.push(format!(
            "view {} landscape ({})",
            v.key.as_deref().unwrap_or("?"),
            counts(&v.element_views, &v.relationship_views)
        ));
    }
    for v in ws.views.system_context_views.iter().flatten() {
        view_lines.push(format!(
            "view {} systemContext of {} ({})",
            v.key.as_deref().unwrap_or("?"),
            scope_of(&v.software_system_id),
            counts(&v.element_views, &v.relationship_views)
        ));
    }
    for v in ws.views.container_views.iter().flatten() {
        view_lines.push(format!(
            "view {} container of {} ({})",
            v.key.as_deref().unwrap_or("?"),
            scope_of(&v.software_system_id),
            counts(&v.element_views, &v.relationship_views)
        ));
    }
    for v in ws.views.component_views.iter().flatten() {
        view_lines.push(format!(
            "view {} component of {} ({})",
            v.key.as_deref().unwrap_or("?"),
            scope_of(&v.container_id),
            counts(&v.element_views, &v.relationship_views)
        ));
    }
    for v in ws.views.dynamic_views.iter().flatten() {
        let scope = v.element_id.as_deref().map(scope_of);
        view_lines.push(format!(
            "view {} dynamic{} ({})",
            v.key.as_deref().unwrap_or("?"),
            scope.map(|s| format!(" of {}", s)).unwrap_or_default(),
            counts(&v.element_views, &v.relationship_views)
        ));
    }
    for v in ws.views.deployment_views.iter().flatten() {
        view_lines.push(format!(
            "view {} deployment env={} ({})",
            v.key.as_deref().unwrap_or("?"),
            v.environment,
            counts(&v.element_views, &v.relationship_views)
        ));
    }
    for v in ws.views.filtered_views.iter().flatten() {
        view_lines.push(format!(
            "view {} filtered base={} mode={}",
            v.key.as_deref().unwrap_or("?"),
            v.base_view_key,
            v.mode
        ));
    }
    if !view_lines.is_empty() {
        let _ = writeln!(out);
        for line in view_lines {
            let _ = writeln!(out, "{}", line);
        }
    }
    out
}

/// Map every element id in the model to a human-readable name for query output.
pub fn element_names(ws: &Workspace) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for p in ws.model.people.iter().flatten() {
        names.insert(p.id.clone(), format!("person \"{}\"", p.name));
    }
    for s in ws.model.software_systems.iter().flatten() {
        names.insert(s.id.clone(), format!("softwareSystem \"{}\"", s.name));
        for c in s.containers.iter().flatten() {
            names.insert(c.id.clone(), format!("container \"{}\"", c.name));
            for comp in c.components.iter().flatten() {
                names.insert(comp.id.clone(), format!("component \"{}\"", comp.name));
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        names.insert(ce.id.clone(), format!("element \"{}\"", ce.name));
    }
    names
}
