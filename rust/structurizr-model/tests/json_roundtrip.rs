use structurizr_model::{
    Container, Milestone, Model, Perspective, Port, PortDirection, Relationship,
    RelationshipKind, SoftwareSystem, Status, Workspace,
};
use std::path::PathBuf;

fn json_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../original-java/structurizr-client/src/test/resources")
        .join(name)
}

#[test]
fn deserialize_big_bank_json() {
    let path = json_path("structurizr-36141-workspace.json");
    let content = std::fs::read_to_string(&path)
        .expect("should read JSON file");
    let ws: Workspace = serde_json::from_str(&content)
        .expect("should deserialize workspace");
    assert_eq!(ws.name, "Big Bank plc - Internet Banking System");
    let people = ws.model.people.as_ref().expect("should have people");
    assert!(!people.is_empty());
    let systems = ws.model.software_systems.as_ref().expect("should have software systems");
    assert!(!systems.is_empty());
}

#[test]
fn roundtrip_workspace() {
    let mut ws = Workspace::default();
    ws.name = "Round Trip Test".to_string();
    ws.description = Some("A test".to_string());

    let json = serde_json::to_string_pretty(&ws).expect("should serialize");
    let ws2: Workspace = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(ws.name, ws2.name);
    assert_eq!(ws.description, ws2.description);
}

/// All new fields survive a serialize → deserialize round-trip.
#[test]
fn roundtrip_new_fields() {
    let port_a = Port {
        id: "port-a".to_string(),
        name: "Port A".to_string(),
        protocol: Some("HTTP".to_string()),
        direction: Some(PortDirection::InOut),
        perspectives: Some(vec![Perspective {
            name: "Security".to_string(),
            description: Some("TLS required".to_string()),
            value: None,
        }]),
        ..Default::default()
    };
    let port_b = Port {
        id: "port-b".to_string(),
        name: "Port B".to_string(),
        direction: Some(PortDirection::Out),
        ..Default::default()
    };

    let mut rel = Relationship {
        id: "r1".to_string(),
        source_id: "c1".to_string(),
        destination_id: "c1".to_string(),
        ..Default::default()
    };
    rel.kind = Some(RelationshipKind::Async);
    rel.source_port_id = Some("port-a".to_string());
    rel.destination_port_id = Some("port-b".to_string());
    rel.status = Some(Status::Draft);
    rel.introduced = Some("v1.0".to_string());

    let container = Container {
        id: "c1".to_string(),
        name: "API Container".to_string(),
        ports: Some(vec![port_a, port_b]),
        status: Some(Status::Implemented),
        introduced: Some("v1.0".to_string()),
        retired: Some("v2.0".to_string()),
        perspectives: Some(vec![Perspective {
            name: "Performance".to_string(),
            description: Some("Must handle 1k rps".to_string()),
            value: None,
        }]),
        relationships: Some(vec![rel]),
        ..Default::default()
    };

    let mut ws = Workspace::default();
    ws.name = "New-Fields RT".to_string();
    ws.milestones = Some(vec![
        Milestone {
            name: "v1.0".to_string(),
            date: Some("2026-01-01".to_string()),
            description: Some("Initial release".to_string()),
        },
        Milestone {
            name: "v2.0".to_string(),
            date: None,
            description: None,
        },
    ]);
    ws.perspectives = Some(vec![Perspective {
        name: "Security".to_string(),
        description: Some("Workspace-level security perspective".to_string()),
        value: None,
    }]);
    ws.model = Model {
        software_systems: Some(vec![SoftwareSystem {
            id: "sys1".to_string(),
            name: "My System".to_string(),
            status: Some(Status::Specified),
            retired: Some("v2.0".to_string()),
            containers: Some(vec![container]),
            ..Default::default()
        }]),
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&ws).expect("should serialize");
    let ws2: Workspace = serde_json::from_str(&json).expect("should deserialize");

    // Milestones survive
    let ms = ws2.milestones.as_ref().expect("milestones should be present");
    assert_eq!(ms.len(), 2);
    assert_eq!(ms[0].name, "v1.0");
    assert_eq!(ms[0].date.as_deref(), Some("2026-01-01"));
    assert_eq!(ms[1].name, "v2.0");

    // Workspace perspective registry survives
    let wsp = ws2.perspectives.as_ref().expect("perspectives should be present");
    assert_eq!(wsp[0].name, "Security");

    let sys = &ws2.model.software_systems.as_ref().unwrap()[0];
    assert_eq!(sys.status, Some(Status::Specified));
    assert_eq!(sys.retired.as_deref(), Some("v2.0"));

    let ct = &sys.containers.as_ref().unwrap()[0];
    assert_eq!(ct.status, Some(Status::Implemented));
    assert_eq!(ct.introduced.as_deref(), Some("v1.0"));
    assert_eq!(ct.retired.as_deref(), Some("v2.0"));

    let ports = ct.ports.as_ref().expect("ports should be present");
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].id, "port-a");
    assert_eq!(ports[0].protocol.as_deref(), Some("HTTP"));
    assert_eq!(ports[0].direction, Some(PortDirection::InOut));
    assert_eq!(ports[1].direction, Some(PortDirection::Out));

    let rel2 = &ct.relationships.as_ref().unwrap()[0];
    assert_eq!(rel2.kind, Some(RelationshipKind::Async));
    assert_eq!(rel2.source_port_id.as_deref(), Some("port-a"));
    assert_eq!(rel2.destination_port_id.as_deref(), Some("port-b"));
    assert_eq!(rel2.status, Some(Status::Draft));
    assert_eq!(rel2.introduced.as_deref(), Some("v1.0"));
}

/// A plain default workspace must not emit any of the new JSON keys.
#[test]
fn plain_workspace_has_no_new_keys() {
    let mut ws = Workspace::default();
    ws.name = "Minimal".to_string();
    let json = serde_json::to_string(&ws).expect("should serialize");
    for key in &[
        "ports",
        "kind",
        "status",
        "milestones",
        "introduced",
        "retired",
        "perspectives",
        "sourcePortId",
        "destinationPortId",
    ] {
        assert!(
            !json.contains(key),
            "plain workspace JSON must not contain '{}'",
            key
        );
    }
}

/// Parse a JSON literal using camelCase keys and assert the field values.
#[test]
fn parse_from_literal_camel_case() {
    let json = r#"{
        "name": "Literal Test",
        "model": {
            "softwareSystems": [
                {
                    "id": "sys1",
                    "name": "Sys",
                    "status": "idea",
                    "introduced": "alpha",
                    "containers": [
                        {
                            "id": "c1",
                            "name": "C",
                            "ports": [
                                {
                                    "id": "p1",
                                    "name": "P",
                                    "direction": "inout"
                                }
                            ],
                            "relationships": [
                                {
                                    "id": "r1",
                                    "sourceId": "c1",
                                    "destinationId": "c1",
                                    "kind": "async",
                                    "status": "draft",
                                    "sourcePortId": "p1",
                                    "destinationPortId": "p1"
                                }
                            ]
                        }
                    ]
                }
            ]
        },
        "views": {}
    }"#;

    let ws: Workspace = serde_json::from_str(json).expect("should deserialize");
    let sys = &ws.model.software_systems.as_ref().unwrap()[0];
    assert_eq!(sys.status, Some(Status::Idea));
    assert_eq!(sys.introduced.as_deref(), Some("alpha"));

    let ct = &sys.containers.as_ref().unwrap()[0];
    let port = &ct.ports.as_ref().unwrap()[0];
    assert_eq!(port.direction, Some(PortDirection::InOut));

    let rel = &ct.relationships.as_ref().unwrap()[0];
    assert_eq!(rel.kind, Some(RelationshipKind::Async));
    assert_eq!(rel.status, Some(Status::Draft));
    assert_eq!(rel.source_port_id.as_deref(), Some("p1"));
    assert_eq!(rel.destination_port_id.as_deref(), Some("p1"));
}
