use std::path::PathBuf;

use thiserror::Error;

/// Error during DSL parsing.
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("parse error at line {line}, column {col}{}: {message}", file_suffix(.file))]
    Syntax {
        /// The file the error is in, when it is not the entry file (an
        /// `!include`d part), as written in the `!include` statement.
        file: Option<String>,
        line: usize,
        col: usize,
        message: String,
    },
    #[error("undefined identifier: {0}")]
    UndefinedIdentifier(String),
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Several independent errors found in one parse: the parser recovers at
    /// statement boundaries so a file with three mistakes reports all three.
    #[error("{} parse errors:\n  {}", .0.len(), join_lines(.0))]
    Multiple(Vec<ParseError>),
}

fn file_suffix(file: &Option<String>) -> String {
    match file {
        Some(f) => format!(" in {}", f),
        None => String::new(),
    }
}

fn join_lines(errors: &[ParseError]) -> String {
    errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// One parse error as structured data, for `validate --json` and editors.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// Stable machine-readable code: `unknown-identifier`, `unknown-keyword`,
    /// `unexpected-token`, `unclosed-block`, `include`, `io` or `syntax`.
    pub code: &'static str,
    /// The file the error is in, when known. For an error in an `!include`d
    /// part this is the path as written in the `!include` statement; the
    /// caller knows the entry file and can join it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// 1-based line, 0 when the error has no position.
    pub line: usize,
    /// 1-based column, 0 when the error has no position.
    pub column: usize,
    pub message: String,
}

impl ParseError {
    pub fn syntax(line: usize, col: usize, message: impl Into<String>) -> Self {
        ParseError::Syntax {
            file: None,
            line,
            col,
            message: message.into(),
        }
    }

    /// Attach the `!include`d file an error was found in.
    pub fn in_file(self, file: Option<String>) -> Self {
        match self {
            ParseError::Syntax {
                line, col, message, ..
            } => ParseError::Syntax {
                file,
                line,
                col,
                message,
            },
            other => other,
        }
    }

    /// Every error in this value, flattening [`ParseError::Multiple`].
    pub fn errors(&self) -> Vec<&ParseError> {
        match self {
            ParseError::Multiple(list) => list.iter().flat_map(|e| e.errors()).collect(),
            other => vec![other],
        }
    }

    /// Stable machine-readable code for this error (see [`Diagnostic::code`]).
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::Syntax { message, .. } => {
                let m = message.as_str();
                if m.starts_with("unknown element identifier")
                    || m.starts_with("unknown identifier")
                    || m.starts_with("unresolved identifier")
                {
                    "unknown-identifier"
                } else if m.starts_with("unknown keyword") || m.contains("cannot be declared at") {
                    "unknown-keyword"
                } else if m.starts_with("unexpected end of input") {
                    "unclosed-block"
                } else if m.starts_with("cannot read !include") || m.starts_with("include depth") {
                    "include"
                } else if m.starts_with("unexpected ") || m.starts_with("expected ") {
                    "unexpected-token"
                } else {
                    "syntax"
                }
            }
            ParseError::UndefinedIdentifier(_) => "unknown-identifier",
            ParseError::UnexpectedEof => "unclosed-block",
            ParseError::Io(_) => "io",
            ParseError::Multiple(_) => "multiple",
        }
    }

    /// This error as structured diagnostics, one per underlying error.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.errors()
            .into_iter()
            .map(|e| match e {
                ParseError::Syntax {
                    file,
                    line,
                    col,
                    message,
                } => Diagnostic {
                    code: e.code(),
                    file: file.clone(),
                    line: *line,
                    column: *col,
                    message: message.clone(),
                },
                other => Diagnostic {
                    code: other.code(),
                    file: None,
                    line: 0,
                    column: 0,
                    message: other.to_string(),
                },
            })
            .collect()
    }

    /// The file an included-file error names, joined onto `entry`'s
    /// directory, for tooling that wants an absolute path.
    pub fn resolve_file(diagnostic: &Diagnostic, entry: &std::path::Path) -> PathBuf {
        match &diagnostic.file {
            Some(f) => entry
                .parent()
                .map(|d| d.join(f))
                .unwrap_or_else(|| PathBuf::from(f)),
            None => entry.to_path_buf(),
        }
    }
}
