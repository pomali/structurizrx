//! Tests for the whole-workspace graph projection (`structurizr_query::graph`).

use structurizr_query::graph;

const DSL: &str = r#"
workspace "Universe" "Everything" {
    model {
        user = person "User"
        shop = softwareSystem "Shop" {
            api = container "API" "Handles requests" "Rust"
            db = container "Database" {
                schema = component "Schema"
            }
            api -> db "reads"
        }
        user -> shop "uses"

        prod = deploymentEnvironment "Production" {
            server = deploymentNode "Server" {
                apiInstance = containerInstance api
            }
        }
    }
    views {
        systemContext shop "Context" {
            include *
        }
        container shop "Containers" {
            include *
        }
    }
}
"#;

fn g() -> graph::Graph {
    let ws = structurizr_dsl::parse_str(DSL).expect("DSL parses");
    graph::graph(&ws)
}

fn node<'a>(g: &'a graph::Graph, name: &str) -> &'a graph::GraphNode {
    g.nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node named {name}; have {:?}", names(g)))
}

fn names(g: &graph::Graph) -> Vec<&str> {
    g.nodes.iter().map(|n| n.name.as_str()).collect()
}

fn links_of<'a>(g: &'a graph::Graph, class: &str) -> Vec<&'a graph::GraphLink> {
    g.links.iter().filter(|l| l.class == class).collect()
}

#[test]
fn every_model_element_becomes_a_node() {
    let g = g();
    for expected in ["User", "Shop", "API", "Database", "Schema", "Server"] {
        let n = node(&g, expected);
        assert!(n.id.starts_with("e:"), "{expected} should be an element node");
    }
    assert_eq!(node(&g, "API").kind, "container");
    assert_eq!(node(&g, "API").technology.as_deref(), Some("Rust"));
    assert_eq!(node(&g, "Schema").kind, "component");
    assert_eq!(node(&g, "Server").kind, "deploymentNode");
    assert_eq!(node(&g, "Server").environment.as_deref(), Some("Production"));
}

#[test]
fn containment_links_follow_the_element_hierarchy() {
    let g = g();
    let pairs: Vec<(&str, &str)> = links_of(&g, "containment")
        .iter()
        .map(|l| (l.source_id.as_str(), l.target_id.as_str()))
        .collect();
    let shop = node(&g, "Shop").id.as_str();
    let api = node(&g, "API").id.as_str();
    let db = node(&g, "Database").id.as_str();
    let schema = node(&g, "Schema").id.as_str();
    assert!(pairs.contains(&(shop, api)), "got {pairs:?}");
    assert!(pairs.contains(&(shop, db)), "got {pairs:?}");
    assert!(pairs.contains(&(db, schema)), "got {pairs:?}");
    assert_eq!(node(&g, "Schema").parent_id.as_deref(), Some(db));
}

#[test]
fn modelled_relationships_become_relationship_links() {
    let g = g();
    let rels = links_of(&g, "relationship");
    let descs: Vec<Option<&str>> = rels.iter().map(|l| l.description.as_deref()).collect();
    assert!(descs.contains(&Some("uses")), "got {descs:?}");
    assert!(descs.contains(&Some("reads")), "got {descs:?}");
    assert!(rels.iter().all(|l| l.id.starts_with("r:")));
}

#[test]
fn container_instances_link_to_the_container_they_instantiate() {
    let g = g();
    let api = node(&g, "API").id.clone();
    let instance = links_of(&g, "instance");
    assert_eq!(instance.len(), 1);
    assert_eq!(instance[0].target_id, api);
    // The instance is named after the container it instantiates.
    let inst = node(&g, "API [1]");
    assert_eq!(inst.kind, "containerInstance");
    assert_eq!(instance[0].source_id, inst.id);
}

#[test]
fn views_become_nodes_linked_to_the_elements_they_show() {
    let g = g();
    let views: Vec<&graph::GraphNode> = g.nodes.iter().filter(|n| n.kind == "view").collect();
    assert_eq!(views.len(), 2, "one node per view: {:?}", names(&g));
    let membership = links_of(&g, "membership");
    assert!(!membership.is_empty());
    assert!(membership.iter().all(|l| l.source_id.starts_with("v:")));
    assert!(membership.iter().all(|l| l.target_id.starts_with("e:")));
}

#[test]
fn links_never_dangle() {
    let g = g();
    let ids: std::collections::HashSet<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
    for l in &g.links {
        assert!(ids.contains(l.source_id.as_str()), "dangling source in {l:?}");
        assert!(ids.contains(l.target_id.as_str()), "dangling target in {l:?}");
    }
}

#[test]
fn empty_workspace_yields_an_empty_graph() {
    let ws = structurizr_model::Workspace::default();
    let g = graph::graph(&ws);
    assert!(g.nodes.is_empty());
    assert!(g.links.is_empty());
}
