//! Integration tests for the structurizr-query selector engine.
//!
//! The test workspace contains:
//!   Elements: user1 (person), user2 (person), shop (softwareSystem),
//!             billing (softwareSystem), api (container), db (container),
//!             router (component), bus (custom)
//!
//!   Relationships:
//!     r_user1_shop  : user1  → shop    (sync)
//!     r_user2_billing: user2 → billing (sync)
//!     r_shop_billing : shop  → billing (async, tag:critical, status:draft,
//!                                       perspective:performance)
//!     r_api_db       : api   → db      (sync, tag:critical)
//!
//!   Parent hierarchy:
//!     api, db   ← shop
//!     router    ← api ← shop

use std::collections::{BTreeSet, HashMap};

use structurizr_model::{
    Component, Container, CustomElement, Model, Perspective, Person, Relationship,
    RelationshipKind, SoftwareSystem, Status, Workspace,
};
use structurizr_query::{query, QueryError};

// ---------------------------------------------------------------------------
// Workspace fixture
// ---------------------------------------------------------------------------

fn ws() -> Workspace {
    let mut workspace = Workspace::default();
    workspace.name = "TestWorkspace".to_string();

    // ---- people ----

    let user1 = Person {
        id: "user1".to_string(),
        name: "End User".to_string(),
        tags: Some("Person,External".to_string()),
        group: Some("ExternalGroup".to_string()),
        status: Some(Status::Implemented),
        perspectives: Some(vec![Perspective {
            name: "security".to_string(),
            ..Default::default()
        }]),
        relationships: Some(vec![Relationship {
            id: "r_user1_shop".to_string(),
            source_id: "user1".to_string(),
            destination_id: "shop".to_string(),
            kind: Some(RelationshipKind::Sync),
            tags: Some("UserFlow".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };

    let user2 = Person {
        id: "user2".to_string(),
        name: "Admin User".to_string(),
        tags: Some("Person,Internal".to_string()),
        group: Some("InternalGroup".to_string()),
        status: Some(Status::Implemented),
        relationships: Some(vec![Relationship {
            id: "r_user2_billing".to_string(),
            source_id: "user2".to_string(),
            destination_id: "billing".to_string(),
            kind: Some(RelationshipKind::Sync),
            ..Default::default()
        }]),
        ..Default::default()
    };

    // ---- shop system: api + db containers, router component ----

    let router = Component {
        id: "router".to_string(),
        name: "Router Component".to_string(),
        technology: Some("Rust".to_string()),
        tags: Some("Component,Router".to_string()),
        status: Some(Status::Draft),
        properties: Some({
            let mut m = HashMap::new();
            m.insert("owner".to_string(), "shop-team".to_string());
            m
        }),
        ..Default::default()
    };

    let api = Container {
        id: "api".to_string(),
        name: "API Gateway".to_string(),
        technology: Some("Rust/Axum".to_string()),
        tags: Some("Container,API".to_string()),
        status: Some(Status::Implemented),
        perspectives: Some(vec![Perspective {
            name: "security".to_string(),
            ..Default::default()
        }]),
        properties: Some({
            let mut m = HashMap::new();
            m.insert("owner".to_string(), "shop-team".to_string());
            m
        }),
        components: Some(vec![router]),
        relationships: Some(vec![Relationship {
            id: "r_api_db".to_string(),
            source_id: "api".to_string(),
            destination_id: "db".to_string(),
            kind: Some(RelationshipKind::Sync),
            tags: Some("critical".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };

    let db = Container {
        id: "db".to_string(),
        name: "Database".to_string(),
        technology: Some("PostgreSQL".to_string()),
        tags: Some("Container,Database".to_string()),
        status: Some(Status::Implemented),
        properties: Some({
            let mut m = HashMap::new();
            m.insert("owner".to_string(), "platform-team".to_string());
            m
        }),
        ..Default::default()
    };

    let shop = SoftwareSystem {
        id: "shop".to_string(),
        name: "Shop System".to_string(),
        tags: Some("SoftwareSystem,Backend".to_string()),
        group: Some("domain".to_string()),
        status: Some(Status::Implemented),
        properties: Some({
            let mut m = HashMap::new();
            m.insert("layer".to_string(), "domain".to_string());
            m.insert("owner".to_string(), "shop-team".to_string());
            m.insert("kind".to_string(), "webapplication".to_string());
            m
        }),
        perspectives: Some(vec![
            Perspective { name: "performance".to_string(), ..Default::default() },
            Perspective { name: "security".to_string(), ..Default::default() },
        ]),
        containers: Some(vec![api, db]),
        relationships: Some(vec![Relationship {
            id: "r_shop_billing".to_string(),
            source_id: "shop".to_string(),
            destination_id: "billing".to_string(),
            kind: Some(RelationshipKind::Async),
            tags: Some("critical".to_string()),
            status: Some(Status::Draft),
            perspectives: Some(vec![Perspective {
                name: "performance".to_string(),
                ..Default::default()
            }]),
            ..Default::default()
        }]),
        ..Default::default()
    };

    // ---- billing system ----

    let billing = SoftwareSystem {
        id: "billing".to_string(),
        name: "Billing System".to_string(),
        tags: Some("SoftwareSystem,Finance".to_string()),
        status: Some(Status::Idea),
        properties: Some({
            let mut m = HashMap::new();
            m.insert("layer".to_string(), "finance".to_string());
            m.insert("owner".to_string(), "billing-team".to_string());
            m
        }),
        perspectives: Some(vec![Perspective {
            name: "security".to_string(),
            ..Default::default()
        }]),
        ..Default::default()
    };

    // ---- custom element: event bus ----

    let bus = CustomElement {
        id: "bus".to_string(),
        name: "Event Bus".to_string(),
        tags: Some("Custom,Connector".to_string()),
        properties: Some({
            let mut m = HashMap::new();
            m.insert("kind".to_string(), "queue".to_string());
            m
        }),
        ..Default::default()
    };

    workspace.model = Model {
        people: Some(vec![user1, user2]),
        software_systems: Some(vec![shop, billing]),
        custom_elements: Some(vec![bus]),
        ..Default::default()
    };

    workspace
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ids(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// Tests: element comparisons by path
// ---------------------------------------------------------------------------

#[test]
fn element_kind_person() {
    let sel = query("element.kind==person", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1", "user2"]));
    assert!(sel.relationships.is_empty());
}

#[test]
fn element_kind_software_system() {
    let sel = query("element.kind==softwareSystem", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["shop", "billing"]));
}

#[test]
fn element_kind_software_system_case_insensitive() {
    // "softwaresystem" (all lower) should still match structural kind
    let sel = query("element.kind==softwaresystem", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["shop", "billing"]));
}

#[test]
fn element_kind_container() {
    let sel = query("element.kind==container", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["api", "db"]));
}

#[test]
fn element_kind_component() {
    let sel = query("element.kind==component", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["router"]));
}

#[test]
fn element_kind_custom() {
    let sel = query("element.kind==custom", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["bus"]));
}

#[test]
fn element_kind_via_property_alias() {
    // "webapplication" is not a structural kind; it is set via `kind` property on shop
    let sel = query("element.kind==webapplication", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["shop"]));
}

#[test]
fn element_kind_via_property_alias_queue() {
    let sel = query("element.kind==queue", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["bus"]));
}

#[test]
fn element_kind_ne() {
    let all = ids(&["user1", "user2", "shop", "billing", "api", "db", "router", "bus"]);
    let containers = ids(&["api", "db"]);
    let expected: BTreeSet<String> =
        all.difference(&containers).cloned().collect();

    let sel = query("element.kind!=container", &ws()).unwrap();
    assert_eq!(sel.elements, expected);
    // relationships are not affected by element comparisons
    assert!(sel.relationships.is_empty());
}

#[test]
fn element_tag() {
    let sel = query("element.tag==External", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1"]));
}

#[test]
fn element_tag_case_insensitive() {
    let sel = query("element.tag==external", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1"]));
}

#[test]
fn element_tag_database() {
    let sel = query("element.tag==Database", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["db"]));
}

#[test]
fn element_status_implemented() {
    let sel = query("element.status==implemented", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1", "user2", "shop", "api", "db"]));
}

#[test]
fn element_status_idea() {
    let sel = query("element.status==idea", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["billing"]));
}

#[test]
fn element_status_draft() {
    let sel = query("element.status==draft", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["router"]));
}

#[test]
fn element_layer_via_group() {
    // shop has group="domain" which is treated as its layer
    let sel = query("element.layer==domain", &ws()).unwrap();
    assert!(sel.elements.contains("shop"));
}

#[test]
fn element_layer_via_property() {
    // billing has layer property = "finance"
    let sel = query("element.layer==finance", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["billing"]));
}

#[test]
fn element_perspective_security() {
    let sel = query("element.perspective==security", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1", "shop", "api", "billing"]));
}

#[test]
fn element_perspective_performance() {
    let sel = query("element.perspective==performance", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["shop"]));
}

#[test]
fn element_parent_by_id() {
    // api and db are children of shop (by id "shop")
    let sel = query("element.parent==shop", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["api", "db"]));
}

#[test]
fn element_parent_by_name() {
    let sel = query("element.parent==\"Shop System\"", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["api", "db"]));
}

#[test]
fn element_parent_container() {
    // router's direct parent is api
    let sel = query("element.parent==api", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["router"]));
}

#[test]
fn element_parent_transitive() {
    // parent^ matches any ancestor — router has both api and shop as ancestors
    let sel_api = query("element.parent^==api", &ws()).unwrap();
    assert!(sel_api.elements.contains("router"));

    let sel_shop = query("element.parent^==shop", &ws()).unwrap();
    // api, db, and router are all transitive children of shop
    assert_eq!(sel_shop.elements, ids(&["api", "db", "router"]));
}

#[test]
fn element_technology() {
    let sel = query("element.technology==PostgreSQL", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["db"]));
}

#[test]
fn element_technology_case_insensitive() {
    let sel = query("element.technology==postgresql", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["db"]));
}

#[test]
fn element_name() {
    let sel = query("element.name==Database", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["db"]));
}

#[test]
fn element_property_owner() {
    let sel = query("element.property.owner==shop-team", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["shop", "api", "router"]));
}

#[test]
fn element_property_owner_platform() {
    let sel = query("element.property.owner==platform-team", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["db"]));
}

#[test]
fn element_property_layer() {
    let sel = query("element.property.layer==domain", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["shop"]));
}

// ---------------------------------------------------------------------------
// Tests: relationship comparisons by path
// ---------------------------------------------------------------------------

#[test]
fn relationship_kind_async() {
    let sel = query("relationship.kind==async", &ws()).unwrap();
    assert!(sel.elements.is_empty());
    assert_eq!(sel.relationships, ids(&["r_shop_billing"]));
}

#[test]
fn relationship_kind_sync() {
    let sel = query("relationship.kind==sync", &ws()).unwrap();
    assert_eq!(
        sel.relationships,
        ids(&["r_user1_shop", "r_user2_billing", "r_api_db"])
    );
}

#[test]
fn relationship_tag_critical() {
    let sel = query("relationship.tag==critical", &ws()).unwrap();
    assert_eq!(sel.relationships, ids(&["r_api_db", "r_shop_billing"]));
}

#[test]
fn relationship_status_draft() {
    let sel = query("relationship.status==draft", &ws()).unwrap();
    assert_eq!(sel.relationships, ids(&["r_shop_billing"]));
}

#[test]
fn relationship_perspective() {
    let sel = query("relationship.perspective==performance", &ws()).unwrap();
    assert_eq!(sel.relationships, ids(&["r_shop_billing"]));
}

#[test]
fn relationship_kind_ne() {
    let all_rels =
        ids(&["r_user1_shop", "r_user2_billing", "r_shop_billing", "r_api_db"]);
    let async_rels = ids(&["r_shop_billing"]);
    let expected: BTreeSet<String> = all_rels.difference(&async_rels).cloned().collect();

    let sel = query("relationship.kind!=async", &ws()).unwrap();
    assert_eq!(sel.relationships, expected);
    assert!(sel.elements.is_empty());
}

// ---------------------------------------------------------------------------
// Tests: Star
// ---------------------------------------------------------------------------

#[test]
fn star_all() {
    let all_elems =
        ids(&["user1", "user2", "shop", "billing", "api", "db", "router", "bus"]);
    let all_rels =
        ids(&["r_user1_shop", "r_user2_billing", "r_shop_billing", "r_api_db"]);

    let sel = query("*", &ws()).unwrap();
    assert_eq!(sel.elements, all_elems);
    assert_eq!(sel.relationships, all_rels);
}

// ---------------------------------------------------------------------------
// Tests: Neighborhood
// ---------------------------------------------------------------------------

#[test]
fn neighborhood_depth_zero() {
    let sel = query("->shop->0", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["shop"]));
    assert!(sel.relationships.is_empty());
}

#[test]
fn neighborhood_depth_one_default() {
    // ->shop->  (default depth 1)
    let sel_arrow = query("->shop->", &ws()).unwrap();
    let sel_one = query("->shop->1", &ws()).unwrap();
    assert_eq!(sel_arrow, sel_one);

    // shop connects to: user1 (via r_user1_shop) and billing (via r_shop_billing)
    assert_eq!(sel_arrow.elements, ids(&["shop", "user1", "billing"]));
    assert_eq!(
        sel_arrow.relationships,
        ids(&["r_user1_shop", "r_shop_billing"])
    );
}

#[test]
fn neighborhood_depth_two() {
    // At depth 2 we also reach user2 (billing → user2 via r_user2_billing)
    let sel = query("->shop->2", &ws()).unwrap();
    assert_eq!(
        sel.elements,
        ids(&["shop", "user1", "billing", "user2"])
    );
    assert_eq!(
        sel.relationships,
        ids(&["r_user1_shop", "r_shop_billing", "r_user2_billing"])
    );
}

#[test]
fn neighborhood_by_name() {
    // Target by element name (case-insensitive)
    let sel_id = query("->shop->0", &ws()).unwrap();
    let sel_name = query("->\"Shop System\"->0", &ws()).unwrap();
    assert_eq!(sel_id.elements, sel_name.elements);
}

#[test]
fn neighborhood_unknown_target() {
    let err = query("->nonexistent->", &ws()).unwrap_err();
    assert!(matches!(err, QueryError::UnknownTarget(ref t) if t == "nonexistent"));
}

// ---------------------------------------------------------------------------
// Tests: Boolean operators
// ---------------------------------------------------------------------------

#[test]
fn and_intersects_element_sets() {
    // containers that are also implemented
    let sel = query("element.kind==container && element.status==implemented", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["api", "db"]));
}

#[test]
fn and_empty_intersection() {
    // containers that have status==idea → empty
    let sel = query("element.kind==container && element.status==idea", &ws()).unwrap();
    assert!(sel.elements.is_empty());
}

#[test]
fn or_unions_element_sets() {
    let sel = query("element.kind==person || element.kind==container", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1", "user2", "api", "db"]));
}

#[test]
fn or_unions_element_and_relationship_sets() {
    let sel = query("element.kind==container || relationship.kind==async", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["api", "db"]));
    assert_eq!(sel.relationships, ids(&["r_shop_billing"]));
}

#[test]
fn not_complements_both_universes() {
    // !element.kind==container → all elements except containers, PLUS all relationships
    let all_rels =
        ids(&["r_user1_shop", "r_user2_billing", "r_shop_billing", "r_api_db"]);
    let non_containers =
        ids(&["user1", "user2", "shop", "billing", "router", "bus"]);

    let sel = query("!(element.kind==container)", &ws()).unwrap();
    assert_eq!(sel.elements, non_containers);
    assert_eq!(sel.relationships, all_rels);
}

#[test]
fn not_star_is_empty() {
    let sel = query("!*", &ws()).unwrap();
    assert!(sel.elements.is_empty());
    assert!(sel.relationships.is_empty());
}

#[test]
fn precedence_or_binds_looser_than_and() {
    // a || b && c  ==  a || (b && c)
    // element.kind==person || element.kind==container && element.status==implemented
    //   = {user1,user2} || ({api,db} ∩ {user1,user2,shop,api,db})
    //   = {user1,user2} || {api,db}
    //   = {user1,user2,api,db}
    let sel = query(
        "element.kind==person || element.kind==container && element.status==implemented",
        &ws(),
    )
    .unwrap();
    assert_eq!(sel.elements, ids(&["user1", "user2", "api", "db"]));
}

#[test]
fn parentheses_change_grouping() {
    // (element.kind==person || element.kind==container) && element.status==implemented
    //   = {user1,user2,api,db} ∩ {user1,user2,shop,api,db}
    //   = {user1,user2,api,db}
    let sel = query(
        "(element.kind==person || element.kind==container) && element.status==implemented",
        &ws(),
    )
    .unwrap();
    assert_eq!(sel.elements, ids(&["user1", "user2", "api", "db"]));

    // A different grouping that yields a different result
    // (element.kind==softwareSystem || element.kind==container) && element.status==idea
    //   = {shop,billing,api,db} ∩ {billing}
    //   = {billing}
    let sel2 = query(
        "(element.kind==softwareSystem || element.kind==container) && element.status==idea",
        &ws(),
    )
    .unwrap();
    assert_eq!(sel2.elements, ids(&["billing"]));

    // Without parens: element.kind==softwareSystem || (element.kind==container && element.status==idea)
    //   = {shop,billing} || {}
    //   = {shop,billing}
    let sel3 = query(
        "element.kind==softwareSystem || element.kind==container && element.status==idea",
        &ws(),
    )
    .unwrap();
    assert_eq!(sel3.elements, ids(&["shop", "billing"]));

    // sel2 ≠ sel3 — parentheses matter
    assert_ne!(sel2.elements, sel3.elements);
}

// ---------------------------------------------------------------------------
// Tests: Quoted values with spaces
// ---------------------------------------------------------------------------

#[test]
fn quoted_value_with_spaces() {
    let sel = query("element.name==\"End User\"", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1"]));
}

#[test]
fn quoted_value_case_insensitive() {
    let sel = query("element.name==\"end user\"", &ws()).unwrap();
    assert_eq!(sel.elements, ids(&["user1"]));
}

// ---------------------------------------------------------------------------
// Tests: Case-insensitivity of keywords and paths
// ---------------------------------------------------------------------------

#[test]
fn keywords_case_insensitive() {
    let sel_lower = query("element.kind==person", &ws()).unwrap();
    let sel_upper = query("ELEMENT.KIND==PERSON", &ws()).unwrap();
    assert_eq!(sel_lower.elements, sel_upper.elements);
}

#[test]
fn relationship_keyword_case_insensitive() {
    let a = query("relationship.kind==async", &ws()).unwrap();
    let b = query("RELATIONSHIP.KIND==async", &ws()).unwrap();
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Tests: Selection ordering determinism
// ---------------------------------------------------------------------------

#[test]
fn selection_is_deterministic() {
    let s1 = query("*", &ws()).unwrap();
    let s2 = query("*", &ws()).unwrap();
    assert_eq!(s1, s2);

    // BTreeSet guarantees lexicographic order.
    let elem_vec: Vec<_> = s1.elements.iter().collect();
    let mut sorted = elem_vec.clone();
    sorted.sort();
    assert_eq!(elem_vec, sorted);
}

// ---------------------------------------------------------------------------
// Tests: Error cases — parse errors name valid paths
// ---------------------------------------------------------------------------

#[test]
fn unknown_element_path_parse_error() {
    let err = query("element.foo==bar", &ws()).unwrap_err();
    match err {
        QueryError::UnknownPath { kind, path, valid } => {
            assert_eq!(kind, "element");
            assert!(path.contains("foo"), "path should mention 'foo'");
            // valid list should name recognised paths
            assert!(valid.contains("tag"), "valid list should mention 'tag'");
            assert!(valid.contains("kind"), "valid list should mention 'kind'");
            assert!(valid.contains("status"), "valid list should mention 'status'");
        }
        other => panic!("expected UnknownPath, got {other:?}"),
    }
}

#[test]
fn unknown_relationship_path_parse_error() {
    let err = query("relationship.layer==x", &ws()).unwrap_err();
    match err {
        QueryError::UnknownPath { kind, path, valid } => {
            assert_eq!(kind, "relationship");
            assert!(path.contains("layer"));
            assert!(valid.contains("kind"));
        }
        other => panic!("expected UnknownPath, got {other:?}"),
    }
}

#[test]
fn unknown_neighborhood_target_error() {
    let err = query("->xyz->", &ws()).unwrap_err();
    match err {
        QueryError::UnknownTarget(t) => assert_eq!(t, "xyz"),
        other => panic!("expected UnknownTarget, got {other:?}"),
    }
}

#[test]
fn parse_error_on_garbage_input() {
    let err = query("???", &ws()).unwrap_err();
    assert!(matches!(err, QueryError::Parse { .. }));
}
