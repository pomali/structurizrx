//! Turns parse/validation results into LSP `Diagnostic`s.

use std::collections::HashMap;

use structurizr_dsl::lexer::Pos;
use structurizr_dsl::ParseError;
use structurizr_model::validation;
use structurizr_model::Workspace;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::convert::line_range;

pub fn syntax_error(text: &str, err: &ParseError) -> Diagnostic {
    let range = match err {
        ParseError::Syntax { line, col, .. } => line_range(
            text,
            Pos {
                line: *line,
                col: *col,
            },
        ),
        _ => Range::new(Position::new(0, 0), Position::new(0, 1)),
    };
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("structurizr-dsl".to_string()),
        message: err.to_string(),
        ..Diagnostic::default()
    }
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
                code: Some(tower_lsp_server::ls_types::NumberOrString::String(
                    err.code().to_string(),
                )),
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
