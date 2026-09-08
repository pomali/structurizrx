//! The workspace graph index: one pass over the model that resolves hierarchy,
//! adjacency and view membership, shared by every consumer that needs to walk
//! the model rather than just serialise it.
//!
//! [`build_index`] is the single entry point. The selector engine ([`crate::eval`]),
//! the lint checks and the review/walkthrough API all read the same index, so a
//! model fact is derived here once rather than re-traversed per feature.
//!
//! Ordering is deterministic everywhere: entries follow model order and
//! adjacency is stored as `Vec`s of indices into that order, never as a
//! `HashMap` iteration. Anything built on top (cluster detection, CI
//! comparisons) can therefore be reproduced run to run.
//!
//! Scope note: this indexes the static model only — people, software systems,
//! containers, components and custom elements. Deployment nodes and instances
//! are deliberately absent, matching the selector engine's element kinds
//! (spec §6.2); [`crate::graph`] is the projection that does include them.

use std::collections::HashMap;

use structurizr_model::{RelationshipKind, Status, Workspace};
// ---------------------------------------------------------------------------
// Index entries
// ---------------------------------------------------------------------------

/// Everything about one model element needed for query evaluation.
#[derive(Debug)]
pub struct ElementEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Structural kind: "person", "softwareSystem", "container", "component", "custom".
    pub kind: &'static str,
    pub tags: Vec<String>,
    pub group: Option<String>,
    pub technology: Option<String>,
    /// Lowercase serde name of the status variant ("idea", "draft", …).
    pub status: Option<String>,
    pub properties: HashMap<String, String>,
    /// Names of perspectives carried by this element.
    pub perspectives: Vec<String>,
    /// Direct parent element id (None for top-level elements).
    pub parent_id: Option<String>,
    /// All ancestor ids from closest to farthest (for `parent^`).
    pub ancestors: Vec<String>,
    /// Names of the corresponding ancestors.
    pub ancestor_names: Vec<String>,
    /// Lifecycle milestone names (spec §8).
    pub introduced: Option<String>,
    pub retired: Option<String>,
    /// Declared ports as (port id, port name).
    pub ports: Vec<(String, String)>,
}

/// Everything about one relationship needed for query evaluation.
#[derive(Debug)]
pub struct RelationshipEntry {
    pub id: String,
    pub source_id: String,
    pub dest_id: String,
    pub description: Option<String>,
    pub technology: Option<String>,
    /// Lowercase serde name ("sync", "async", …).
    pub kind: Option<String>,
    /// Lowercase serde name.
    pub status: Option<String>,
    pub tags: Vec<String>,
    pub perspectives: Vec<String>,
    pub properties: HashMap<String, String>,
    pub introduced: Option<String>,
    pub retired: Option<String>,
    pub source_port_id: Option<String>,
    pub dest_port_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

pub struct Index {
    pub elements: Vec<ElementEntry>,
    pub relationships: Vec<RelationshipEntry>,
    /// Every view in the workspace, in model order.
    pub views: Vec<ViewEntry>,
    /// Maps element id → index into `elements`.
    pub by_id: HashMap<String, usize>,
    /// Maps lowercase element name → index into `elements`.
    pub by_name: HashMap<String, usize>,
    /// Per element (by index into `elements`), the indices into
    /// `relationships` of the relationships leaving it, in model order.
    outgoing: Vec<Vec<usize>>,
    /// Per element, the indices of the relationships arriving at it.
    incoming: Vec<Vec<usize>>,
    /// Per element, the indices of its direct children.
    children: Vec<Vec<usize>>,
    /// Per element, the indices into `views` of the views that show it.
    appears_in: Vec<Vec<usize>>,
}

/// One view, reduced to what a reader needs to name and locate it.
#[derive(Debug, Clone)]
pub struct ViewEntry {
    pub key: String,
    /// The view's title, falling back to its key.
    pub name: String,
    /// `system landscape`, `system context`, `container`, `component`,
    /// `dynamic`, `deployment` or `custom`.
    pub kind: &'static str,
    pub description: Option<String>,
    /// Ids of the elements the view shows, in view order.
    pub element_ids: Vec<String>,
}

impl Index {
    /// The entry for `id`, if the model has that element.
    pub fn element(&self, id: &str) -> Option<&ElementEntry> {
        self.by_id.get(id).map(|&i| &self.elements[i])
    }

    /// The entry for `name` (case-insensitive), if exactly that name exists.
    pub fn element_by_name(&self, name: &str) -> Option<&ElementEntry> {
        self.by_name.get(&name.to_lowercase()).map(|&i| &self.elements[i])
    }

    /// Relationships leaving `id`, in model order.
    pub fn outgoing(&self, id: &str) -> Vec<&RelationshipEntry> {
        self.adjacent(&self.outgoing, id)
    }

    /// Relationships arriving at `id`, in model order.
    pub fn incoming(&self, id: &str) -> Vec<&RelationshipEntry> {
        self.adjacent(&self.incoming, id)
    }

    fn adjacent(&self, table: &[Vec<usize>], id: &str) -> Vec<&RelationshipEntry> {
        match self.by_id.get(id) {
            Some(&i) => table[i].iter().map(|&r| &self.relationships[r]).collect(),
            None => Vec::new(),
        }
    }

    /// Direct children of `id` (containers of a system, components of a
    /// container), in model order.
    pub fn children(&self, id: &str) -> Vec<&ElementEntry> {
        match self.by_id.get(id) {
            Some(&i) => self.children[i].iter().map(|&c| &self.elements[c]).collect(),
            None => Vec::new(),
        }
    }

    /// The views that show `id`.
    ///
    /// Membership is literal: a view lists the elements it draws, so a software
    /// system is *not* reported as appearing in the container view that draws
    /// its children.
    pub fn views_for(&self, id: &str) -> Vec<&ViewEntry> {
        match self.by_id.get(id) {
            Some(&i) => self.appears_in[i].iter().map(|&v| &self.views[v]).collect(),
            None => Vec::new(),
        }
    }

    /// Incoming and outgoing relationship counts (afferent, efferent).
    pub fn degree(&self, id: &str) -> (usize, usize) {
        match self.by_id.get(id) {
            Some(&i) => (self.incoming[i].len(), self.outgoing[i].len()),
            None => (0, 0),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Idea => "idea",
        Status::Draft => "draft",
        Status::Specified => "specified",
        Status::Implemented => "implemented",
        Status::Deprecated => "deprecated",
    }
}

fn rel_kind_str(k: RelationshipKind) -> &'static str {
    match k {
        RelationshipKind::Sync => "sync",
        RelationshipKind::Async => "async",
        RelationshipKind::Publish => "publish",
        RelationshipKind::Subscribe => "subscribe",
        RelationshipKind::Dataflow => "dataflow",
        RelationshipKind::Dependency => "dependency",
        RelationshipKind::Deploy => "deploy",
    }
}

fn split_tags(tags: &Option<String>) -> Vec<String> {
    match tags {
        Some(t) => t
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => vec![],
    }
}

fn persp_names(ps: &Option<Vec<structurizr_model::Perspective>>) -> Vec<String> {
    match ps {
        Some(v) => v.iter().map(|p| p.name.clone()).collect(),
        None => vec![],
    }
}

fn props(m: &Option<HashMap<String, String>>) -> HashMap<String, String> {
    m.clone().unwrap_or_default()
}

fn port_pairs(ports: &Option<Vec<structurizr_model::Port>>) -> Vec<(String, String)> {
    ports
        .iter()
        .flatten()
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Index construction
// ---------------------------------------------------------------------------

pub fn build_index(workspace: &Workspace) -> Index {
    let mut elements: Vec<ElementEntry> = Vec::new();
    let mut relationships: Vec<RelationshipEntry> = Vec::new();

    // Helper: push one relationship entry.
    let mut push_rel = |r: &structurizr_model::Relationship| {
        relationships.push(RelationshipEntry {
            id: r.id.clone(),
            source_id: r.source_id.clone(),
            dest_id: r.destination_id.clone(),
            description: r.description.clone(),
            technology: r.technology.clone(),
            kind: r.kind.map(|k| rel_kind_str(k).to_string()),
            status: r.status.map(|s| status_str(s).to_string()),
            tags: split_tags(&r.tags),
            perspectives: persp_names(&r.perspectives),
            properties: props(&r.properties),
            introduced: r.introduced.clone(),
            retired: r.retired.clone(),
            source_port_id: r.source_port_id.clone(),
            dest_port_id: r.destination_port_id.clone(),
        });
    };

    let model = &workspace.model;

    // --- People ---
    for p in model.people.as_deref().unwrap_or(&[]) {
        elements.push(ElementEntry {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            kind: "person",
            tags: split_tags(&p.tags),
            group: p.group.clone(),
            technology: None,
            status: p.status.map(|s| status_str(s).to_string()),
            properties: props(&p.properties),
            perspectives: persp_names(&p.perspectives),
            parent_id: None,
            ancestors: vec![],
            ancestor_names: vec![],
            introduced: p.introduced.clone(),
            retired: p.retired.clone(),
            ports: port_pairs(&p.ports),
        });
        for r in p.relationships.as_deref().unwrap_or(&[]) {
            push_rel(r);
        }
    }

    // --- Software systems → containers → components ---
    for sys in model.software_systems.as_deref().unwrap_or(&[]) {
        elements.push(ElementEntry {
            id: sys.id.clone(),
            name: sys.name.clone(),
            description: sys.description.clone(),
            kind: "softwareSystem",
            tags: split_tags(&sys.tags),
            group: sys.group.clone(),
            technology: None,
            status: sys.status.map(|s| status_str(s).to_string()),
            properties: props(&sys.properties),
            perspectives: persp_names(&sys.perspectives),
            parent_id: None,
            ancestors: vec![],
            ancestor_names: vec![],
            introduced: sys.introduced.clone(),
            retired: sys.retired.clone(),
            ports: port_pairs(&sys.ports),
        });
        for r in sys.relationships.as_deref().unwrap_or(&[]) {
            push_rel(r);
        }

        for cont in sys.containers.as_deref().unwrap_or(&[]) {
            elements.push(ElementEntry {
                id: cont.id.clone(),
                name: cont.name.clone(),
                description: cont.description.clone(),
                kind: "container",
                tags: split_tags(&cont.tags),
                group: cont.group.clone(),
                technology: cont.technology.clone(),
                status: cont.status.map(|s| status_str(s).to_string()),
                properties: props(&cont.properties),
                perspectives: persp_names(&cont.perspectives),
                parent_id: Some(sys.id.clone()),
                ancestors: vec![sys.id.clone()],
                ancestor_names: vec![sys.name.clone()],
                introduced: cont.introduced.clone(),
                retired: cont.retired.clone(),
                ports: port_pairs(&cont.ports),
            });
            for r in cont.relationships.as_deref().unwrap_or(&[]) {
                push_rel(r);
            }

            for comp in cont.components.as_deref().unwrap_or(&[]) {
                elements.push(ElementEntry {
                    id: comp.id.clone(),
                    name: comp.name.clone(),
                    description: comp.description.clone(),
                    kind: "component",
                    tags: split_tags(&comp.tags),
                    group: comp.group.clone(),
                    technology: comp.technology.clone(),
                    status: comp.status.map(|s| status_str(s).to_string()),
                    properties: props(&comp.properties),
                    perspectives: persp_names(&comp.perspectives),
                    parent_id: Some(cont.id.clone()),
                    ancestors: vec![cont.id.clone(), sys.id.clone()],
                    ancestor_names: vec![cont.name.clone(), sys.name.clone()],
                    introduced: comp.introduced.clone(),
                    retired: comp.retired.clone(),
                    ports: port_pairs(&comp.ports),
                });
                for r in comp.relationships.as_deref().unwrap_or(&[]) {
                    push_rel(r);
                }
            }
        }
    }

    // --- Custom elements ---
    for c in model.custom_elements.as_deref().unwrap_or(&[]) {
        elements.push(ElementEntry {
            id: c.id.clone(),
            name: c.name.clone(),
            description: c.description.clone(),
            kind: "custom",
            tags: split_tags(&c.tags),
            group: c.group.clone(),
            technology: None,
            status: c.status.map(|s| status_str(s).to_string()),
            properties: props(&c.properties),
            perspectives: persp_names(&c.perspectives),
            parent_id: None,
            ancestors: vec![],
            ancestor_names: vec![],
            introduced: c.introduced.clone(),
            retired: c.retired.clone(),
            ports: port_pairs(&c.ports),
        });
        for r in c.relationships.as_deref().unwrap_or(&[]) {
            push_rel(r);
        }
    }

    // Build look-up maps.
    let mut by_id = HashMap::new();
    let mut by_name = HashMap::new();
    for (i, e) in elements.iter().enumerate() {
        by_id.insert(e.id.clone(), i);
        by_name.insert(e.name.to_lowercase(), i);
    }

    // Adjacency. Relationships whose source or destination is not a model
    // element (deployment instances, for example) simply contribute nothing.
    let mut outgoing = vec![Vec::new(); elements.len()];
    let mut incoming = vec![Vec::new(); elements.len()];
    for (ri, r) in relationships.iter().enumerate() {
        if let Some(&i) = by_id.get(&r.source_id) {
            outgoing[i].push(ri);
        }
        if let Some(&i) = by_id.get(&r.dest_id) {
            incoming[i].push(ri);
        }
    }

    let mut children = vec![Vec::new(); elements.len()];
    for (i, e) in elements.iter().enumerate() {
        if let Some(parent) = e.parent_id.as_ref().and_then(|p| by_id.get(p)) {
            children[*parent].push(i);
        }
    }

    let views = build_views(workspace);
    let mut appears_in = vec![Vec::new(); elements.len()];
    for (vi, v) in views.iter().enumerate() {
        for element_id in &v.element_ids {
            if let Some(&i) = by_id.get(element_id) {
                if !appears_in[i].contains(&vi) {
                    appears_in[i].push(vi);
                }
            }
        }
    }

    Index {
        elements,
        relationships,
        views,
        by_id,
        by_name,
        outgoing,
        incoming,
        children,
        appears_in,
    }
}

/// Collect every view, in the order the view set stores them.
fn build_views(workspace: &Workspace) -> Vec<ViewEntry> {
    let views = &workspace.views;
    let mut out: Vec<ViewEntry> = Vec::new();

    let mut add = |key: &Option<String>,
                   title: &Option<String>,
                   description: &Option<String>,
                   elements: &Option<Vec<structurizr_model::ElementView>>,
                   kind: &'static str| {
        let Some(key) = key else { return };
        out.push(ViewEntry {
            key: key.clone(),
            name: title.clone().unwrap_or_else(|| key.clone()),
            kind,
            description: description.clone(),
            element_ids: elements
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|e| e.id.clone())
                .collect(),
        });
    };

    for v in views.system_landscape_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "system landscape");
    }
    for v in views.system_context_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "system context");
    }
    for v in views.container_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "container");
    }
    for v in views.component_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "component");
    }
    for v in views.dynamic_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "dynamic");
    }
    for v in views.deployment_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "deployment");
    }
    for v in views.custom_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "custom");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_model::{
        Container, ContainerView, ElementView, Model, Relationship, SoftwareSystem,
        SystemContextView, Workspace,
    };

    fn workspace() -> Workspace {
        let mut ws = Workspace {
            name: "T".to_string(),
            ..Default::default()
        };
        ws.model = Model {
            software_systems: Some(vec![
                SoftwareSystem {
                    id: "1".into(),
                    name: "Shop".into(),
                    containers: Some(vec![
                        Container {
                            id: "2".into(),
                            name: "Web".into(),
                            relationships: Some(vec![Relationship {
                                id: "10".into(),
                                source_id: "2".into(),
                                destination_id: "3".into(),
                                description: Some("Calls".into()),
                                technology: Some("HTTPS".into()),
                                ..Default::default()
                            }]),
                            ..Default::default()
                        },
                        Container {
                            id: "3".into(),
                            name: "API".into(),
                            ..Default::default()
                        },
                    ]),
                    ..Default::default()
                },
                SoftwareSystem {
                    id: "4".into(),
                    name: "Mainframe".into(),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        ws.views.system_context_views = Some(vec![SystemContextView {
            key: Some("ctx".into()),
            software_system_id: "1".into(),
            element_views: Some(vec![
                ElementView { id: "1".into(), ..Default::default() },
                ElementView { id: "4".into(), ..Default::default() },
            ]),
            ..Default::default()
        }]);
        ws.views.container_views = Some(vec![ContainerView {
            key: Some("containers".into()),
            software_system_id: "1".into(),
            element_views: Some(vec![
                ElementView { id: "2".into(), ..Default::default() },
                ElementView { id: "3".into(), ..Default::default() },
            ]),
            ..Default::default()
        }]);
        ws
    }

    #[test]
    fn adjacency_is_directional() {
        let idx = build_index(&workspace());

        let out: Vec<&str> = idx.outgoing("2").iter().map(|r| r.id.as_str()).collect();
        assert_eq!(out, vec!["10"]);
        assert!(idx.incoming("2").is_empty());

        let inc: Vec<&str> = idx.incoming("3").iter().map(|r| r.id.as_str()).collect();
        assert_eq!(inc, vec!["10"]);

        assert_eq!(idx.degree("2"), (0, 1));
        assert_eq!(idx.degree("3"), (1, 0));
        assert_eq!(idx.degree("4"), (0, 0));
    }

    #[test]
    fn relationship_description_and_technology_are_indexed() {
        let idx = build_index(&workspace());
        let r = idx.outgoing("2")[0];
        assert_eq!(r.description.as_deref(), Some("Calls"));
        assert_eq!(r.technology.as_deref(), Some("HTTPS"));
    }

    #[test]
    fn children_resolve_one_level_down() {
        let idx = build_index(&workspace());
        let names: Vec<&str> = idx.children("1").iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Web", "API"]);
        assert!(idx.children("2").is_empty());
    }

    #[test]
    fn view_membership_is_literal_not_inherited() {
        let idx = build_index(&workspace());

        let keys: Vec<&str> = idx.views_for("1").iter().map(|v| v.key.as_str()).collect();
        assert_eq!(keys, vec!["ctx"]);

        // The system owns the containers the container view draws, but the
        // view does not list the system itself, so it is not reported here.
        let keys: Vec<&str> = idx.views_for("2").iter().map(|v| v.key.as_str()).collect();
        assert_eq!(keys, vec!["containers"]);
    }

    #[test]
    fn unknown_ids_are_empty_rather_than_panicking() {
        let idx = build_index(&workspace());
        assert!(idx.outgoing("nope").is_empty());
        assert!(idx.views_for("nope").is_empty());
        assert_eq!(idx.degree("nope"), (0, 0));
        assert!(idx.element("nope").is_none());
    }

    #[test]
    fn repeated_builds_agree_element_for_element() {
        let ws = workspace();
        let a = build_index(&ws);
        let b = build_index(&ws);

        let ids = |i: &Index| -> Vec<String> { i.elements.iter().map(|e| e.id.clone()).collect() };
        assert_eq!(ids(&a), ids(&b));

        let adj = |i: &Index| -> Vec<String> {
            i.elements
                .iter()
                .flat_map(|e| i.outgoing(&e.id).into_iter().map(|r| r.id.clone()))
                .collect()
        };
        assert_eq!(adj(&a), adj(&b));
    }
}
