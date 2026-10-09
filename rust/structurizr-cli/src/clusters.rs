//! `structurizrx clusters`: the viewer's cluster analysis as a text report
//! or JSON, on `structurizr_query::cluster`.

use std::path::PathBuf;

use anyhow::Result;
use structurizr_query::cluster::{cluster, ClusterAnalysis, ClusterOptions, Level};

pub struct Args {
    pub file: PathBuf,
    pub level: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub implied: bool,
    pub json: bool,
}

fn parse_level(value: &str) -> Result<Level> {
    match value {
        "system" | "softwareSystem" | "systems" => Ok(Level::SoftwareSystem),
        "container" | "containers" => Ok(Level::Container),
        "component" | "components" => Ok(Level::Component),
        other => {
            anyhow::bail!("invalid level '{other}': expected one of system, container, component")
        }
    }
}

pub fn run(args: &Args) -> Result<()> {
    let level = parse_level(&args.level)?;
    let mut workspace = crate::load_workspace(&args.file)?;
    if let Err(e) = structurizr_query::generate_views(&mut workspace) {
        eprintln!("warning: view generation failed: {}", e);
    }

    let options = ClusterOptions {
        level,
        include_tags: args.include.clone(),
        exclude_tags: args.exclude.clone(),
        implied: args.implied,
    };
    let analysis = cluster(&workspace, &options);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&analysis)?);
    } else {
        print_text(&analysis);
    }

    Ok(())
}

fn print_text(a: &ClusterAnalysis) {
    println!(
        "{} elements, {} dependencies, {} communities (modularity {:.3}), {} disconnected part{}, {} cycle{}, {} bridge{}, {} conformance finding{}",
        a.nodes.len(),
        a.edges.len(),
        a.communities.len(),
        a.modularity,
        a.components,
        if a.components == 1 { "" } else { "s" },
        a.cycles.len(),
        if a.cycles.len() == 1 { "" } else { "s" },
        a.bridges.len(),
        if a.bridges.len() == 1 { "" } else { "s" },
        a.conformance.len(),
        if a.conformance.len() == 1 { "" } else { "s" },
    );

    if !a.conformance.is_empty() {
        println!();
        println!("Conformance findings:");
        for f in &a.conformance {
            println!("  {}", f.message);
        }
    }

    if !a.communities.is_empty() {
        println!();
        println!("Communities:");
        for c in &a.communities {
            let names: Vec<&str> = c
                .member_ids
                .iter()
                .map(|id| {
                    a.nodes
                        .iter()
                        .find(|n| &n.id == id)
                        .map(|n| n.name.as_str())
                        .unwrap_or(id.as_str())
                })
                .collect();
            let dominant = c.dominant_declared.as_deref().unwrap_or("(none)");
            println!(
                "  #{}: {} [{}/{} agree with '{}']",
                c.id,
                names.join(", "),
                c.agreeing,
                c.member_ids.len(),
                dominant,
            );
        }
    }

    if !a.cycles.is_empty() {
        println!();
        println!("Cycles:");
        for c in &a.cycles {
            println!("  {}", c.member_names.join(" -> "));
        }
    }

    if !a.bridges.is_empty() {
        println!();
        println!("Bridges:");
        for b in &a.bridges {
            println!("  {} -> {}", b.source_name, b.target_name);
        }
    }

    if !a.articulation_point_ids.is_empty() {
        println!();
        println!("Articulation points:");
        for id in &a.articulation_point_ids {
            let name = a
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .map(|n| n.name.as_str())
                .unwrap_or(id.as_str());
            println!("  {}", name);
        }
    }

    if !a.nodes.is_empty() {
        println!();
        let name_w = a
            .nodes
            .iter()
            .map(|n| n.name.len())
            .max()
            .unwrap_or(4)
            .max(4);
        println!(
            "{:<name_w$}  {:>9}  {:>9}  {:>11}  {:>8}  {:>11}",
            "name",
            "afferent",
            "efferent",
            "instability",
            "pagerank",
            "betweenness",
            name_w = name_w
        );
        for n in &a.nodes {
            let instability = n
                .instability
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "-".to_string());
            println!(
                "{:<name_w$}  {:>9}  {:>9}  {:>11}  {:>8.4}  {:>11.4}",
                n.name,
                n.afferent,
                n.efferent,
                instability,
                n.page_rank,
                n.betweenness,
                name_w = name_w
            );
        }
    }
}
