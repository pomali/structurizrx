//! End-to-end tests for `structurizrx locate`, run against the built binary.

use std::path::PathBuf;
use std::process::{Command, Output};

const SHOP: &str = "workspace \"Shop\" {
    model {
        shop = softwareSystem \"Shop\" {
            api = container \"API\" {
                port http \"HTTP\"
            }
            !include parts/data.dsl
            api -> db \"reads orders\"
        }
    }
    views {
        container shop \"containers\" {
            include *
        }
    }
}
";

/// A two-file workspace in its own temp directory, named so it is served as `shop`.
fn workspace(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sx-locate-{}-{}", test, std::process::id()));
    std::fs::create_dir_all(dir.join("parts")).unwrap();
    std::fs::write(dir.join("shop.dsl"), SHOP).unwrap();
    std::fs::write(dir.join("parts/data.dsl"), "db = container \"Database\"\n").unwrap();
    dir
}

fn locate(dir: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_structurizrx"))
        .current_dir(dir)
        .arg("locate")
        .arg("shop.dsl")
        .args(args)
        .output()
        .expect("runs")
}

#[test]
fn text_output_points_at_declarations_across_includes() {
    let dir = workspace("text");
    let out = locate(
        &dir,
        &["Shop/API", "Shop/Database", "Shop/API->Shop/Database"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        stdout.contains(
            "container  Shop/API\n  shop.dsl:4:13 (to line 6)\n    api = container \"API\" {"
        ),
        "{stdout}"
    );
    let included = PathBuf::from("parts").join("data.dsl");
    assert!(
        stdout.contains(&format!(
            "  {}:1:1\n    db = container \"Database\"",
            included.display()
        )),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("relationship  Shop/API -> Shop/Database \"reads orders\"\n  shop.dsl:8:13"),
        "{stdout}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn viewer_link_resolves_view_and_selection_as_json() {
    let dir = workspace("link");
    let link = "http://localhost:3000/workspace/shop#containers&sel=Shop%2FAPI.http,Shop%2FAPI-%3EShop%2FDatabase";
    let out = locate(&dir, &[link, "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "the slug matches: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let items = json["items"].as_array().unwrap();
    let summary: Vec<(&str, u64)> = items
        .iter()
        .map(|i| {
            (
                i["kind"].as_str().unwrap(),
                i["locations"][0]["line"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![("view", 12), ("port", 5), ("relationship", 8)]
    );
    assert_eq!(items[1]["path"], "Shop/API.HTTP");
    assert_eq!(
        items[0]["locations"][0]["source"],
        "container shop \"containers\" {"
    );
    assert_eq!(json["unresolved"], serde_json::json!([]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unknown_references_fail_with_suggestions() {
    let dir = workspace("miss");
    let out = locate(&dir, &["Shop/APX", "Other/API", "Shop/API"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains(
            "error: Shop/APX: no element or port with path 'Shop/APX' (did you mean 'Shop/API'?)"
        ),
        "{stderr}"
    );
    // Too far for an edit-distance match, but the same name under another parent.
    assert!(
        stderr.contains("error: Other/API:") && stderr.contains("did you mean 'Shop/API'?"),
        "{stderr}"
    );
    // What did resolve is still printed.
    assert!(String::from_utf8_lossy(&out.stdout).contains("container  Shop/API"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn link_to_another_workspace_warns() {
    let dir = workspace("slug");
    let out = locate(&dir, &["http://localhost:3000/workspace/bank#containers"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success());
    assert!(
        stderr.contains("warning:") && stderr.contains("'bank'") && stderr.contains("'shop'"),
        "{stderr}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
