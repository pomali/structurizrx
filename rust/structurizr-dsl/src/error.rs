use thiserror::Error;

/// Error during DSL parsing.
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("parse error at line {line}, column {col}: {message}")]
    Syntax {
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
}

impl ParseError {
    pub fn syntax(line: usize, col: usize, message: impl Into<String>) -> Self {
        ParseError::Syntax {
            line,
            col,
            message: message.into(),
        }
    }
}
