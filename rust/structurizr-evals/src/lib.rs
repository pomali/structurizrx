//! Deterministic rubric grader for LLM-agent-workflow eval tasks.
//!
//! A task is a natural-language modeling prompt plus a rubric. Grading a
//! candidate `.dsl` against its rubric reuses exactly the tool feedback an
//! agent following `skills/structurizrx/SKILL.md` already has access to:
//! `structurizr_model::validation::validate`, `structurizr_query::lint`,
//! `structurizr_query::query`, and `structurizr_query::digest`. No live LLM
//! calls happen here — this crate only grades a `.dsl` that already exists
//! (hand-written or produced by an agent) against a task's rubric.

use serde::Deserialize;
use structurizr_model::{validation, Workspace};

/// One eval task: a prompt to hand an agent, plus the rubric its output is
/// graded against.
#[derive(Debug, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    /// The natural-language modeling task, exactly as it should be handed
    /// to an agent (with no other context beyond the `structurizrx` skill).
    pub prompt: String,
    /// Path to the candidate `.dsl`, relative to this crate's root.
    pub candidate: String,
    pub rubric: Rubric,
}

/// A rubric: every check is optional and empty/false means "not checked."
#[derive(Debug, Deserialize)]
pub struct Rubric {
    /// `validate --strict` must come back clean: zero validation errors and
    /// zero lint findings.
    #[serde(default)]
    pub strict_valid: bool,
    /// Selector expressions (spec §6.2) that must each match at least one
    /// element or relationship.
    #[serde(default)]
    pub required: Vec<String>,
    /// Selector expressions that must match nothing.
    #[serde(default)]
    pub forbidden: Vec<String>,
    /// Substrings the plain-text `digest` output must contain.
    #[serde(default)]
    pub digest_contains: Vec<String>,
}

/// Grade `workspace` against `rubric`. Returns one human-readable failure
/// message per violated rubric line; an empty vec is a full pass.
pub fn grade(workspace: &Workspace, rubric: &Rubric) -> Vec<String> {
    let mut failures = Vec::new();

    if rubric.strict_valid {
        let errors = validation::validate(workspace);
        if !errors.is_empty() {
            failures.push(format!(
                "strict_valid: {} validation error(s): {}",
                errors.len(),
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        let findings = structurizr_query::lint(workspace);
        if !findings.is_empty() {
            failures.push(format!(
                "strict_valid: {} lint finding(s): {}",
                findings.len(),
                findings
                    .iter()
                    .map(|f| format!("{}({})", f.code, f.name))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }

    for selector in &rubric.required {
        match structurizr_query::query(selector, workspace) {
            Ok(sel) if sel.elements.is_empty() && sel.relationships.is_empty() => {
                failures.push(format!(
                    "required: selector `{selector}` matched nothing"
                ));
            }
            Ok(_) => {}
            Err(e) => failures.push(format!(
                "required: selector `{selector}` failed to evaluate: {e}"
            )),
        }
    }

    for selector in &rubric.forbidden {
        match structurizr_query::query(selector, workspace) {
            Ok(sel) if !sel.elements.is_empty() || !sel.relationships.is_empty() => {
                failures.push(format!(
                    "forbidden: selector `{selector}` matched {} element(s) / {} relationship(s), expected none",
                    sel.elements.len(),
                    sel.relationships.len()
                ));
            }
            Ok(_) => {}
            Err(e) => failures.push(format!(
                "forbidden: selector `{selector}` failed to evaluate: {e}"
            )),
        }
    }

    if !rubric.digest_contains.is_empty() {
        let text = structurizr_query::digest(workspace);
        for needle in &rubric.digest_contains {
            if !text.contains(needle.as_str()) {
                failures.push(format!(
                    "digest_contains: digest is missing `{needle}`"
                ));
            }
        }
    }

    failures
}

/// Load every `*.json` task definition in `dir`, sorted by filename so the
/// suite runs (and reports) in a stable, deterministic order.
pub fn load_tasks(dir: &std::path::Path) -> std::io::Result<Vec<Task>> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    paths.sort();

    let mut tasks = Vec::with_capacity(paths.len());
    for path in paths {
        let text = std::fs::read_to_string(&path)?;
        let task: Task = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("invalid task JSON in {}: {e}", path.display()));
        tasks.push(task);
    }
    Ok(tasks)
}
