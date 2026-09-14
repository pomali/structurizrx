//! Workspace path resolution.
//!
//! Accepts a file path or directory and discovers workspace files.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use structurizr_dsl::SourceLocations;
use structurizr_model::Workspace;

/// A discovered workspace with its display name and parsed content.
#[derive(Clone, Debug)]
pub struct WorkspaceEntry {
    /// Slug-friendly name derived from the file or directory name.
    pub name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Parsed workspace.
    pub workspace: Workspace,
    /// Where each element, relationship and view is declared; `None` for a
    /// `.json` workspace.
    pub locations: Option<SourceLocations>,
    /// Source path that was parsed (used for reloading).
    pub source_path: PathBuf,
}

/// Resolve a path into one or more workspace entries.
///
/// Rules:
/// - File (`.dsl` / `.json`) → single workspace
/// - Directory that directly contains a workspace file → single workspace  
/// - Directory whose sub-directories each contain a workspace file → multiple workspaces
pub fn resolve(path: &Path) -> Result<Vec<WorkspaceEntry>> {
    if path.is_file() {
        let entry = load_entry(path)?;
        return Ok(vec![entry]);
    }

    if path.is_dir() {
        // Check if this directory directly contains a workspace file
        if let Some(ws_file) = find_workspace_file_in(path) {
            let entry = load_entry(&ws_file)?;
            return Ok(vec![entry]);
        }

        // Otherwise collect workspaces from sub-directories
        let mut entries = Vec::new();
        let mut sub_dirs: Vec<_> = std::fs::read_dir(path)
            .with_context(|| format!("Cannot read directory {}", path.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        sub_dirs.sort();

        for sub in &sub_dirs {
            if let Some(ws_file) = find_workspace_file_in(sub) {
                match load_entry(&ws_file) {
                    Ok(entry) => entries.push(entry),
                    Err(e) => eprintln!("Warning: skipping {}: {}", ws_file.display(), e),
                }
            }
        }

        if !entries.is_empty() {
            return Ok(entries);
        }
    }

    anyhow::bail!(
        "No workspace file found at {}. Expected a .dsl or .json file, or a directory containing one.",
        path.display()
    )
}

/// Attempt to find a `.dsl` or `.json` workspace file directly inside `dir`.
fn find_workspace_file_in(dir: &Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_lowercase();
                if ext == "dsl" || ext == "json" {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Load a workspace from a `.dsl` or `.json` file and produce a [`WorkspaceEntry`].
fn load_entry(path: &Path) -> Result<WorkspaceEntry> {
    let (workspace, locations) = load_workspace(path)?;

    let display_name = workspace.name.clone();

    // Derive a slug from the file stem or parent directory name
    let name = slug_from_path(path);

    Ok(WorkspaceEntry {
        name,
        display_name,
        source_path: path.to_path_buf(),
        workspace,
        locations,
    })
}

fn load_workspace(path: &Path) -> Result<(Workspace, Option<SourceLocations>)> {
    let (mut ws, locations) = if is_json(path) {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let ws = serde_json::from_str::<Workspace>(&content)
            .with_context(|| format!("Failed to parse JSON from {}", path.display()))?;
        (ws, None)
    } else {
        let parsed = structurizr_dsl::parse_file_detailed(path)
            .with_context(|| format!("Failed to parse DSL from {}", path.display()))?;
        (parsed.workspace, Some(parsed.locations))
    };

    materialize_views(&mut ws, &path.display().to_string());
    Ok((ws, locations))
}

/// True when `path` names a JSON workspace rather than a DSL one.
pub fn is_json(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

/// Parse a workspace out of source text that is not on disk — a past revision
/// read from git, say.
///
/// The result goes through the same view generation as a workspace loaded from
/// a file, so a comparison between two versions is a comparison of like with
/// like: without it, every generated view would read as removed the moment one
/// side came from git.
///
/// `label` names the source in warnings only.
pub fn workspace_from_source(content: &str, json: bool, label: &str) -> Result<Workspace> {
    let mut ws = if json {
        serde_json::from_str::<Workspace>(content)
            .with_context(|| format!("Failed to parse JSON from {label}"))?
    } else {
        structurizr_dsl::parse_str(content)
            .with_context(|| format!("Failed to parse DSL from {label}"))?
    };

    materialize_views(&mut ws, label);
    Ok(ws)
}

/// Materialize `auto` views (and the zero-config default set) so the browser
/// lists generated diagrams alongside hand-authored ones.
fn materialize_views(ws: &mut Workspace, label: &str) {
    if let Err(e) = structurizr_query::generate_views(ws) {
        eprintln!("warning: view generation failed for {label}: {e}");
    }
}

/// The slug `serve` gives a workspace file: its stem, or its directory's name
/// when the file is called `workspace.*`.
pub fn slug_from_path(path: &Path) -> String {
    // Prefer parent directory name for files called workspace.*
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace");

    let raw = if stem.to_lowercase() == "workspace" {
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or(stem)
    } else {
        stem
    };

    to_slug(raw)
}

fn to_slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
