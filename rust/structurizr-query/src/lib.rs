//! Selector expression engine for StructurizrX (spec §6.2).
//!
//! # Usage
//!
//! ```ignore
//! use structurizr_query::{query, Selection};
//! let sel = query("element.kind==container", &workspace)?;
//! for id in &sel.elements { println!("{id}"); }
//! ```

pub mod lexer;
mod parser;
mod eval;

use std::collections::BTreeSet;
use structurizr_model::Workspace;

/// A set of matched elements and relationships in deterministic (BTreeSet) order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Matched element ids in lexicographic order.
    pub elements: BTreeSet<String>,
    /// Matched relationship ids in lexicographic order.
    pub relationships: BTreeSet<String>,
}

/// Errors from parsing or evaluating a selector expression.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// A syntax error; `offset` is the byte offset in the original string.
    #[error("parse error at offset {offset}: {message}")]
    Parse { offset: usize, message: String },

    /// A comparison path that is not in the valid set.
    ///
    /// The message names the valid paths so LLM agents can self-correct.
    #[error("unknown path '{path}' for {kind}; valid paths are: {valid}")]
    UnknownPath {
        kind: &'static str,
        path: String,
        valid: String,
    },

    /// A neighborhood target identifier that cannot be resolved to any element.
    #[error("unknown neighborhood target '{0}'")]
    UnknownTarget(String),
}

/// Comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompOp {
    Eq,
    Ne,
}

/// AST node for a selector expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// `*` — all elements and all relationships.
    Star,
    /// Neighborhood: BFS from `target` to `depth` hops (both directions).
    Neighborhood { target: String, depth: u32 },
    /// Element field comparison (path, op, value).
    ElementComparison { path: Vec<String>, op: CompOp, value: String },
    /// Relationship field comparison (path, op, value).
    RelationshipComparison { path: Vec<String>, op: CompOp, value: String },
    /// Logical AND — intersects both element and relationship sets.
    And(Box<Expr>, Box<Expr>),
    /// Logical OR — unions both element and relationship sets.
    Or(Box<Expr>, Box<Expr>),
    /// Logical NOT — complements both sets against the full model universes.
    Not(Box<Expr>),
}

/// Parse a selector expression into an AST.
///
/// Keywords (`element`, `relationship`) are case-insensitive.
/// Comparison paths are validated at parse time; an unknown path returns
/// [`QueryError::UnknownPath`] with the list of valid alternatives.
pub fn parse(expr: &str) -> Result<Expr, QueryError> {
    parser::parse(expr)
}

/// Evaluate a parsed expression against a workspace.
pub fn eval(expr: &Expr, workspace: &Workspace) -> Result<Selection, QueryError> {
    eval::eval(expr, workspace)
}

/// Parse and evaluate a selector expression in one step.
pub fn query(expr: &str, workspace: &Workspace) -> Result<Selection, QueryError> {
    let ast = parse(expr)?;
    eval(&ast, workspace)
}
