//! Whole-workspace graph projection: every element, view and documentation
//! artefact as one node set, every relationship, containment and membership as
//! one link set.
//!
//! Unlike a C4 view (which shows one scope at one level of abstraction), this
//! flattens the entire workspace into a single "universe" graph for the
//! force-directed browser in `structurizr-web`. It is a projection only —
//! nothing here changes the workspace.

use serde::Serialize;
use structurizr_model::{
    Component, Container, DeploymentNode, Status, Workspace,
};

/// Prefix marking a node built from a model element.
const P_ELEMENT: &str = "e:";
/// Prefix marking a node built from a view.
const P_VIEW: &str = "v:";
/// Prefix marking a node built from an ADR.
const P_DECISION: &str = "d:";
/// Prefix marking a node built from a documentation section.
const P_SECTION: &str = "s:";

/// One node of the universe graph.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    /// Namespaced, graph-unique id (`e:`, `v:`, `d:` or `s:` prefix).
    pub id: String,
    /// The underlying element id / view key / decision id, without the prefix.
    pub ref_id: String,
    pub name: String,
    /// `person`, `softwareSystem`, `container`, `component`, `custom`,
    /// `deploymentNode`, `infrastructureNode`, `containerInstance`,
    /// `softwareSystemInstance`, `view`, `decision` or `section`.
    pub kind: &'static str,
    /// Id of the node that structurally contains this one, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Deployment environment, for deployment-side nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
}

/// One link of the universe graph.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphLink {
    /// Graph-unique link id. Model relationships keep `r:<relationship id>`;
    /// derived links get a synthetic id.
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    /// What the link means: `relationship` (a modelled relationship),
    /// `containment` (parent → child), `instance` (deployed instance → the
    /// element it instantiates), `membership` (view → element it shows) or
    /// `documents` (ADR/section → the element it is attached to).
    pub class: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technology: Option<String>,
    /// Relationship kind (`sync`, `async`, …), for `relationship` links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// The whole workspace as one graph.
#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub workspace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_description: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

fn status_str(s: Status) -> String {
    format!("{:?}", s).to_lowercase()
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

fn elem(id: &str) -> String {
    format!("{P_ELEMENT}{id}")
}

/// Build the universe graph for `ws`.
///
/// Node and link order is deterministic: model elements in declaration order
/// (people → software systems → deployment nodes → custom elements), then
/// views, then documentation.
pub fn graph(ws: &Workspace) -> Graph {
    let mut g = Graph {
        workspace_name: ws.name.clone(),
        workspace_description: ws.description.clone(),
        ..Default::default()
    };

    let model = &ws.model;

    // --- People ---
    for p in model.people.as_deref().unwrap_or(&[]) {
        g.nodes.push(GraphNode {
            id: elem(&p.id),
            ref_id: p.id.clone(),
            name: p.name.clone(),
            kind: "person",
            parent_id: None,
            description: p.description.clone(),
            technology: None,
            tags: split_tags(&p.tags),
            group: p.group.clone(),
            status: p.status.map(status_str),
            url: p.url.clone(),
            environment: None,
        });
        push_relationships(&mut g, p.relationships.as_deref());
    }

    // --- Software systems → containers → components ---
    for sys in model.software_systems.as_deref().unwrap_or(&[]) {
        g.nodes.push(GraphNode {
            id: elem(&sys.id),
            ref_id: sys.id.clone(),
            name: sys.name.clone(),
            kind: "softwareSystem",
            parent_id: None,
            description: sys.description.clone(),
            technology: None,
            tags: split_tags(&sys.tags),
            group: sys.group.clone(),
            status: sys.status.map(status_str),
            url: sys.url.clone(),
            environment: None,
        });
        push_relationships(&mut g, sys.relationships.as_deref());

        for cont in sys.containers.as_deref().unwrap_or(&[]) {
            push_container(&mut g, cont, &sys.id);
        }
    }

    // --- Deployment nodes (recursive) ---
    // Instances carry no name of their own in the schema, so they are named
    // after the element they instantiate once every node exists.
    let mut pending_instances: Vec<(usize, String, i32)> = Vec::new();
    for dn in model.deployment_nodes.as_deref().unwrap_or(&[]) {
        push_deployment_node(&mut g, dn, None, &mut pending_instances);
    }

    // --- Custom elements ---
    for c in model.custom_elements.as_deref().unwrap_or(&[]) {
        g.nodes.push(GraphNode {
            id: elem(&c.id),
            ref_id: c.id.clone(),
            name: c.name.clone(),
            kind: "custom",
            parent_id: None,
            description: c.description.clone(),
            technology: None,
            tags: split_tags(&c.tags),
            group: c.group.clone(),
            status: c.status.map(status_str),
            url: c.url.clone(),
            environment: None,
        });
        push_relationships(&mut g, c.relationships.as_deref());
    }

    {
        let names: std::collections::HashMap<String, String> =
            g.nodes.iter().map(|n| (n.id.clone(), n.name.clone())).collect();
        for (idx, of_id, number) in &pending_instances {
            if let Some(name) = names.get(of_id) {
                g.nodes[*idx].name = format!("{name} [{number}]");
            }
        }
    }

    push_views(&mut g, ws);
    push_documentation(&mut g, ws);

    // Drop links whose endpoints are not in the graph (for example a
    // relationship to an element the workspace never declared), so the client
    // never has to resolve a dangling id.
    let ids: std::collections::HashSet<String> = g.nodes.iter().map(|n| n.id.clone()).collect();
    g.links.retain(|l| ids.contains(&l.source_id) && ids.contains(&l.target_id));

    g
}

fn push_container(g: &mut Graph, cont: &Container, system_id: &str) {
    g.nodes.push(GraphNode {
        id: elem(&cont.id),
        ref_id: cont.id.clone(),
        name: cont.name.clone(),
        kind: "container",
        parent_id: Some(elem(system_id)),
        description: cont.description.clone(),
        technology: cont.technology.clone(),
        tags: split_tags(&cont.tags),
        group: cont.group.clone(),
        status: cont.status.map(status_str),
        url: cont.url.clone(),
        environment: None,
    });
    push_containment(g, &elem(system_id), &elem(&cont.id));
    push_relationships(g, cont.relationships.as_deref());

    for comp in cont.components.as_deref().unwrap_or(&[]) {
        push_component(g, comp, &cont.id);
    }
}

fn push_component(g: &mut Graph, comp: &Component, container_id: &str) {
    g.nodes.push(GraphNode {
        id: elem(&comp.id),
        ref_id: comp.id.clone(),
        name: comp.name.clone(),
        kind: "component",
        parent_id: Some(elem(container_id)),
        description: comp.description.clone(),
        technology: comp.technology.clone(),
        tags: split_tags(&comp.tags),
        group: comp.group.clone(),
        status: comp.status.map(status_str),
        url: comp.url.clone(),
        environment: None,
    });
    push_containment(g, &elem(container_id), &elem(&comp.id));
    push_relationships(g, comp.relationships.as_deref());
}

fn push_deployment_node(
    g: &mut Graph,
    dn: &DeploymentNode,
    parent: Option<&str>,
    pending_instances: &mut Vec<(usize, String, i32)>,
) {
    let id = elem(&dn.id);
    g.nodes.push(GraphNode {
        id: id.clone(),
        ref_id: dn.id.clone(),
        name: dn.name.clone(),
        kind: "deploymentNode",
        parent_id: parent.map(str::to_string),
        description: dn.description.clone(),
        technology: dn.technology.clone(),
        tags: split_tags(&dn.tags),
        group: None,
        status: None,
        url: dn.url.clone(),
        environment: dn.environment.clone(),
    });
    if let Some(p) = parent {
        push_containment(g, p, &id);
    }
    push_relationships(g, dn.relationships.as_deref());

    for infra in dn.infrastructure_nodes.as_deref().unwrap_or(&[]) {
        let infra_id = elem(&infra.id);
        g.nodes.push(GraphNode {
            id: infra_id.clone(),
            ref_id: infra.id.clone(),
            name: infra.name.clone(),
            kind: "infrastructureNode",
            parent_id: Some(id.clone()),
            description: infra.description.clone(),
            technology: infra.technology.clone(),
            tags: split_tags(&infra.tags),
            group: None,
            status: None,
            url: infra.url.clone(),
            environment: dn.environment.clone(),
        });
        push_containment(g, &id, &infra_id);
        push_relationships(g, infra.relationships.as_deref());
    }

    for ci in dn.container_instances.as_deref().unwrap_or(&[]) {
        let inst_id = elem(&ci.id);
        pending_instances.push((g.nodes.len(), elem(&ci.container_id), ci.instance_id.unwrap_or(1)));
        g.nodes.push(GraphNode {
            id: inst_id.clone(),
            ref_id: ci.id.clone(),
            name: String::new(), // filled in from the instantiated container
            kind: "containerInstance",
            parent_id: Some(id.clone()),
            description: None,
            technology: None,
            tags: split_tags(&ci.tags),
            group: None,
            status: None,
            url: ci.url.clone(),
            environment: ci.environment.clone().or_else(|| dn.environment.clone()),
        });
        push_containment(g, &id, &inst_id);
        push_instance_of(g, &inst_id, &elem(&ci.container_id));
        push_relationships(g, ci.relationships.as_deref());
    }

    for si in dn.software_system_instances.as_deref().unwrap_or(&[]) {
        let inst_id = elem(&si.id);
        pending_instances.push((
            g.nodes.len(),
            elem(&si.software_system_id),
            si.instance_id.unwrap_or(1),
        ));
        g.nodes.push(GraphNode {
            id: inst_id.clone(),
            ref_id: si.id.clone(),
            name: String::new(), // filled in from the instantiated system
            kind: "softwareSystemInstance",
            parent_id: Some(id.clone()),
            description: None,
            technology: None,
            tags: split_tags(&si.tags),
            group: None,
            status: None,
            url: si.url.clone(),
            environment: si.environment.clone().or_else(|| dn.environment.clone()),
        });
        push_containment(g, &id, &inst_id);
        push_instance_of(g, &inst_id, &elem(&si.software_system_id));
        push_relationships(g, si.relationships.as_deref());
    }

    for child in dn.children.as_deref().unwrap_or(&[]) {
        push_deployment_node(g, child, Some(&id), pending_instances);
    }
}

fn push_relationships(g: &mut Graph, rels: Option<&[structurizr_model::Relationship]>) {
    for r in rels.unwrap_or(&[]) {
        g.links.push(GraphLink {
            id: format!("r:{}", r.id),
            source_id: elem(&r.source_id),
            target_id: elem(&r.destination_id),
            class: "relationship",
            description: r.description.clone(),
            technology: r.technology.clone(),
            kind: r.kind.map(|k| format!("{:?}", k).to_lowercase()),
            tags: split_tags(&r.tags),
            status: r.status.map(status_str),
        });
    }
}

fn push_containment(g: &mut Graph, parent: &str, child: &str) {
    g.links.push(GraphLink {
        id: format!("c:{parent}->{child}"),
        source_id: parent.to_string(),
        target_id: child.to_string(),
        class: "containment",
        description: None,
        technology: None,
        kind: None,
        tags: vec![],
        status: None,
    });
}

fn push_instance_of(g: &mut Graph, instance: &str, of: &str) {
    g.links.push(GraphLink {
        id: format!("i:{instance}->{of}"),
        source_id: instance.to_string(),
        target_id: of.to_string(),
        class: "instance",
        description: None,
        technology: None,
        kind: None,
        tags: vec![],
        status: None,
    });
}

/// Add one node per view, linked to every element the view shows.
fn push_views(g: &mut Graph, ws: &Workspace) {
    let views = &ws.views;

    let add = |key: &Option<String>,
                   title: &Option<String>,
                   description: &Option<String>,
                   elements: &Option<Vec<structurizr_model::ElementView>>,
                   view_kind: &'static str,
                   g: &mut Graph| {
        let Some(key) = key else { return };
        let id = format!("{P_VIEW}{key}");
        g.nodes.push(GraphNode {
            id: id.clone(),
            ref_id: key.clone(),
            name: title.clone().unwrap_or_else(|| key.clone()),
            kind: "view",
            parent_id: None,
            description: description.clone(),
            technology: Some(view_kind.to_string()),
            tags: vec![],
            group: None,
            status: None,
            url: None,
            environment: None,
        });
        for ev in elements.as_deref().unwrap_or(&[]) {
            g.links.push(GraphLink {
                id: format!("m:{key}->{}", ev.id),
                source_id: id.clone(),
                target_id: elem(&ev.id),
                class: "membership",
                description: None,
                technology: None,
                kind: None,
                tags: vec![],
                status: None,
            });
        }
    };

    for v in views.system_landscape_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "system landscape", g);
    }
    for v in views.system_context_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "system context", g);
    }
    for v in views.container_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "container", g);
    }
    for v in views.component_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "component", g);
    }
    for v in views.dynamic_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "dynamic", g);
    }
    for v in views.deployment_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "deployment", g);
    }
    for v in views.custom_views.as_deref().unwrap_or(&[]) {
        add(&v.key, &v.title, &v.description, &v.element_views, "custom", g);
    }
}

/// Add one node per ADR and per documentation section, linked to the element
/// each is attached to (if any).
fn push_documentation(g: &mut Graph, ws: &Workspace) {
    let Some(doc) = &ws.documentation else { return };

    for d in doc.decisions.as_deref().unwrap_or(&[]) {
        let id = format!("{P_DECISION}{}", d.id);
        g.nodes.push(GraphNode {
            id: id.clone(),
            ref_id: d.id.clone(),
            name: d.title.clone(),
            kind: "decision",
            parent_id: None,
            description: Some(format!("{} · {}", d.status, d.date)),
            technology: None,
            tags: vec![],
            group: None,
            status: Some(d.status.to_lowercase()),
            url: None,
            environment: None,
        });
        if let Some(eid) = &d.element_id {
            g.links.push(GraphLink {
                id: format!("doc:{}->{}", d.id, eid),
                source_id: id.clone(),
                target_id: elem(eid),
                class: "documents",
                description: None,
                technology: None,
                kind: None,
                tags: vec![],
                status: None,
            });
        }
    }

    for (i, s) in doc.sections.as_deref().unwrap_or(&[]).iter().enumerate() {
        let id = format!("{P_SECTION}{i}");
        g.nodes.push(GraphNode {
            id: id.clone(),
            ref_id: i.to_string(),
            name: s.title.clone(),
            kind: "section",
            parent_id: None,
            description: None,
            technology: None,
            tags: vec![],
            group: None,
            status: None,
            url: None,
            environment: None,
        });
        if let Some(eid) = &s.element_id {
            g.links.push(GraphLink {
                id: format!("doc:s{i}->{eid}"),
                source_id: id.clone(),
                target_id: elem(eid),
                class: "documents",
                description: None,
                technology: None,
                kind: None,
                tags: vec![],
                status: None,
            });
        }
    }
}
