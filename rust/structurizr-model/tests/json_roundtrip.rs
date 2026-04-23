use structurizr_model::Workspace;
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
