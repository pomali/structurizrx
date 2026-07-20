use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use structurizr_dsl::parse_file;
use structurizr_model::{validation, Workspace};
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
        /// Emit machine-readable JSON (parse/validation errors and lint
        /// findings with stable codes) instead of text
        #[arg(long)]
        json: bool,
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
    /// Print the DSL extension cheat sheet (llms.txt) — the format reference
    /// for LLM agents and humans authoring workspaces
    Docs,
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { file, strict, json } => {
            let workspace = match load_workspace(&file) {
                Ok(ws) => ws,
                Err(e) if json => {
                    let out = serde_json::json!({
                        "valid": false,
                        "errors": [{ "code": "parse", "message": format!("{:#}", e) }],
                        "lint": [],
                    });
                    println!("{}", serde_json::to_string_pretty(&out)?);
                    std::process::exit(1);
                }
                Err(e) => return Err(e),
            };
            let errors = validation::validate(&workspace);
            let findings = structurizr_query::lint(&workspace);
            let failed = !errors.is_empty() || (strict && !findings.is_empty());

            if json {
                let out = serde_json::json!({
                    "valid": errors.is_empty(),
                    "errors": errors.iter().map(|e| serde_json::json!({
                        "code": e.code(),
                        "message": e.to_string(),
                    })).collect::<Vec<_>>(),
                    "lint": findings.iter().map(|f| serde_json::json!({
                        "code": f.code,
                        "elementId": f.element_id,
                        "name": f.name,
                        "message": f.message,
                    })).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                if !errors.is_empty() {
                    eprintln!("Validation errors:");
                    for e in &errors {
                        eprintln!("  - [{}] {}", e.code(), e);
                    }
                }
                if strict && !findings.is_empty() {
                    eprintln!("Lint findings:");
                    for f in &findings {
                        eprintln!("  - [{}] {} (element {})", f.code, f.message, f.element_id);
                    }
                }
                if !failed {
                    println!("✓ Workspace '{}' is valid", workspace.name);
                }
            }
            if failed {
                std::process::exit(1);
            }
        }
        Commands::Render { file, format, output } => {
            let mut workspace = load_workspace(&file)?;
            let generated = structurizr_query::generate_views(&mut workspace)
                .map_err(|e| anyhow::anyhow!("view generation: {}", e))?;
            if !generated.is_empty() {
                println!("Generated views: {}", generated.join(", "));
            }
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
            let mut workspace = load_workspace(&file)?;
            // Materialize generated (`auto`) views so the digest lists the
            // effective view set, matching what render/serve produce.
            if let Err(e) = structurizr_query::generate_views(&mut workspace) {
                eprintln!("warning: view generation failed: {}", e);
            }
            print!("{}", structurizr_query::digest(&workspace));
        }
        Commands::Query { file, expression, json } => {
            let workspace = load_workspace(&file)?;
            let selection = structurizr_query::query(&expression, &workspace)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let names = structurizr_query::element_names(&workspace);
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
        Commands::Docs => {
            print!("{}", include_str!("../../../llms.txt"));
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

