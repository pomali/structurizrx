use structurizr_dsl::{parse_file, parse_str};
use std::path::PathBuf;
use structurizr_model::{RelationshipKind, Status};

fn dsl_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../original-java/structurizr-dsl/src/test/resources/dsl")
        .join(name)
}

fn examples_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../examples")
        .join(name)
}

#[test]
fn parse_getting_started() {
    let path = dsl_path("getting-started.dsl");
    let ws = parse_file(&path).expect("should parse getting-started.dsl");
    assert_eq!(ws.name, "Workspace");
    let people = ws.model.people.as_ref().expect("should have people");
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].name, "User");
    let systems = ws.model.software_systems.as_ref().expect("should have software systems");
    assert_eq!(systems.len(), 1);
    assert_eq!(systems[0].name, "Software System");
}

#[test]
fn parse_big_bank_plc() {
    let path = dsl_path("big-bank-plc.dsl");
    let ws = parse_file(&path).expect("should parse big-bank-plc.dsl");
    assert_eq!(ws.name, "Big Bank plc");
    let people = ws.model.people.as_ref().expect("should have people");
    // customer, supportStaff, backoffice
    assert!(people.len() >= 3, "expected at least 3 people, got {}", people.len());
    let systems = ws.model.software_systems.as_ref().expect("should have software systems");
    assert!(systems.len() >= 1, "expected at least 1 software system");
    // internetBankingSystem should have containers
    let ibs = systems.iter().find(|s| s.name == "Internet Banking System")
        .expect("Internet Banking System not found");
    let containers = ibs.containers.as_ref().expect("should have containers");
    assert!(containers.len() >= 4, "expected at least 4 containers, got {}", containers.len());
}

#[test]
fn parse_inline_dsl() {
    let dsl = r#"
workspace "Hello" {
    model {
        u = person "User" "A user"
        s = softwareSystem "System" "The system" {
            c = container "API" "API container" "Java"
        }
        u -> s "Uses"
    }
    views {
        systemContext s "SC" {
            include *
            autolayout
        }
        styles {
            element "Person" {
                shape Person
            }
        }
        theme default
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse inline DSL");
    assert_eq!(ws.name, "Hello");
    let people = ws.model.people.as_ref().unwrap();
    assert_eq!(people[0].name, "User");
    let systems = ws.model.software_systems.as_ref().unwrap();
    assert_eq!(systems[0].name, "System");
    let containers = systems[0].containers.as_ref().unwrap();
    assert_eq!(containers[0].name, "API");
}

#[test]
fn parse_avisi_adrs() {
    let path = examples_path("avisi/workspace.dsl");
    let ws = parse_file(&path).expect("should parse avisi workspace.dsl");

    // Should have parsed ADRs from workspace-adrs/ (4) plus element-scoped ones (6 = 1+1+3+1)
    let decisions = ws
        .documentation
        .as_ref()
        .and_then(|d| d.decisions.as_ref())
        .expect("workspace should have decisions");
    assert_eq!(decisions.len(), 10, "expected 10 total decisions");

    // The 4 workspace-level ADRs should have no element_id
    let workspace_level: Vec<_> = decisions.iter().filter(|d| d.element_id.is_none()).collect();
    assert_eq!(workspace_level.len(), 4, "expected 4 workspace-level decisions");

    // element-scoped decisions should have element_id set
    let element_level: Vec<_> = decisions.iter().filter(|d| d.element_id.is_some()).collect();
    assert_eq!(element_level.len(), 6, "expected 6 element-scoped decisions");

    // Spot-check a workspace-level decision
    let adr1 = workspace_level.iter().find(|d| d.id == "1").expect("ADR-1 not found");
    assert_eq!(adr1.title, "Record architecture decisions");
    assert_eq!(adr1.status, "Accepted");
    assert_eq!(adr1.format, "Markdown");

    // Internet Banking System should have 5 containers with API Application having 6 components
    let systems = ws.model.software_systems.as_ref().expect("no software systems");
    let ibs = systems
        .iter()
        .find(|s| s.name == "Internet Banking System")
        .expect("Internet Banking System not found");
    let containers = ibs.containers.as_ref().expect("IBS should have containers");
    assert_eq!(containers.len(), 5, "expected 5 containers in IBS");
    let api_app = containers
        .iter()
        .find(|c| c.name == "API Application")
        .expect("API Application container not found");
    let components = api_app.components.as_ref().expect("API App should have components");
    assert_eq!(components.len(), 6, "expected 6 components in API Application");
}

// ─── Phase 2a: Relationship body parsing ────────────────────────────────────

#[test]
fn relationship_body_kind_and_status() {
    let dsl = r#"
workspace {
    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        a -> b "Sends data" {
            kind async
            status implemented
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    let a = systems.iter().find(|s| s.name == "A").unwrap();
    let rels = a.relationships.as_ref().expect("A should have relationships");
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].kind, Some(RelationshipKind::Async));
    assert_eq!(rels[0].status, Some(Status::Implemented));
}

#[test]
fn relationship_body_perspective_and_introduced() {
    let dsl = r#"
workspace {
    model {
        u = person "User"
        s = softwareSystem "System"
        u -> s "Uses" {
            introduced v1
            retired v3
            perspective "Security" "Must use TLS"
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let people = ws.model.people.as_ref().unwrap();
    let rels = people[0].relationships.as_ref().expect("User should have relationships");
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].introduced.as_deref(), Some("v1"));
    assert_eq!(rels[0].retired.as_deref(), Some("v3"));
    let persp = rels[0].perspectives.as_ref().expect("should have perspectives");
    assert_eq!(persp.len(), 1);
    assert_eq!(persp[0].name, "Security");
    assert_eq!(persp[0].description.as_deref(), Some("Must use TLS"));
}

#[test]
fn relationship_body_properties_and_tags() {
    let dsl = r#"
workspace {
    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        a -> b "Calls" {
            technology "gRPC"
            tags "SyncCall" "Critical"
            properties {
                sla "99.9"
                owner "team-alpha"
            }
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    let a = systems.iter().find(|s| s.name == "A").unwrap();
    let rels = a.relationships.as_ref().expect("A should have relationships");
    let rel = &rels[0];
    assert_eq!(rel.technology.as_deref(), Some("gRPC"));
    // Tags should include "Relationship" base plus the extra tags
    let tags = rel.tags.as_deref().unwrap_or("");
    assert!(tags.contains("Relationship"), "base tag must be present");
    assert!(tags.contains("SyncCall"),     "SyncCall tag must be present");
    assert!(tags.contains("Critical"),     "Critical tag must be present");
    let props = rel.properties.as_ref().expect("should have properties");
    assert_eq!(props.get("sla").map(|s| s.as_str()), Some("99.9"));
    assert_eq!(props.get("owner").map(|s| s.as_str()), Some("team-alpha"));
}

#[test]
fn relationship_body_inside_element_block() {
    // Relationships defined inside a person/component body block should also
    // have their body parsed (parse_element_block site).
    let dsl = r#"
workspace {
    model {
        u = person "User" {
            u -> s "Uses web app" {
                kind sync
                status specified
            }
        }
        s = softwareSystem "System"
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let people = ws.model.people.as_ref().unwrap();
    let rels = people[0].relationships.as_ref().expect("User should have relationships");
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].kind, Some(RelationshipKind::Sync));
    assert_eq!(rels[0].status, Some(Status::Specified));
}

// ─── Phase 2a: Element body attributes ──────────────────────────────────────

#[test]
fn container_element_body_status_introduced_perspective() {
    let dsl = r#"
workspace {
    model {
        ss = softwareSystem "System" {
            api = container "API" "The API" "Rust" {
                status implemented
                introduced "v2.0"
                perspective "Security" "Audited monthly" "High"
            }
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    let containers = systems[0].containers.as_ref().expect("should have containers");
    let api = containers.iter().find(|c| c.name == "API").expect("API container not found");
    assert_eq!(api.status, Some(Status::Implemented));
    assert_eq!(api.introduced.as_deref(), Some("v2.0"));
    let persp = api.perspectives.as_ref().expect("should have perspectives");
    assert_eq!(persp.len(), 1);
    assert_eq!(persp[0].name, "Security");
    assert_eq!(persp[0].description.as_deref(), Some("Audited monthly"));
    assert_eq!(persp[0].value.as_deref(), Some("High"));
}

#[test]
fn person_element_body_status() {
    let dsl = r#"
workspace {
    model {
        u = person "Customer" {
            status deprecated
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let people = ws.model.people.as_ref().unwrap();
    assert_eq!(people[0].status, Some(Status::Deprecated));
}

#[test]
fn software_system_element_body_status_and_perspectives_block() {
    let dsl = r#"
workspace {
    model {
        ss = softwareSystem "System" {
            status draft
            introduced milestone-1
            perspectives {
                "Security" "Threat model in progress"
                "Performance"
            }
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    assert_eq!(systems[0].status, Some(Status::Draft));
    assert_eq!(systems[0].introduced.as_deref(), Some("milestone-1"));
    let persp = systems[0].perspectives.as_ref().expect("should have perspectives");
    assert_eq!(persp.len(), 2);
    assert_eq!(persp[0].name, "Security");
    assert_eq!(persp[0].description.as_deref(), Some("Threat model in progress"));
    assert_eq!(persp[1].name, "Performance");
}

// ─── Phase 2a: Workspace-level milestones and perspectives ──────────────────

#[test]
fn workspace_milestones_ordered() {
    let dsl = r#"
workspace {
    milestones {
        alpha "2024-03-01" "Alpha release"
        beta  "2024-06-01"
        ga
    }
    model {}
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let ms = ws.milestones.as_ref().expect("should have milestones");
    assert_eq!(ms.len(), 3, "order must be preserved");
    assert_eq!(ms[0].name, "alpha");
    assert_eq!(ms[0].date.as_deref(), Some("2024-03-01"));
    assert_eq!(ms[0].description.as_deref(), Some("Alpha release"));
    assert_eq!(ms[1].name, "beta");
    assert_eq!(ms[1].date.as_deref(), Some("2024-06-01"));
    assert!(ms[1].description.is_none());
    assert_eq!(ms[2].name, "ga");
    assert!(ms[2].date.is_none());
    assert!(ms[2].description.is_none());
}

#[test]
fn workspace_perspectives_registry() {
    let dsl = r#"
workspace {
    perspectives {
        "Security" "All security-related concerns"
        "Compliance"
    }
    model {}
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let persp = ws.perspectives.as_ref().expect("should have workspace perspectives");
    assert_eq!(persp.len(), 2);
    assert_eq!(persp[0].name, "Security");
    assert_eq!(persp[0].description.as_deref(), Some("All security-related concerns"));
    assert_eq!(persp[1].name, "Compliance");
    assert!(persp[1].description.is_none());
}

// ─── Phase 2a: Error cases ───────────────────────────────────────────────────

#[test]
fn unknown_relationship_kind_produces_error() {
    let dsl = r#"
workspace {
    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        a -> b "Sends" {
            kind teleportation
        }
    }
}
"#;
    let result = parse_str(dsl);
    assert!(result.is_err(), "unknown kind value must produce an error");
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("kind") || msg.contains("teleportation"),
        "error message should mention the bad value; got: {}", msg);
}

#[test]
fn unknown_status_value_produces_error() {
    let dsl = r#"
workspace {
    model {
        a = softwareSystem "A" {
            status unknown_status_xyz
        }
    }
}
"#;
    let result = parse_str(dsl);
    assert!(result.is_err(), "unknown status value must produce an error");
}

// ─── Regression: technology and tags in relationship body no longer skipped ──

#[test]
fn relationship_body_technology_and_tags_parsed_not_skipped() {
    // Before this feature, a `{ technology "..." tags "..." }` block after a
    // relationship would be silently skipped. Now it must be parsed.
    let dsl = r#"
workspace {
    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        a -> b "Calls" {
            technology "REST"
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    let a = systems.iter().find(|s| s.name == "A").unwrap();
    let rels = a.relationships.as_ref().expect("A should have relationships");
    // technology must be set from the body block, not silently lost
    assert_eq!(rels[0].technology.as_deref(), Some("REST"),
        "technology from relationship body must not be silently skipped");
}

// ─── Phase-2b: ports ─────────────────────────────────────────────────────────

#[test]
fn container_ports_full_and_bare() {
    let dsl = r#"
workspace {
    model {
        api = softwareSystem "Shop" {
            apiC = container "API" {
                port rest "Customer REST API" {
                    protocol "HTTPS/JSON"
                    direction in
                    description "Public API"
                    perspective "security" "rate-limited"
                }
                port events
            }
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    let container = &systems[0].containers.as_ref().unwrap()[0];
    let ports = container.ports.as_ref().expect("container should have ports");
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].name, "Customer REST API");
    assert_eq!(ports[0].protocol.as_deref(), Some("HTTPS/JSON"));
    assert_eq!(ports[0].direction, Some(structurizr_model::PortDirection::In));
    assert_eq!(ports[0].description.as_deref(), Some("Public API"));
    let persp = ports[0].perspectives.as_ref().expect("port perspective");
    assert_eq!(persp[0].name, "security");
    // bare port: name defaults to its identifier
    assert_eq!(ports[1].name, "events");
    assert!(ports[1].direction.is_none());
}

#[test]
fn relationship_to_port_sets_destination_port_id() {
    let dsl = r#"
workspace {
    model {
        shop = softwareSystem "Shop" {
            webC = container "Web App"
            apiC = container "API" {
                port rest "REST API"
            }
        }
        webC -> apiC.rest "calls"
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let shop = &ws.model.software_systems.as_ref().unwrap()[0];
    let containers = shop.containers.as_ref().unwrap();
    let web = &containers[0];
    let api = &containers[1];
    let port_id = &api.ports.as_ref().unwrap()[0].id;
    let rels = web.relationships.as_ref().expect("webC should own the relationship");
    assert_eq!(rels[0].destination_id, api.id);
    assert_eq!(rels[0].destination_port_id.as_ref(), Some(port_id));
    assert!(rels[0].source_port_id.is_none());
}

#[test]
fn relationship_from_port_sets_source_port_id() {
    let dsl = r#"
workspace {
    model {
        shop = softwareSystem "Shop" {
            apiC = container "API" {
                port events "Order events" {
                    direction out
                    protocol "Kafka"
                }
            }
        }
        billing = softwareSystem "Billing"
        apiC.events -> billing "OrderPlaced" {
            kind async
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let shop = &ws.model.software_systems.as_ref().unwrap()[0];
    let api = &shop.containers.as_ref().unwrap()[0];
    let billing = &ws.model.software_systems.as_ref().unwrap()[1];
    let port_id = &api.ports.as_ref().unwrap()[0].id;
    let rels = api.relationships.as_ref().expect("apiC should own the relationship");
    assert_eq!(rels[0].source_port_id.as_ref(), Some(port_id));
    assert_eq!(rels[0].destination_id, billing.id);
    assert_eq!(rels[0].kind, Some(RelationshipKind::Async));
}

#[test]
fn hierarchical_dotted_element_refs_still_resolve_as_elements() {
    let dsl = r#"
workspace {
    !identifiers hierarchical
    model {
        shop = softwareSystem "Shop" {
            web = container "Web App"
            api = container "API"
        }
        shop.web -> shop.api "calls"
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let shop = &ws.model.software_systems.as_ref().unwrap()[0];
    let containers = shop.containers.as_ref().unwrap();
    let web = &containers[0];
    let api = &containers[1];
    let rels = web.relationships.as_ref().expect("web should own the relationship");
    // dotted hierarchical element refs must resolve to elements, never ports
    assert_eq!(rels[0].destination_id, api.id);
    assert!(rels[0].destination_port_id.is_none());
    assert!(rels[0].source_port_id.is_none());
}

#[test]
fn ports_on_software_system_and_person() {
    let dsl = r#"
workspace {
    model {
        support = person "Support" {
            port phone "Phone line"
        }
        shop = softwareSystem "Shop" {
            port api "Public API" {
                direction inout
            }
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let person = &ws.model.people.as_ref().unwrap()[0];
    assert_eq!(person.ports.as_ref().unwrap()[0].name, "Phone line");
    let system = &ws.model.software_systems.as_ref().unwrap()[0];
    let sp = &system.ports.as_ref().unwrap()[0];
    assert_eq!(sp.name, "Public API");
    assert_eq!(sp.direction, Some(structurizr_model::PortDirection::InOut));
}

#[test]
fn unknown_port_direction_produces_error() {
    let dsl = r#"
workspace {
    model {
        shop = softwareSystem "Shop" {
            port api {
                direction sideways
            }
        }
    }
}
"#;
    let err = parse_str(dsl).expect_err("sideways is not a valid direction");
    let msg = format!("{}", err);
    assert!(msg.contains("direction") || msg.contains("sideways"),
        "error should mention the bad direction, got: {}", msg);
}

#[test]
fn unresolved_dotted_port_ref_falls_back_gracefully() {
    let dsl = r#"
workspace {
    model {
        shop = softwareSystem "Shop" {
            apiC = container "API"
        }
        billing = softwareSystem "Billing"
        billing -> apiC.nonexistent "calls"
    }
}
"#;
    // Must not panic; the relationship is still created with the raw identifier.
    let ws = parse_str(dsl).expect("should parse without panic");
    let billing = &ws.model.software_systems.as_ref().unwrap()[1];
    let rels = billing.relationships.as_ref().expect("relationship still created");
    assert!(rels[0].destination_port_id.is_none());
}

// ─── Phase-2c: sketch mode, aliases, named rels, ?, !include ────────────────

#[test]
fn bare_sketch_file_parses_and_vivifies() {
    let dsl = r#"
customer -> shop "buys things"
shop -> billing "somehow charges" ?
billing -> erp
"#;
    let ws = parse_str(dsl).expect("bare sketch should parse");
    assert_eq!(ws.name, "Sketch");
    let systems = ws.model.software_systems.as_ref().expect("placeholders created");
    assert_eq!(systems.len(), 4, "customer, shop, billing, erp");
    assert!(systems.iter().all(|s| s.tags.as_deref().unwrap_or("").contains("Placeholder")));
    // the uncertain relationship carries the Uncertain tag
    let shop = systems.iter().find(|s| s.name == "shop").unwrap();
    let rels = shop.relationships.as_ref().expect("shop -> billing");
    assert!(rels[0].tags.as_deref().unwrap_or("").contains("Uncertain"));
    // a default landscape view exists and includes all four placeholders
    let views = ws.views.system_landscape_views.as_ref().expect("default sketch view");
    assert_eq!(views.len(), 1);
    let elements = views[0].element_views.as_ref().expect("view has elements");
    assert_eq!(elements.len(), 4);
}

#[test]
fn sketch_directive_enables_vivification_in_workspace() {
    let dsl = r#"
workspace "WS" {
    !sketch
    model {
        shop = softwareSystem "Shop"
        shop -> billing "charges"
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    assert_eq!(systems.len(), 2, "billing vivified");
    assert!(systems[1].tags.as_deref().unwrap().contains("Placeholder"));
}

#[test]
fn strict_workspace_does_not_vivify() {
    let dsl = r#"
workspace {
    model {
        shop = softwareSystem "Shop"
        shop -> billing "charges"
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    assert_eq!(systems.len(), 1, "no vivification without !sketch");
}

#[test]
fn uncertainty_marker_on_element() {
    let dsl = r#"
workspace {
    model {
        billing = softwareSystem "Billing" ?
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let systems = ws.model.software_systems.as_ref().unwrap();
    assert!(systems[0].tags.as_deref().unwrap().contains("Uncertain"));
}

#[test]
fn named_relationship_registers_identifier() {
    let dsl = r#"
workspace {
    model {
        api = softwareSystem "API"
        billing = softwareSystem "Billing"
        orderFlow = api -> billing "OrderPlaced" {
            kind async
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let api = &ws.model.software_systems.as_ref().unwrap()[0];
    let rels = api.relationships.as_ref().expect("api owns the relationship");
    assert_eq!(rels[0].kind, Some(RelationshipKind::Async));
    assert_eq!(rels[0].description.as_deref(), Some("OrderPlaced"));
}

#[test]
fn kind_alias_container_desugars() {
    let dsl = r#"
workspace {
    specification {
        kind queue container {
            tags "Queue,Connector"
            technology "Kafka"
        }
    }
    model {
        shop = softwareSystem "Shop" {
            orders = queue "Order queue"
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let shop = &ws.model.software_systems.as_ref().unwrap()[0];
    let containers = shop.containers.as_ref().expect("alias should create a container");
    assert_eq!(containers.len(), 1);
    let q = &containers[0];
    assert_eq!(q.name, "Order queue");
    let tags = q.tags.as_deref().unwrap();
    assert!(tags.contains("Queue") && tags.contains("Connector"), "alias tags merged: {}", tags);
    assert_eq!(q.technology.as_deref(), Some("Kafka"), "alias default technology");
    let props = q.properties.as_ref().expect("kind property recorded");
    assert_eq!(props.get("kind").map(|s| s.as_str()), Some("queue"));
}

#[test]
fn kind_alias_system_level_and_explicit_technology_wins() {
    let dsl = r#"
workspace {
    specification {
        kind actor person
        kind lambda container { technology "AWS Lambda" }
    }
    model {
        ops = actor "Operator"
        shop = softwareSystem "Shop" {
            resize = lambda "Image resizer" "" "Rust"
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let people = ws.model.people.as_ref().expect("actor alias creates person");
    assert_eq!(people[0].name, "Operator");
    assert_eq!(people[0].properties.as_ref().unwrap().get("kind").unwrap(), "actor");
    let shop = &ws.model.software_systems.as_ref().unwrap()[0];
    let lambda = &shop.containers.as_ref().unwrap()[0];
    assert_eq!(lambda.technology.as_deref(), Some("Rust"), "explicit technology beats alias default");
}

#[test]
fn kind_alias_bad_base_errors() {
    let dsl = r#"
workspace {
    specification {
        kind widget gadget
    }
    model {}
}
"#;
    let err = parse_str(dsl).expect_err("gadget is not a valid base kind");
    assert!(format!("{}", err).contains("base"), "error should mention base kinds");
}

#[test]
fn include_directive_splices_files() {
    let dir = std::env::temp_dir().join(format!("sdsl-include-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sub.dsl"), "billing = softwareSystem \"Billing\"\n").unwrap();
    std::fs::write(
        dir.join("main.dsl"),
        "workspace {\n    model {\n        shop = softwareSystem \"Shop\"\n        !include sub.dsl\n        shop -> billing \"charges\"\n    }\n}\n",
    ).unwrap();
    let ws = parse_file(dir.join("main.dsl")).expect("include should splice");
    let systems = ws.model.software_systems.as_ref().unwrap();
    assert_eq!(systems.len(), 2);
    let shop = &systems[0];
    let rels = shop.relationships.as_ref().expect("relationship resolved across include");
    assert_eq!(rels[0].destination_id, systems[1].id);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn element_body_tags_description_technology_url() {
    let dsl = r#"
workspace {
    model {
        shop = softwareSystem "Shop" {
            db = container "Database" {
                technology "PostgreSQL"
                description "Primary store"
                tags "Database"
                url "https://internal/db"
            }
        }
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let db = &ws.model.software_systems.as_ref().unwrap()[0].containers.as_ref().unwrap()[0];
    assert_eq!(db.technology.as_deref(), Some("PostgreSQL"));
    assert_eq!(db.description.as_deref(), Some("Primary store"));
    assert_eq!(db.url.as_deref(), Some("https://internal/db"));
    assert!(db.tags.as_deref().unwrap().contains("Database"),
        "body tags must not be silently dropped: {:?}", db.tags);
}

// ─── Phase 4a: auto view specs ───────────────────────────────────────────────

#[test]
fn auto_view_specs_parse() {
    let dsl = r#"
workspace {
    model {
        api = softwareSystem "API"
        db = softwareSystem "DB"
        api -> db "reads"
    }
    views {
        auto
        auto focus api {
            depth 2
            direction in
            splitBy kind
        }
        auto perspective "security"
        auto layer "domain"
        auto slice relationship.kind==dataflow && element.tag==Core
        auto paths api db
        auto rollup owner
        auto rollup
        auto asof m1
        auto delta m1 m2
        auto lint
    }
}
"#;
    let ws = parse_str(dsl).expect("should parse");
    let api_id = ws.model.software_systems.as_ref().unwrap()[0].id.clone();
    let db_id = ws.model.software_systems.as_ref().unwrap()[1].id.clone();
    let specs = ws.views.auto_views.as_ref().expect("auto views recorded");
    assert_eq!(specs.len(), 11);
    assert_eq!(specs[0].generator, "default");
    assert_eq!(specs[1].generator, "focus");
    // element refs are resolved to ids at parse time, while the register is alive
    assert_eq!(specs[1].target.as_deref(), Some(api_id.as_str()));
    assert_eq!(specs[1].depth, Some(2));
    assert_eq!(specs[1].direction.as_deref(), Some("in"));
    assert_eq!(specs[1].split_by.as_deref(), Some("kind"));
    assert_eq!(specs[2].generator, "perspective");
    assert_eq!(specs[2].target.as_deref(), Some("security"));
    assert_eq!(specs[3].target.as_deref(), Some("domain"));
    assert_eq!(specs[4].generator, "slice");
    assert_eq!(specs[4].expression.as_deref(), Some("relationship.kind==dataflow&&element.tag==Core"));
    assert_eq!(specs[5].target.as_deref(), Some(api_id.as_str()));
    assert_eq!(specs[5].target2.as_deref(), Some(db_id.as_str()));
    assert_eq!(specs[6].target.as_deref(), Some("owner"));
    assert!(specs[7].target.is_none(), "bare rollup takes owner default at generation time");
    assert_eq!(specs[8].target.as_deref(), Some("m1"));
    assert_eq!(specs[9].target.as_deref(), Some("m1"));
    assert_eq!(specs[9].target2.as_deref(), Some("m2"));
    assert_eq!(specs[10].generator, "lint");
}

#[test]
fn auto_focus_bad_direction_errors() {
    let dsl = r#"
workspace {
    model { api = softwareSystem "API" }
    views {
        auto focus api { direction sideways }
    }
}
"#;
    assert!(parse_str(dsl).is_err());
}
