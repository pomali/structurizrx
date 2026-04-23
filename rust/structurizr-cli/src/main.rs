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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { file } => {
            let workspace = load_workspace(&file)?;
            let errors = validation::validate(&workspace);
            if errors.is_empty() {
                println!("✓ Workspace '{}' is valid", workspace.name);
            } else {
                eprintln!("Validation errors:");
                for e in &errors {
                    eprintln!("  - {}", e);
                }
                std::process::exit(1);
            }
        }
        Commands::Render { file, format, output } => {
            let workspace = load_workspace(&file)?;
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
    }

    Ok(())
}
