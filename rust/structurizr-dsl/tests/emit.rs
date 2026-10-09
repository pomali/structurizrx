//! Round-trip tests for `structurizr_dsl::emit`: parse a fixture, emit DSL
//! from the resulting model, re-parse that DSL, and check the two models are
//! equivalent (and that emitting the re-parsed model again is byte-for-byte
//! identical — idempotence).

use std::collections::{HashMap, HashSet};

use structurizr_dsl::{parse_file_detailed, parse_str};
use structurizr_model::*;

// ─── Path helpers (canonical "ancestor/…/name" paths, computed identically
// for both the original and the re-parsed workspace so they compare equal
// regardless of the (arbitrary, id-based) order elements were declared in). ──

fn build_paths(ws: &Workspace) -> HashMap<String, String> {
    let mut paths = HashMap::new();
    for p in ws.model.people.iter().flatten() {
        paths.insert(p.id.clone(), p.name.clone());
    }
    for ss in ws.model.software_systems.iter().flatten() {
        paths.insert(ss.id.clone(), ss.name.clone());
        for c in ss.containers.iter().flatten() {
            let cpath = format!("{}/{}", ss.name, c.name);
            paths.insert(c.id.clone(), cpath.clone());
            for comp in c.components.iter().flatten() {
                paths.insert(comp.id.clone(), format!("{}/{}", cpath, comp.name));
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        paths.insert(ce.id.clone(), ce.name.clone());
    }
    for node in ws.model.deployment_nodes.iter().flatten() {
        let env = node.environment.clone().unwrap_or_default();
        build_deployment_paths(node, &env, &mut paths.clone(), &mut paths);
    }
    paths
}

fn build_deployment_paths(
    node: &DeploymentNode,
    prefix: &str,
    element_paths: &mut HashMap<String, String>,
    paths: &mut HashMap<String, String>,
) {
    let node_path = format!("{}/{}", prefix, node.name);
    paths.insert(node.id.clone(), node_path.clone());
    for ci in node.container_instances.iter().flatten() {
        let target = element_paths
            .get(&ci.container_id)
            .cloned()
            .unwrap_or_else(|| ci.container_id.clone());
        paths.insert(ci.id.clone(), format!("{}/{}(instance)", node_path, target));
    }
    for ssi in node.software_system_instances.iter().flatten() {
        let target = element_paths
            .get(&ssi.software_system_id)
            .cloned()
            .unwrap_or_else(|| ssi.software_system_id.clone());
        paths.insert(
            ssi.id.clone(),
            format!("{}/{}(instance)", node_path, target),
        );
    }
    for inf in node.infrastructure_nodes.iter().flatten() {
        paths.insert(inf.id.clone(), format!("{}/{}", node_path, inf.name));
    }
    for child in node.children.iter().flatten() {
        build_deployment_paths(child, &node_path, element_paths, paths);
    }
}

fn sorted_tags(tags: &Option<String>) -> Vec<String> {
    let mut v: Vec<String> = tags
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    v.sort();
    v
}

type ElementTuple = (&'static str, String, String, String, Vec<String>);

fn element_tuples(ws: &Workspace) -> HashSet<ElementTuple> {
    let mut out = HashSet::new();
    for p in ws.model.people.iter().flatten() {
        out.insert((
            "person",
            p.name.clone(),
            p.description.clone().unwrap_or_default(),
            String::new(),
            sorted_tags(&p.tags),
        ));
    }
    for ss in ws.model.software_systems.iter().flatten() {
        out.insert((
            "softwareSystem",
            ss.name.clone(),
            ss.description.clone().unwrap_or_default(),
            String::new(),
            sorted_tags(&ss.tags),
        ));
        for c in ss.containers.iter().flatten() {
            out.insert((
                "container",
                c.name.clone(),
                c.description.clone().unwrap_or_default(),
                c.technology.clone().unwrap_or_default(),
                sorted_tags(&c.tags),
            ));
            for comp in c.components.iter().flatten() {
                out.insert((
                    "component",
                    comp.name.clone(),
                    comp.description.clone().unwrap_or_default(),
                    comp.technology.clone().unwrap_or_default(),
                    sorted_tags(&comp.tags),
                ));
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        out.insert((
            "element",
            ce.name.clone(),
            ce.description.clone().unwrap_or_default(),
            String::new(),
            sorted_tags(&ce.tags),
        ));
    }
    out
}

type RelTuple = (String, String, String, String, String, String);

fn relationship_tuples(ws: &Workspace, paths: &HashMap<String, String>) -> HashSet<RelTuple> {
    let mut out = HashSet::new();
    let mut all: Vec<&Relationship> = Vec::new();
    for p in ws.model.people.iter().flatten() {
        all.extend(p.relationships.iter().flatten());
    }
    for ss in ws.model.software_systems.iter().flatten() {
        all.extend(ss.relationships.iter().flatten());
        for c in ss.containers.iter().flatten() {
            all.extend(c.relationships.iter().flatten());
            for comp in c.components.iter().flatten() {
                all.extend(comp.relationships.iter().flatten());
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        all.extend(ce.relationships.iter().flatten());
    }
    for node in ws.model.deployment_nodes.iter().flatten() {
        collect_deployment_rels(node, &mut all);
    }
    for r in all {
        if r.linked_relationship_id.is_some() {
            continue;
        }
        let src = paths
            .get(&r.source_id)
            .cloned()
            .unwrap_or_else(|| r.source_id.clone());
        let dst = paths
            .get(&r.destination_id)
            .cloned()
            .unwrap_or_else(|| r.destination_id.clone());
        let kind = r.kind.map(|k| format!("{:?}", k)).unwrap_or_default();
        let status = r.status.map(|s| format!("{:?}", s)).unwrap_or_default();
        out.insert((
            src,
            dst,
            r.description.clone().unwrap_or_default(),
            r.technology.clone().unwrap_or_default(),
            kind,
            status,
        ));
    }
    out
}

fn collect_deployment_rels<'a>(node: &'a DeploymentNode, out: &mut Vec<&'a Relationship>) {
    out.extend(node.relationships.iter().flatten());
    for ci in node.container_instances.iter().flatten() {
        out.extend(ci.relationships.iter().flatten());
    }
    for ssi in node.software_system_instances.iter().flatten() {
        out.extend(ssi.relationships.iter().flatten());
    }
    for inf in node.infrastructure_nodes.iter().flatten() {
        out.extend(inf.relationships.iter().flatten());
    }
    for child in node.children.iter().flatten() {
        collect_deployment_rels(child, out);
    }
}

type PortTuple = (String, String, String, String);

fn port_tuples(ws: &Workspace, paths: &HashMap<String, String>) -> HashSet<PortTuple> {
    let mut out = HashSet::new();
    let mut push = |elem_id: &str, ports: &Option<Vec<Port>>| {
        let path = paths
            .get(elem_id)
            .cloned()
            .unwrap_or_else(|| elem_id.to_string());
        for port in ports.iter().flatten() {
            out.insert((
                path.clone(),
                port.name.clone(),
                port.protocol.clone().unwrap_or_default(),
                port.direction
                    .map(|d| format!("{:?}", d))
                    .unwrap_or_default(),
            ));
        }
    };
    for p in ws.model.people.iter().flatten() {
        push(&p.id, &p.ports);
    }
    for ss in ws.model.software_systems.iter().flatten() {
        push(&ss.id, &ss.ports);
        for c in ss.containers.iter().flatten() {
            push(&c.id, &c.ports);
            for comp in c.components.iter().flatten() {
                push(&comp.id, &comp.ports);
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        push(&ce.id, &ce.ports);
    }
    out
}

fn milestone_tuples(ws: &Workspace) -> HashSet<(String, String, String)> {
    ws.milestones
        .iter()
        .flatten()
        .map(|m| {
            (
                m.name.clone(),
                m.date.clone().unwrap_or_default(),
                m.description.clone().unwrap_or_default(),
            )
        })
        .collect()
}

fn auto_view_tuples(ws: &Workspace, paths: &HashMap<String, String>) -> HashSet<String> {
    ws.views
        .auto_views
        .iter()
        .flatten()
        .map(|s| {
            let resolve = |t: &Option<String>| {
                t.as_ref()
                    .map(|v| paths.get(v).cloned().unwrap_or_else(|| v.clone()))
            };
            format!(
                "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                s.generator,
                resolve(&s.target),
                resolve(&s.target2),
                s.expression,
                s.depth,
                s.direction,
                s.split_by,
                s.asof
            )
        })
        .collect()
}

fn view_key_counts(ws: &Workspace) -> HashMap<&'static str, HashSet<Option<String>>> {
    let mut m: HashMap<&'static str, HashSet<Option<String>>> = HashMap::new();
    m.insert(
        "systemLandscape",
        ws.views
            .system_landscape_views
            .iter()
            .flatten()
            .map(|v| v.key.clone())
            .collect(),
    );
    m.insert(
        "systemContext",
        ws.views
            .system_context_views
            .iter()
            .flatten()
            .map(|v| v.key.clone())
            .collect(),
    );
    m.insert(
        "container",
        ws.views
            .container_views
            .iter()
            .flatten()
            .map(|v| v.key.clone())
            .collect(),
    );
    m.insert(
        "component",
        ws.views
            .component_views
            .iter()
            .flatten()
            .map(|v| v.key.clone())
            .collect(),
    );
    m.insert(
        "dynamic",
        ws.views
            .dynamic_views
            .iter()
            .flatten()
            .map(|v| v.key.clone())
            .collect(),
    );
    m.insert(
        "deployment",
        ws.views
            .deployment_views
            .iter()
            .flatten()
            .map(|v| v.key.clone())
            .collect(),
    );
    m.insert(
        "filtered",
        ws.views
            .filtered_views
            .iter()
            .flatten()
            .map(|v| v.key.clone())
            .collect(),
    );
    m
}

fn style_tuples(ws: &Workspace) -> (HashSet<String>, HashSet<String>) {
    let elements = ws
        .views
        .configuration
        .as_ref()
        .and_then(|c| c.styles.as_ref())
        .and_then(|s| s.elements.as_ref())
        .into_iter()
        .flatten()
        .map(|e| {
            format!(
                "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                e.tag,
                e.shape,
                e.background,
                e.color,
                e.stroke,
                e.font_size,
                e.border,
                e.opacity,
                e.width
            )
        })
        .collect();
    let rels = ws
        .views
        .configuration
        .as_ref()
        .and_then(|c| c.styles.as_ref())
        .and_then(|s| s.relationships.as_ref())
        .into_iter()
        .flatten()
        .map(|r| {
            format!(
                "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                r.tag,
                r.thickness,
                r.color,
                r.font_size,
                r.line_style,
                r.routing,
                r.opacity,
                r.dashed
            )
        })
        .collect();
    (elements, rels)
}

/// Parse `path`, emit it, re-parse the emitted text, and assert the two
/// models are equivalent by the criteria above. Also asserts idempotence:
/// emitting the re-parsed workspace again gives byte-identical text.
fn assert_round_trip(path: &str) {
    let parsed = parse_file_detailed(path).unwrap_or_else(|e| panic!("parse {}: {}", path, e));
    let emitted = structurizr_dsl::emit_with_identifiers(&parsed.workspace, &parsed.identifiers);
    let reparsed = parse_str(&emitted).unwrap_or_else(|e| {
        panic!(
            "re-parse of emitted DSL for {} failed: {}\n\n--- emitted ---\n{}",
            path, e, emitted
        )
    });

    assert_eq!(
        parsed.workspace.name, reparsed.name,
        "name mismatch for {}",
        path
    );
    assert_eq!(
        parsed.workspace.description, reparsed.description,
        "description mismatch for {}",
        path
    );

    assert_eq!(
        element_tuples(&parsed.workspace),
        element_tuples(&reparsed),
        "elements mismatch for {}",
        path
    );

    let paths_a = build_paths(&parsed.workspace);
    let paths_b = build_paths(&reparsed);
    assert_eq!(
        relationship_tuples(&parsed.workspace, &paths_a),
        relationship_tuples(&reparsed, &paths_b),
        "relationships mismatch for {}",
        path
    );
    assert_eq!(
        port_tuples(&parsed.workspace, &paths_a),
        port_tuples(&reparsed, &paths_b),
        "ports mismatch for {}",
        path
    );
    assert_eq!(
        milestone_tuples(&parsed.workspace),
        milestone_tuples(&reparsed),
        "milestones mismatch for {}",
        path
    );
    assert_eq!(
        auto_view_tuples(&parsed.workspace, &paths_a),
        auto_view_tuples(&reparsed, &paths_b),
        "auto view specs mismatch for {}",
        path
    );

    let counts_a = view_key_counts(&parsed.workspace);
    let counts_b = view_key_counts(&reparsed);
    for kind in [
        "systemLandscape",
        "systemContext",
        "container",
        "component",
        "dynamic",
        "deployment",
        "filtered",
    ] {
        assert_eq!(
            counts_a[kind].len(),
            counts_b[kind].len(),
            "{} view count mismatch for {}: {:?} vs {:?}",
            kind,
            path,
            counts_a[kind],
            counts_b[kind]
        );
    }

    assert_eq!(
        style_tuples(&parsed.workspace),
        style_tuples(&reparsed),
        "styles mismatch for {}",
        path
    );

    // Idempotence: emit(reparsed) is a fixed point.
    let emitted2 = structurizr_dsl::emit(&reparsed);
    let reparsed2 = parse_str(&emitted2)
        .unwrap_or_else(|e| panic!("re-parse of second emit for {} failed: {}", path, e));
    let emitted3 = structurizr_dsl::emit(&reparsed2);
    assert_eq!(emitted2, emitted3, "emit is not idempotent for {}", path);
}

#[test]
fn round_trip_site_examples() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../site/examples");
    let mut any = false;
    for entry in std::fs::read_dir(dir).expect("read site/examples") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "dsl") {
            any = true;
            assert_round_trip(path.to_str().unwrap());
        }
    }
    assert!(any, "expected at least one .dsl fixture under {}", dir);
}

#[test]
fn round_trip_big_bank_plc() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/big-bank-plc/workspace.dsl"
    );
    assert_round_trip(path);
}

#[test]
fn round_trip_getting_started() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/getting-started/workspace.dsl"
    );
    assert_round_trip(path);
}

#[test]
fn round_trip_microservices() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/microservices/workspace.dsl"
    );
    assert_round_trip(path);
}

/// The sketch example (no `workspace { }` wrapper at all) must round-trip
/// into a *full* workspace with the placeholder systems declared: sketch
/// mode isn't itself representable by `emit` (there's no DSL syntax that
/// says "and treat unknown identifiers as auto-vivified"), so the emitted
/// text is a normal workspace whose placeholder elements are ordinary
/// `softwareSystem`s tagged `Placeholder`.
#[test]
fn sketch_promotes_to_full_workspace() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../site/examples/sketch.dsl"
    );
    let parsed = parse_file_detailed(path).expect("parse sketch");
    let emitted = structurizr_dsl::emit_with_identifiers(&parsed.workspace, &parsed.identifiers);

    assert!(
        emitted.trim_start().starts_with("workspace"),
        "emitted sketch should open with `workspace {{`"
    );
    assert!(
        emitted.contains("softwareSystem"),
        "emitted sketch should declare its placeholder systems"
    );
    assert!(
        emitted.contains("Placeholder"),
        "placeholder tag should survive promotion to a full workspace"
    );

    let reparsed = parse_str(&emitted).expect("re-parse promoted sketch");
    assert_eq!(element_tuples(&parsed.workspace), element_tuples(&reparsed));
}

/// A hand-written workspace exercising ports, relationship kinds, statuses,
/// milestones, perspectives, groups, named relationships, `?` uncertainty
/// markers, `auto` views with options, styles and a deployment environment.
#[test]
fn round_trip_kitchen_sink() {
    let dsl = r#"
workspace "Kitchen Sink" "Exercises the whole DSL surface" {
    milestones {
        mvp "2026-08" "First cut"
        billingSplit "2026-12" "Billing extracted"
    }
    perspectives {
        security "STRIDE-reviewed"
        performance
    }

    model {
        customer = person "Customer" "Shops online" "Vip" ?
        shop = softwareSystem "Shop" "Online store" {
            group "Storefront" {
                web = container "Web App" "Storefront" "TypeScript"
            }
            api = container "API" "Handles requests" "Rust" {
                status implemented
                introduced mvp
                port rest "Customer REST API" {
                    protocol "HTTPS/JSON"
                    direction in
                }
            }
            db = container "Database" "Stores data" "PostgreSQL" "Database"

            web -> api.rest "calls" "HTTPS"
            api -> db "maybe caches" ?
        }

        customer -> web "shops on"
        // Named relationships are only registered by the parser at model
        // level (the identifier is silently dropped when written directly
        // inside a softwareSystem/container body).
        orderFlow = api -> db "reads and writes" {
            kind sync
            status specified
            perspective "reliability" "at-least-once"
        }

        deploymentEnvironment "Live" {
            deploymentNode "Server" "Prod box" "Ubuntu" {
                webInstance = containerInstance web
                apiInstance = containerInstance api
                lb = infrastructureNode "LB" "Load balancer" "nginx"
            }
        }
        lb -> apiInstance "routes to"
    }

    views {
        auto
        auto focus api { depth 2 direction in }

        systemContext shop "ctx" "Shop Context" {
            include customer shop
            autoLayout lr
        }

        styles {
            element "Database" {
                shape Cylinder
                background #1168bd
                color #ffffff
            }
            relationship "Relationship" {
                thickness 2
                color #707070
            }
        }
        theme default
    }
}
"#;
    let (ws, ids) = structurizr_dsl::parse_str_with_identifiers(dsl).expect("parse kitchen sink");
    let emitted = structurizr_dsl::emit_with_identifiers(&ws, &ids);
    let reparsed = parse_str(&emitted).unwrap_or_else(|e| {
        panic!(
            "re-parse of kitchen-sink emit failed: {}\n\n--- emitted ---\n{}",
            e, emitted
        )
    });

    assert_eq!(element_tuples(&ws), element_tuples(&reparsed));
    let paths_a = build_paths(&ws);
    let paths_b = build_paths(&reparsed);
    assert_eq!(
        relationship_tuples(&ws, &paths_a),
        relationship_tuples(&reparsed, &paths_b)
    );
    assert_eq!(port_tuples(&ws, &paths_a), port_tuples(&reparsed, &paths_b));
    assert_eq!(milestone_tuples(&ws), milestone_tuples(&reparsed));
    assert_eq!(
        auto_view_tuples(&ws, &paths_a),
        auto_view_tuples(&reparsed, &paths_b)
    );
    assert_eq!(style_tuples(&ws), style_tuples(&reparsed));

    // Uncertain markers on both the person and the extra relationship must
    // round-trip: the customer keeps an (unstripped-by-us) `Uncertain` tag,
    // and the second api->db relationship does too.
    let customer = reparsed
        .model
        .people
        .as_ref()
        .unwrap()
        .iter()
        .find(|p| p.name == "Customer")
        .unwrap();
    assert!(customer
        .tags
        .as_deref()
        .unwrap_or_default()
        .contains("Uncertain"));

    // Named relationship (`orderFlow = api -> db`) keeps its name in the
    // first emission (which has the source register). A relationship's name
    // is a fact about the identifier register, not the model itself, so it
    // only survives a *second* round-trip when that register is carried
    // along too (`parse_str_with_identifiers` + `emit_with_identifiers`),
    // not through the plain `parse_str`/`emit` used for the plumbing
    // equivalence checks above.
    assert!(
        emitted.contains("orderFlow ="),
        "named relationship should be named in the first emission"
    );
    let (reparsed_with_ids, ids2) =
        structurizr_dsl::parse_str_with_identifiers(&emitted).expect("re-parse with ids");
    let emitted2 = structurizr_dsl::emit_with_identifiers(&reparsed_with_ids, &ids2);
    assert!(
        emitted2.contains("orderFlow ="),
        "named relationship should still be named after round-tripping"
    );

    // Idempotence.
    let reparsed2 = parse_str(&emitted2).expect("re-parse second emit");
    let emitted3 = structurizr_dsl::emit(&reparsed2);
    let reparsed3 = parse_str(&emitted3).expect("re-parse third emit");
    let emitted4 = structurizr_dsl::emit(&reparsed3);
    assert_eq!(emitted3, emitted4);
}
