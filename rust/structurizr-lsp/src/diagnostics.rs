//! Turns parse/validation results into LSP `Diagnostic`s.

use std::collections::HashMap;

use ls_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use structurizr_dsl::lexer::Pos;
use structurizr_dsl::ParseError;
use structurizr_model::validation;
use structurizr_model::Workspace;

use crate::convert::line_range;

/// One diagnostic per parse error: the parser recovers at statement
/// boundaries, so a single parse can carry several.
pub fn syntax_errors(text: &str, err: &ParseError) -> Vec<Diagnostic> {
    err.errors()
        .into_iter()
        .map(|e| syntax_error(text, e))
        .collect()
}

pub fn syntax_error(text: &str, err: &ParseError) -> Diagnostic {
    let range = match err {
        ParseError::Syntax {
            file, line, col, ..
        } => {
            // An error inside an `!include`d file carries that file's line
            // number; mark the `!include` in this document instead.
            let pos = file
                .as_deref()
                .and_then(|file| include_line(text, file))
                .unwrap_or(Pos {
                    line: *line,
                    col: *col,
                });
            line_range(text, pos)
        }
        _ => Range::new(Position::new(0, 0), Position::new(0, 1)),
    };
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("structurizr-dsl".to_string()),
        code: Some(ls_types::NumberOrString::String(err.code().to_string())),
        message: err.to_string(),
        ..Diagnostic::default()
    }
}

/// Position of the `!include <file>` line in `text`. Only direct includes are
/// found: a nested include's label is relative to the file that includes it.
fn include_line(text: &str, file: &str) -> Option<Pos> {
    text.lines().enumerate().find_map(|(i, line)| {
        let trimmed = line.trim_start();
        let rest = trimmed
            .strip_prefix("!include ")
            .or_else(|| trimmed.strip_prefix("!INCLUDE "))?;
        (rest.trim().trim_matches('"') == file).then(|| Pos {
            line: i + 1,
            col: line.len() - trimmed.len() + 1,
        })
    })
}

/// `ValidationError`'s `Display` messages quote the offending element id
/// (e.g. `"source 'sys1' of relationship 'r1' does not exist in the model"`)
/// but don't expose it as a structured field. Rather than re-parsing the
/// message format (which `thiserror` owns and could change wording on),
/// we search for any known element id quoted in the message — good enough to
/// anchor a diagnostic without depending on exact message shape.
fn resolve_position(message: &str, id_to_pos: &HashMap<String, Pos>) -> Option<Pos> {
    id_to_pos
        .iter()
        .find(|(id, _)| message.contains(&format!("'{}'", id.as_str())))
        .map(|(_, pos)| *pos)
}

/// Runs `structurizr_model::validation::validate` and maps each error to a
/// position via `id_to_pos` (element id -> declaration position) when
/// possible. Errors with no resolvable position fall back to the top of the
/// document — a known v1 limitation, since `ValidationError` carries no span.
pub fn validation_diagnostics(
    workspace: &Workspace,
    id_to_pos: &HashMap<String, Pos>,
) -> Vec<Diagnostic> {
    validation::validate(workspace)
        .into_iter()
        .map(|err| {
            let message = err.to_string();
            let pos = resolve_position(&message, id_to_pos);
            let range = match pos {
                Some(pos) => Range::new(
                    Position::new(
                        (pos.line.saturating_sub(1)) as u32,
                        (pos.col.saturating_sub(1)) as u32,
                    ),
                    Position::new((pos.line.saturating_sub(1)) as u32, u32::MAX),
                ),
                None => Range::new(Position::new(0, 0), Position::new(0, 1)),
            };
            Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("structurizr-model".to_string()),
                code: Some(ls_types::NumberOrString::String(err.code().to_string())),
                message,
                ..Diagnostic::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_error_lands_on_reported_line() {
        let text = "workspace {\n  model {\n    x\n";
        let err = ParseError::syntax(3, 5, "unexpected token".to_string());
        let diag = syntax_error(text, &err);
        assert_eq!(diag.range.start.line, 2);
        assert_eq!(diag.range.start.character, 4);
        assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn validation_error_falls_back_without_position() {
        let workspace_json = r#"{"name":"","model":{},"views":{}}"#;
        let workspace: Workspace = serde_json::from_str(workspace_json).unwrap();
        let diags = validation_diagnostics(&workspace, &HashMap::new());
        assert!(diags.iter().any(|d| d.message.contains("empty")));
    }
}
