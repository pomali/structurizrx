use structurizr_dsl::{parse_file, parse_str};
use std::path::PathBuf;

fn dsl_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../original-java/structurizr-dsl/src/test/resources/dsl")
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
