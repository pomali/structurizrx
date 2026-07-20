//! Per-document state the backend keeps between LSP notifications.

use std::collections::HashMap;

use structurizr_dsl::lexer::{tokenize, Pos, Spanned};
use structurizr_dsl::{parse_str_with_identifiers, IdentifierRegister};
use structurizr_model::Workspace;
use tower_lsp_server::ls_types::Diagnostic;

use crate::diagnostics;
use crate::index::{self, Declarations};

/// The result of the last *successful* parse. Kept around across edits that
/// introduce a syntax error, so hover/completion/go-to-definition keep
/// working off the last good version instead of going blank mid-edit.
pub struct Analyzed {
    pub workspace: Workspace,
    pub identifiers: IdentifierRegister,
    /// Element id -> declaration position, derived by joining `identifiers`
    /// (DSL identifier -> element id) with `declarations` (DSL identifier ->
    /// position).
    pub id_to_pos: HashMap<String, Pos>,
}

pub struct DocumentState {
    pub text: String,
    pub tokens: Vec<Spanned>,
    pub declarations: Declarations,
    pub last_ok: Option<Analyzed>,
}

impl DocumentState {
    pub fn empty() -> Self {
        DocumentState {
            text: String::new(),
            tokens: Vec::new(),
            declarations: Declarations::new(),
            last_ok: None,
        }
    }

    /// Re-tokenizes and re-parses `text`, updating all derived state, and
    /// returns the diagnostics to publish for it.
    pub fn update(&mut self, text: String) -> Vec<Diagnostic> {
        self.tokens = tokenize(&text);
        self.declarations = index::build_declarations(&self.tokens);
        self.text = text;

        match parse_str_with_identifiers(&self.text) {
            Ok((workspace, identifiers)) => {
                let id_to_pos: HashMap<String, Pos> = identifiers
                    .identifiers
                    .iter()
                    .filter_map(|(ident, (id, _kind))| {
                        self.declarations.get(ident).map(|pos| (id.clone(), *pos))
                    })
                    .collect();
                let diags = diagnostics::validation_diagnostics(&workspace, &id_to_pos);
                self.last_ok = Some(Analyzed {
                    workspace,
                    identifiers,
                    id_to_pos,
                });
                diags
            }
            Err(err) => vec![diagnostics::syntax_error(&self.text, &err)],
        }
    }

    /// The `Word` token whose span contains `pos`, if any.
    pub fn word_at(&self, pos: Pos) -> Option<&str> {
        self.tokens.iter().find_map(|t| {
            let structurizr_dsl::lexer::Token::Word(w) = &t.token else {
                return None;
            };
            if t.pos.line != pos.line {
                return None;
            }
            let start = t.pos.col;
            let end = start + w.chars().count();
            (start..end).contains(&pos.col).then_some(w.as_str())
        })
    }
}
