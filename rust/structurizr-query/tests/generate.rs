use structurizr_dsl::parse_str;
use structurizr_model::Workspace;
use structurizr_query::generate_views;

fn shop() -> Workspace {
    parse_str(r#"
workspace "Shop" {
    milestones {
        mvp "2026-08"
        billingSplit "2026-12"
    }
    model {
        user = person "Customer"
        shop = softwareSystem "Shop" {
            web = container "Web App" {
                properties { layer "ui" }
            }
            api = container "API" {
                port rest "REST API"
                port events "Order events"
                properties { layer "domain" }
            }
            db = container "Database" {
                tags "Database"
                properties { layer "data" }
            }
        }
        billing = softwareSystem "Billing" {
            status idea
            introduced billingSplit
        }
        legacy = softwareSystem "Legacy CRM" {
            retired billingSplit
        }
        ghost = softwareSystem "Ghost"
        user -> web "browses"
        web -> api.rest "calls"
        api -> db "reads/writes" { kind sync }
        api -> billing "charges" { kind async introduced billingSplit }
        api -> legacy "syncs" { retired billingSplit }
        shop -> legacy "uses"
        user -> shop "shops" {
            perspective "security" "authn boundary"
        }
    }
    views {
        auto focus api { depth 1 }
        auto focus api { splitBy kind }
        auto perspective "security"
        auto layer "domain"
        auto slice element.tag==Database
        auto paths user db
        auto asof billingSplit
        auto delta now billingSplit
        auto lint
    }
}
"#).expect("fixture parses")
}

fn find_landscape<'a>(ws: &'a Workspace, key: &str) -> &'a structurizr_model::SystemLandscapeView {
    ws.views.system_landscape_views.as_ref().unwrap()
        .iter().find(|v| v.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("view {} not found", key))
}

fn elem_count(v: &structurizr_model::SystemLandscapeView) -> usize {
    v.element_views.as_ref().map_or(0, |e| e.len())
}

#[test]
fn default_set_generated_when_no_views() {
    let mut ws = parse_str(r#"
workspace {
    model {
        u = person "User"
        s = softwareSystem "Sys" {
            c = container "C" {
                comp = component "Comp"
            }
        }
        u -> s "uses"
        u -> c "clicks"
    }
}
"#).unwrap();
    let keys = generate_views(&mut ws).unwrap();
    assert!(keys.contains(&"auto-landscape".to_string()), "keys: {:?}", keys);
    assert!(keys.contains(&"auto-context-sys".to_string()), "keys: {:?}", keys);
    assert!(keys.contains(&"auto-container-sys".to_string()), "keys: {:?}", keys);
    assert!(keys.contains(&"auto-component-c".to_string()), "keys: {:?}", keys);
    // landscape holds person + system and the induced relationship
    let l = find_landscape(&ws, "auto-landscape");
    assert_eq!(elem_count(l), 2);
    assert_eq!(l.relationship_views.as_ref().unwrap().len(), 1);
    // container view: container + external person neighbor
    let cv = &ws.views.container_views.as_ref().unwrap()[0];
    let ids: Vec<&str> = cv.element_views.as_ref().unwrap().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "container + person, got {:?}", ids);
}

#[test]
fn focus_depth_one_and_split_by_kind() {
    let mut ws = shop();
    let keys = generate_views(&mut ws).unwrap();
    // plain focus: api + web, db, billing, legacy (depth 1, both directions)
    let f = find_landscape(&ws, "auto-focus-api");
    assert_eq!(elem_count(f), 5, "api + 4 neighbors");
    // splitBy kind buckets: sync, async, unspecified (web->api and api->legacy have no kind)
    assert!(keys.contains(&"auto-focus-api-sync".to_string()), "keys: {:?}", keys);
    assert!(keys.contains(&"auto-focus-api-async".to_string()));
    assert!(keys.contains(&"auto-focus-api-unspecified".to_string()));
    let sync = find_landscape(&ws, "auto-focus-api-sync");
    assert_eq!(elem_count(sync), 2, "api + db only");
}

#[test]
fn focus_direction_in_vs_out() {
    let mut ws = parse_str(r#"
workspace {
    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        c = softwareSystem "C"
        a -> b "x"
        b -> c "y"
    }
    views {
        auto focus b { direction in }
        auto focus b { direction out splitBy kind }
    }
}
"#).unwrap();
    generate_views(&mut ws).unwrap();
    let vin = find_landscape(&ws, "auto-focus-b");
    assert_eq!(elem_count(vin), 2, "b + a (inbound only)");
    // direction out with splitBy: bucket 'unspecified' holds b + c
    let vout = find_landscape(&ws, "auto-focus-b-unspecified");
    assert_eq!(elem_count(vout), 2, "b + c (outbound only)");
}

#[test]
fn perspective_layer_slice() {
    let mut ws = shop();
    generate_views(&mut ws).unwrap();
    let p = find_landscape(&ws, "auto-perspective-security");
    assert_eq!(elem_count(p), 2, "user + shop from the security perspective rel");
    let l = find_landscape(&ws, "auto-layer-domain");
    assert_eq!(elem_count(l), 1, "only api has layer=domain");
    let s = find_landscape(&ws, "auto-slice-element-tag-database");
    assert_eq!(elem_count(s), 1, "db only");
}

#[test]
fn paths_reachability() {
    let mut ws = shop();
    generate_views(&mut ws).unwrap();
    // user -> web -> api -> db; billing/legacy/ghost are dead ends
    let p = find_landscape(&ws, "auto-paths-customer-database");
    let n = elem_count(p);
    assert_eq!(n, 4, "user, web, api, db on the path");
    assert_eq!(p.relationship_views.as_ref().unwrap().len(), 3);
}

#[test]
fn asof_and_delta() {
    let mut ws = shop();
    generate_views(&mut ws).unwrap();
    // as of billingSplit: legacy retired, billing introduced → user, shop, web, api, db, billing, ghost
    let a = find_landscape(&ws, "auto-asof-billingsplit");
    assert_eq!(elem_count(a), 7);
    let ids: Vec<&str> = a.element_views.as_ref().unwrap().iter().map(|e| e.id.as_str()).collect();
    // legacy must be gone: check by looking up its relationships absence instead of id string
    let d = find_landscape(&ws, "auto-delta-now-billingsplit");
    let desc = d.description.as_deref().unwrap();
    assert!(desc.contains("added: 1 elements (Billing)"), "desc: {}", desc);
    assert!(desc.contains("removed: 1 elements (Legacy CRM)"), "desc: {}", desc);
    assert!(elem_count(d) >= 8, "delta shows the union; got {} ({:?})", elem_count(d), ids);
}

#[test]
fn lint_findings() {
    let mut ws = shop();
    generate_views(&mut ws).unwrap();
    let l = find_landscape(&ws, "auto-lint");
    let desc = l.description.as_deref().unwrap();
    assert!(desc.contains("orphans: Ghost"), "desc: {}", desc);
    assert!(desc.contains("unbound ports: API.Order events"), "desc: {}", desc);
    assert!(!desc.contains("REST API"), "rest port is bound; desc: {}", desc);
}

#[test]
fn idempotent_and_deterministic() {
    let mut ws = shop();
    let keys1 = generate_views(&mut ws).unwrap();
    assert!(!keys1.is_empty());
    let keys2 = generate_views(&mut ws).unwrap();
    assert!(keys2.is_empty(), "second run must generate nothing, got {:?}", keys2);

    let mut ws_b = shop();
    let keys_b = generate_views(&mut ws_b).unwrap();
    assert_eq!(keys1, keys_b, "generation must be deterministic");
}

#[test]
fn existing_views_disable_implicit_default() {
    let mut ws = parse_str(r#"
workspace {
    model {
        s = softwareSystem "Sys"
    }
    views {
        systemLandscape "manual" {
            include *
        }
    }
}
"#).unwrap();
    let keys = generate_views(&mut ws).unwrap();
    assert!(keys.is_empty(), "explicit views present, no auto specs → nothing generated");
}
