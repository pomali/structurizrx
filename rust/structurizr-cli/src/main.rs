use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use structurizr_dsl::parse_file;
use structurizr_model::{validation, ViewSet, Workspace};
use structurizr_renderer::{
    dot::DotExporter, exporter::DiagramExporter, mermaid::MermaidExporter,
    plantuml::PlantUmlExporter, svg::SvgExporter,
};

#[derive(Parser)]
#[command(name = "structurizrx", version, about = "Structurizr DSL toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and validate a .dsl or .json workspace file
    Validate {
        file: PathBuf,
        /// Also fail on lint findings (placeholders, uncertain items,
        /// orphans, unbound ports)
        #[arg(long)]
        strict: bool,
    },
    /// Render diagrams from a workspace file
    Render {
        file: PathBuf,
        #[arg(long, default_value = "plantuml")]
        format: String,
        #[arg(long, short, default_value = ".")]
        output: PathBuf,
    },
    /// Export workspace to JSON
    Export {
        file: PathBuf,
        #[arg(long, short, default_value = "workspace.json")]
        output: PathBuf,
    },
    /// Print a compact plain-text summary of the model, sized for LLM context
    Digest {
        file: PathBuf,
    },
    /// Run a selector expression against a workspace (spec §6.2),
    /// e.g. `query ws.dsl "element.tag==Database"` or `query ws.dsl "->api->2"`
    Query {
        file: PathBuf,
        /// Selector expression, e.g. `element.status==idea && element.layer==domain`
        #[arg(allow_hyphen_values = true)]
        expression: String,
        /// Emit machine-readable JSON instead of a text listing
        #[arg(long)]
        json: bool,
    },
    /// Serve a workspace or directory of workspaces in a local web browser
    Serve {
        /// Path to a .dsl/.json file or a directory containing workspace(s).
        /// Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// TCP port to listen on.
        #[arg(long, short, default_value_t = 3000)]
        port: u16,
        /// Open the browser automatically after starting the server.
        #[arg(long)]
        open: bool,
    },
}

/// Render a compact, deterministic plain-text digest of the model (spec §9.1):
/// one line per element with its qualified name-path, ports, and markers; one
/// line per relationship as a name-path triple. Sized to paste into LLM context.
fn digest(ws: &Workspace) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "workspace: {}", ws.name);
    if let Some(desc) = &ws.description {
        let _ = writeln!(out, "description: {}", desc);
    }
    if let Some(ms) = &ws.milestones {
        let list: Vec<String> = ms.iter()
            .map(|m| match &m.date {
                Some(d) => format!("{}({})", m.name, d),
                None => m.name.clone(),
            })
            .collect();
        let _ = writeln!(out, "milestones: {}", list.join(", "));
    }
    if let Some(ps) = &ws.perspectives {
        let list: Vec<&str> = ps.iter().map(|p| p.name.as_str()).collect();
        let _ = writeln!(out, "perspectives: {}", list.join(", "));
    }
    let _ = writeln!(out);

    // id → qualified name path, for relationship lines
    let mut paths: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    // (element id, port id) → port name, for port-attached relationship endpoints
    let mut port_names: std::collections::HashMap<(String, String), String> = std::collections::HashMap::new();

    fn markers(
        status: &Option<structurizr_model::Status>,
        introduced: &Option<String>,
        retired: &Option<String>,
        technology: &Option<String>,
    ) -> String {
        let mut m = Vec::new();
        if let Some(t) = technology {
            m.push(t.clone());
        }
        if let Some(s) = status {
            m.push(format!("status:{}", format!("{:?}", s).to_lowercase()));
        }
        if let Some(i) = introduced {
            m.push(format!("introduced:{}", i));
        }
        if let Some(r) = retired {
            m.push(format!("retired:{}", r));
        }
        if m.is_empty() { String::new() } else { format!(" [{}]", m.join(", ")) }
    }

    fn ports_suffix(ports: &Option<Vec<structurizr_model::Port>>) -> String {
        match ports {
            Some(ps) if !ps.is_empty() => {
                let list: Vec<String> = ps.iter().map(|p| {
                    let mut s = p.name.clone();
                    if let Some(proto) = &p.protocol { s = format!("{}({})", s, proto); }
                    s
                }).collect();
                format!(" ports: {}", list.join(", "))
            }
            _ => String::new(),
        }
    }

    for p in ws.model.people.iter().flatten() {
        paths.insert(&p.id, p.name.clone());
        let _ = writeln!(out, "person {}{}", p.name, markers(&p.status, &p.introduced, &p.retired, &None));
    }
    let record_ports = |element_id: &str, ports: &Option<Vec<structurizr_model::Port>>,
                            port_names: &mut std::collections::HashMap<(String, String), String>| {
        for p in ports.iter().flatten() {
            port_names.insert((element_id.to_string(), p.id.clone()), p.name.clone());
        }
    };
    for s in ws.model.software_systems.iter().flatten() {
        paths.insert(&s.id, s.name.clone());
        record_ports(&s.id, &s.ports, &mut port_names);
        let _ = writeln!(out, "system {}{}{}", s.name, markers(&s.status, &s.introduced, &s.retired, &None), ports_suffix(&s.ports));
        for c in s.containers.iter().flatten() {
            let path = format!("{}/{}", s.name, c.name);
            paths.insert(&c.id, path.clone());
            record_ports(&c.id, &c.ports, &mut port_names);
            let _ = writeln!(out, "  container {}{}{}", path, markers(&c.status, &c.introduced, &c.retired, &c.technology), ports_suffix(&c.ports));
            for comp in c.components.iter().flatten() {
                let cpath = format!("{}/{}", path, comp.name);
                paths.insert(&comp.id, cpath.clone());
                record_ports(&comp.id, &comp.ports, &mut port_names);
                let _ = writeln!(out, "    component {}{}{}", cpath, markers(&comp.status, &comp.introduced, &comp.retired, &comp.technology), ports_suffix(&comp.ports));
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        paths.insert(&ce.id, ce.name.clone());
        let _ = writeln!(out, "element {}", ce.name);
    }
    let _ = writeln!(out);

    // Relationships, in model order
    let mut rel_lines: Vec<String> = Vec::new();
    let mut push_rels = |rels: &Option<Vec<structurizr_model::Relationship>>, paths: &std::collections::HashMap<&str, String>| {
        for r in rels.iter().flatten() {
            let mut src = paths.get(r.source_id.as_str()).cloned().unwrap_or_else(|| r.source_id.clone());
            let mut dst = paths.get(r.destination_id.as_str()).cloned().unwrap_or_else(|| r.destination_id.clone());
            if let Some(pid) = &r.source_port_id {
                if let Some(pname) = port_names.get(&(r.source_id.clone(), pid.clone())) {
                    src = format!("{}.{}", src, pname);
                }
            }
            if let Some(pid) = &r.destination_port_id {
                if let Some(pname) = port_names.get(&(r.destination_id.clone(), pid.clone())) {
                    dst = format!("{}.{}", dst, pname);
                }
            }
            let mut markers = Vec::new();
            if let Some(k) = &r.kind { markers.push(format!("{:?}", k).to_lowercase()); }
            if let Some(s) = &r.status { markers.push(format!("status:{}", format!("{:?}", s).to_lowercase())); }
            if let Some(i) = &r.introduced { markers.push(format!("introduced:{}", i)); }
            if let Some(x) = &r.retired { markers.push(format!("retired:{}", x)); }
            let marker_s = if markers.is_empty() { String::new() } else { format!(" [{}]", markers.join(", ")) };
            let desc = r.description.as_deref().map(|d| format!(" \"{}\"", d)).unwrap_or_default();
            rel_lines.push(format!("rel {} -> {}{}{}", src, dst, desc, marker_s));
        }
    };
    for p in ws.model.people.iter().flatten() { push_rels(&p.relationships, &paths); }
    for s in ws.model.software_systems.iter().flatten() {
        push_rels(&s.relationships, &paths);
        for c in s.containers.iter().flatten() {
            push_rels(&c.relationships, &paths);
            for comp in c.components.iter().flatten() { push_rels(&comp.relationships, &paths); }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() { push_rels(&ce.relationships, &paths); }
    for line in rel_lines {
        let _ = writeln!(out, "{}", line);
    }
    out
}

/// Map every element id in the model to a human-readable name for query output.
fn collect_element_names(ws: &Workspace) -> std::collections::HashMap<String, String> {
    let mut names = std::collections::HashMap::new();
    for p in ws.model.people.iter().flatten() {
        names.insert(p.id.clone(), format!("person \"{}\"", p.name));
    }
    for s in ws.model.software_systems.iter().flatten() {
        names.insert(s.id.clone(), format!("softwareSystem \"{}\"", s.name));
        for c in s.containers.iter().flatten() {
            names.insert(c.id.clone(), format!("container \"{}\"", c.name));
            for comp in c.components.iter().flatten() {
                names.insert(comp.id.clone(), format!("component \"{}\"", comp.name));
            }
        }
    }
    for ce in ws.model.custom_elements.iter().flatten() {
        names.insert(ce.id.clone(), format!("element \"{}\"", ce.name));
    }
    names
}

fn load_workspace(path: &PathBuf) -> Result<Workspace> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "json" {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let ws: Workspace = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from {}", path.display()))?;
        Ok(ws)
    } else {
        parse_file(path)
            .with_context(|| format!("Failed to parse DSL from {}", path.display()))
    }
}

/// View-type name paired with how many views of that type are defined.
fn view_type_counts(views: &ViewSet) -> Vec<(&'static str, usize)> {
    vec![
        ("systemLandscape", views.system_landscape_views.as_ref().map_or(0, Vec::len)),
        ("systemContext", views.system_context_views.as_ref().map_or(0, Vec::len)),
        ("container", views.container_views.as_ref().map_or(0, Vec::len)),
        ("component", views.component_views.as_ref().map_or(0, Vec::len)),
        ("dynamic", views.dynamic_views.as_ref().map_or(0, Vec::len)),
        ("deployment", views.deployment_views.as_ref().map_or(0, Vec::len)),
        ("filtered", views.filtered_views.as_ref().map_or(0, Vec::len)),
        ("image", views.image_views.as_ref().map_or(0, Vec::len)),
        ("custom", views.custom_views.as_ref().map_or(0, Vec::len)),
    ]
}

/// View types each exporter's `export_workspace` actually renders. Kept in
/// sync manually with the `if let Some(..) = views.*` cases in each exporter;
/// anything not listed here is silently dropped by that exporter today.
fn handled_view_types(format: &str) -> &'static [&'static str] {
    match format.to_lowercase().as_str() {
        "svg" => &["systemLandscape", "systemContext", "container"],
        "mermaid" => &["systemLandscape", "systemContext"],
        "dot" | "graphviz" => &["systemLandscape", "systemContext"],
        _ => &["systemLandscape", "systemContext", "container"], // plantuml
    }
}

/// Warn about views defined in the workspace that the chosen exporter has no
/// support for, so they don't just vanish without explanation.
fn warn_on_unsupported_views(views: &ViewSet, format: &str) {
    let handled = handled_view_types(format);
    let skipped: Vec<(&str, usize)> = view_type_counts(views)
        .into_iter()
        .filter(|(name, count)| *count > 0 && !handled.contains(name))
        .collect();
    if skipped.is_empty() {
        return;
    }
    let total: usize = skipped.iter().map(|(_, count)| count).sum();
    let breakdown: Vec<String> = skipped
        .iter()
        .map(|(name, count)| format!("{} {}", count, name))
        .collect();
    eprintln!(
        "Warning: {} view(s) skipped ({} exporter does not support: {})",
        total,
        format,
        breakdown.join(", ")
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { file, strict } => {
            let workspace = load_workspace(&file)?;
            let errors = validation::validate(&workspace);
            if !errors.is_empty() {
                eprintln!("Validation errors:");
                for e in &errors {
                    eprintln!("  - {}", e);
                }
                std::process::exit(1);
            }
            if strict {
                // Reuse the lint generator: its auto-lint view description
                // carries the findings summary.
                let mut ws = workspace.clone();
                ws.views.auto_views = Some(vec![structurizr_model::AutoViewSpec {
                    generator: "lint".to_string(),
                    ..Default::default()
                }]);
                structurizr_query::generate_views(&mut ws)
                    .map_err(|e| anyhow::anyhow!("lint: {}", e))?;
                let findings = ws
                    .views
                    .system_landscape_views
                    .iter()
                    .flatten()
                    .find(|v| v.key.as_deref() == Some("auto-lint"))
                    .and_then(|v| v.description.clone())
                    .unwrap_or_default();
                if findings != "no findings" {
                    eprintln!("Lint findings: {}", findings);
                    std::process::exit(1);
                }
            }
            println!("✓ Workspace '{}' is valid", workspace.name);
        }
        Commands::Render { file, format, output } => {
            let mut workspace = load_workspace(&file)?;
            let generated = structurizr_query::generate_views(&mut workspace)
                .map_err(|e| anyhow::anyhow!("view generation: {}", e))?;
            if !generated.is_empty() {
                println!("Generated views: {}", generated.join(", "));
            }
            warn_on_unsupported_views(&workspace.views, &format);
            std::fs::create_dir_all(&output)
                .with_context(|| format!("Cannot create output dir {}", output.display()))?;

            let diagrams: Vec<_> = match format.to_lowercase().as_str() {
                "mermaid" => MermaidExporter.export_workspace(&workspace),
                "dot" | "graphviz" => DotExporter.export_workspace(&workspace),
                "svg" => SvgExporter.export_workspace(&workspace),
                _ => PlantUmlExporter.export_workspace(&workspace),
            };

            if diagrams.is_empty() {
                println!("No diagrams to render.");
            } else {
                for d in &diagrams {
                    let filename = output.join(format!("{}.{}", d.key, d.extension()));
                    std::fs::write(&filename, &d.content)
                        .with_context(|| format!("Cannot write {}", filename.display()))?;
                    println!("Written: {}", filename.display());
                }
            }
        }
        Commands::Export { file, output } => {
            let workspace = load_workspace(&file)?;
            let json = serde_json::to_string_pretty(&workspace)
                .context("Failed to serialize workspace to JSON")?;
            std::fs::write(&output, &json)
                .with_context(|| format!("Cannot write {}", output.display()))?;
            println!("Exported workspace to {}", output.display());
        }
        Commands::Digest { file } => {
            let workspace = load_workspace(&file)?;
            print!("{}", digest(&workspace));
        }
        Commands::Query { file, expression, json } => {
            let workspace = load_workspace(&file)?;
            let selection = structurizr_query::query(&expression, &workspace)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let names = collect_element_names(&workspace);
            if json {
                let out = serde_json::json!({
                    "elements": selection.elements.iter().map(|id| serde_json::json!({
                        "id": id,
                        "name": names.get(id),
                    })).collect::<Vec<_>>(),
                    "relationships": selection.relationships.iter().collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                for id in &selection.elements {
                    match names.get(id) {
                        Some(name) => println!("element  {}  {}", id, name),
                        None => println!("element  {}", id),
                    }
                }
                for id in &selection.relationships {
                    println!("relationship  {}", id);
                }
                if selection.elements.is_empty() && selection.relationships.is_empty() {
                    eprintln!("(no matches)");
                }
            }
        }
        Commands::Serve { path, port, open } => {
            structurizr_web::serve(structurizr_web::ServeOptions {
                path,
                port,
                open_browser: open,
            })
            .await?;
        }
    }

    Ok(())
}

