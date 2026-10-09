//! Smoke tests for the `diff`, `lint`, `clusters` and `graph` subcommands,
//! exercised as a subprocess against the built `structurizrx` binary — CLI
//! parity with the web viewer's diff/review/clusters/graph pages.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_structurizrx"))
}

fn shop_dsl() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("site")
        .join("examples")
        .join("shop.dsl")
}

fn write_temp(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "structurizrx-cli-parity-{}-{}",
        std::process::id(),
        name
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write temp workspace");
    path
}

const WS_A: &str = r#"
workspace "Shop" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            webapp = container "Web App"
            api = container "API"
        }
        customer -> webapp "shops on"
        webapp -> api "calls"
    }
    views {
        systemContext shop {
            include *
            autoLayout
        }
    }
}
"#;

const WS_B: &str = r#"
workspace "Shop" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            webapp = container "Web App"
            api = container "API"
            db = container "Database" "" "PostgreSQL"
        }
        customer -> webapp "shops on"
        webapp -> api "calls"
        api -> db "reads and writes" "SQL"
    }
    views {
        systemContext shop {
            include *
            autoLayout
        }
    }
}
"#;

/// A tiny workspace crafted to trip several review/lint codes at once: an
/// undescribed, technology-less, view-less, unrelated element plus a
/// dependency cycle between two containers.
const WS_LINT: &str = r#"
workspace "Lint" {
    model {
        shop = softwareSystem "Shop" {
            a = container "A"
            b = container "B"
        }
        a -> b "calls"
        b -> a "calls back"
    }
    views {
        systemContext shop {
            include *
            autoLayout
        }
    }
}
"#;

#[test]
fn lint_runs_and_reports_expected_codes() {
    let path = write_temp("lint.dsl", WS_LINT);

    let output = bin().arg("lint").arg(&path).arg("--json").output().expect("run lint");
    assert!(output.status.success(), "lint should exit 0 without --strict: {output:?}");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("lint --json must be valid JSON");
    assert_eq!(json["passed"], true);

    let warning_codes: Vec<&str> = json["warnings"]
        .as_array()
        .expect("warnings array")
        .iter()
        .map(|w| w["code"].as_str().unwrap())
        .collect();
    assert!(warning_codes.contains(&"missing-description"), "{warning_codes:?}");
    assert!(warning_codes.contains(&"missing-technology"), "{warning_codes:?}");
    assert!(warning_codes.contains(&"cycle"), "{warning_codes:?}");
    assert!(json["blocking"].as_array().unwrap().is_empty());

    // Text mode also runs cleanly and mentions the cycle.
    let text_output = bin().arg("lint").arg(&path).output().expect("run lint text");
    assert!(text_output.status.success());
    let text = String::from_utf8_lossy(&text_output.stdout);
    assert!(text.contains("cycle"), "{text}");

    // --strict fails the build because of the warnings.
    let strict = bin().arg("lint").arg(&path).arg("--strict").status().expect("run lint --strict");
    assert!(!strict.success(), "strict lint should fail on warnings");
}

#[test]
fn clusters_prints_stats_line_and_json_parses() {
    let path = shop_dsl();

    let text_output = bin().arg("clusters").arg(&path).output().expect("run clusters");
    assert!(text_output.status.success(), "{text_output:?}");
    let text = String::from_utf8_lossy(&text_output.stdout);
    assert!(text.contains("elements"), "{text}");
    assert!(text.contains("communities"), "{text}");
    assert!(text.contains("cycle"), "{text}");

    let json_output = bin()
        .arg("clusters")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("run clusters --json");
    assert!(json_output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&json_output.stdout).expect("clusters --json must be valid JSON");
    assert_eq!(json["level"], "container");
    assert!(json["nodes"].is_array());

    // An invalid level is rejected clearly.
    let bad = bin()
        .arg("clusters")
        .arg(&path)
        .arg("--level")
        .arg("nonsense")
        .output()
        .expect("run clusters with bad level");
    assert!(!bad.status.success());
}

#[test]
fn graph_dot_starts_with_digraph() {
    let path = shop_dsl();

    let output = bin()
        .arg("graph")
        .arg(&path)
        .arg("--format")
        .arg("dot")
        .output()
        .expect("run graph --format dot");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("digraph"), "{text}");

    let json_output = bin()
        .arg("graph")
        .arg(&path)
        .arg("--format")
        .arg("json")
        .output()
        .expect("run graph --format json");
    assert!(json_output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&json_output.stdout).expect("graph --format json must be valid JSON");
    assert!(json["nodes"].is_array());
}

#[test]
fn diff_against_reports_the_added_element_and_exit_codes() {
    let a = write_temp("a.dsl", WS_A);
    let b = write_temp("b.dsl", WS_B);

    let output = bin()
        .arg("diff")
        .arg(&a)
        .arg("--against")
        .arg(&b)
        .output()
        .expect("run diff --against");
    assert!(output.status.success(), "diff without --fail-on-change exits 0: {output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Shop/Database"), "{text}");
    assert!(text.contains('+'), "{text}");

    let json_output = bin()
        .arg("diff")
        .arg(&a)
        .arg("--against")
        .arg(&b)
        .arg("--json")
        .output()
        .expect("run diff --against --json");
    assert!(json_output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&json_output.stdout).expect("diff --json must be valid JSON");
    assert_eq!(json["diff"]["summary"]["elements"]["added"], 1);

    let fail_on_change = bin()
        .arg("diff")
        .arg(&a)
        .arg("--against")
        .arg(&b)
        .arg("--fail-on-change")
        .status()
        .expect("run diff --fail-on-change");
    assert!(!fail_on_change.success(), "diff --fail-on-change must exit 1 when something changed");

    // No changes: comparing a workspace against itself.
    let no_change = bin()
        .arg("diff")
        .arg(&a)
        .arg("--against")
        .arg(&a)
        .status()
        .expect("run diff a-vs-a");
    assert!(no_change.success());
}
