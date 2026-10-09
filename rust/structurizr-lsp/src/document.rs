//! Per-document state the backend keeps between LSP notifications.

use std::collections::HashMap;
use std::path::PathBuf;

use ls_types::Diagnostic;
use structurizr_dsl::lexer::{tokenize, Pos, Spanned};
use structurizr_dsl::{IdentifierRegister, Parsed, SourceLocation, SourceLocations};
use structurizr_model::Workspace;

use crate::diagnostics;
use crate::index::{self, Declarations};

/// The result of the last *successful* parse. Kept around across edits that
/// introduce a syntax error, so hover/completion/go-to-definition keep
/// working off the last good version instead of going blank mid-edit.
pub struct Analyzed {
    pub workspace: Workspace,
    pub identifiers: IdentifierRegister,
    /// Where every element, port, relationship and view was declared — in
    /// this document or in a file it `!include`s.
    pub locations: SourceLocations,
}

pub struct DocumentState {
    pub text: String,
    /// The file this document is, when the server can read around it:
    /// `!include`s resolve against its directory. `None` for non-`file:`
    /// documents and in the WASM build, which has no filesystem.
    pub path: Option<PathBuf>,
    pub tokens: Vec<Spanned>,
    pub declarations: Declarations,
    pub last_ok: Option<Analyzed>,
}

impl DocumentState {
    pub fn empty() -> Self {
        DocumentState {
            text: String::new(),
            path: None,
            tokens: Vec::new(),
            declarations: Declarations::new(),
            last_ok: None,
        }
    }

    /// Re-tokenizes and re-parses `text` (the contents of `path`, when known),
    /// updating all derived state, and returns the diagnostics to publish.
    pub fn update(&mut self, text: String, path: Option<PathBuf>) -> Vec<Diagnostic> {
        self.tokens = tokenize(&text);
        self.declarations = index::build_declarations(&self.tokens);
        self.text = text;
        self.path = path;

        let parsed = match &self.path {
            Some(path) => structurizr_dsl::parse_str_detailed_at(&self.text, path),
            None => structurizr_dsl::parse_str_detailed(&self.text),
        };
        match parsed {
            Ok(Parsed {
                workspace,
                identifiers,
                locations,
            }) => {
                // Model id -> declaration position in this document (anonymous
                // elements and relationships included), to anchor validation
                // diagnostics.
                let id_to_pos: HashMap<String, Pos> = locations
                    .iter()
                    .filter(|(_, location)| self.is_here(location))
                    .map(|(id, location)| {
                        (
                            id.to_string(),
                            Pos {
                                line: location.line,
                                col: location.col,
                            },
                        )
                    })
                    .collect();
                let diags = diagnostics::validation_diagnostics(&workspace, &id_to_pos);
                self.last_ok = Some(Analyzed {
                    workspace,
                    identifiers,
                    locations,
                });
                diags
            }
            Err(err) => diagnostics::syntax_errors(&self.text, &err),
        }
    }

    /// Whether `location` is in this document rather than in a file it includes.
    pub fn is_here(&self, location: &SourceLocation) -> bool {
        match (&location.file, &self.path) {
            (None, _) => true,
            (Some(file), Some(path)) => file == path,
            (Some(_), None) => false,
        }
    }

    /// The `Word` token whose span contains `pos`, if any.
    ///
    /// The end of the span counts as inside it: editors report the caret
    /// *after* the word you double-clicked or right-clicked at its end, and
    /// rename in particular is unusable if that misses.
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
            (start..=end).contains(&pos.col).then_some(w.as_str())
        })
    }
}
