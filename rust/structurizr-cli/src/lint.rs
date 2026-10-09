//! `structurizrx lint`: blocking findings (`structurizr_query::lint`), hygiene
//! warnings (`structurizr_query::review`) and dependency cycles
//! (`structurizr_query::cluster`), in one report.

use std::path::Path;

use anyhow::Result;
use structurizr_query::cluster::{cluster, ClusterOptions, Level};

/// One finding, blocking or warning, ready to print or serialize.
struct Finding {
    code: &'static str,
    message: String,
    element_id: String,
    name: String,
    blocking: bool,
}

/// Print the findings. Returns whether the workspace passes: no blocking
/// findings, and with `strict` no warnings or cycles either.
pub fn run(file: &Path, strict: bool, json: bool) -> Result<bool> {
    let mut workspace = crate::load_workspace(&file.to_path_buf())?;
    // Materialize generated (`auto`) views first, so `not-in-any-view` is
    // judged against the effective view set, matching the web review page.
    if let Err(e) = structurizr_query::generate_views(&mut workspace) {
        eprintln!("warning: view generation failed: {}", e);
    }

    let mut blocking: Vec<Finding> = Vec::new();
    let mut warnings: Vec<Finding> = Vec::new();

    let review = structurizr_query::review(&workspace);
    for element in &review.elements {
        for f in &element.findings {
            let finding = Finding {
                code: f.code,
                message: f.message.clone(),
                element_id: element.id.clone(),
                name: element.name.clone(),
                blocking: f.blocking,
            };
            if f.blocking {
                blocking.push(finding);
            } else {
                warnings.push(finding);
            }
        }
    }

    // Cycles: container level always, component level too when the model has
    // any components.
    let has_components = review
        .elements
        .iter()
        .any(|e| e.kind == "component");
    let mut levels = vec![Level::Container];
    if has_components {
        levels.push(Level::Component);
    }
    for level in levels {
        let analysis = cluster(
            &workspace,
            &ClusterOptions { level, ..Default::default() },
        );
        for cyc in &analysis.cycles {
            let members = cyc.member_names.join(" -> ");
            warnings.push(Finding {
                code: "cycle",
                message: format!("dependency cycle: {members}"),
                element_id: cyc.member_ids.first().cloned().unwrap_or_default(),
                name: members,
                blocking: false,
            });
        }
    }

    let passed = blocking.is_empty() && (!strict || warnings.is_empty());

    if json {
        let to_json = |f: &Finding| {
            serde_json::json!({
                "code": f.code,
                "message": f.message,
                "elementId": f.element_id,
                "name": f.name,
                "blocking": f.blocking,
            })
        };
        let out = serde_json::json!({
            "passed": passed,
            "blocking": blocking.iter().map(to_json).collect::<Vec<_>>(),
            "warnings": warnings.iter().map(to_json).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if blocking.is_empty() && warnings.is_empty() {
        println!("✓ no findings");
    } else {
        if !blocking.is_empty() {
            println!("Blocking:");
            for f in &blocking {
                println!("  [{}] {} ({})", f.code, f.message, f.name);
            }
        }
        if !warnings.is_empty() {
            if !blocking.is_empty() {
                println!();
            }
            println!("Warnings:");
            for f in &warnings {
                println!("  [{}] {} ({})", f.code, f.message, f.name);
            }
        }
        println!();
        println!(
            "{} blocking, {} warning{}",
            blocking.len(),
            warnings.len(),
            if warnings.len() == 1 { "" } else { "s" }
        );
    }

    Ok(passed)
}
