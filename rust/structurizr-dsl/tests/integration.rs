use structurizr_dsl::{parse_file, parse_str};
use std::path::PathBuf;

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
