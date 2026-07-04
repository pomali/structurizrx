//! Hand-rolled recursive-descent parser for the §6.2 selector grammar.
//!
//! Grammar (keywords case-insensitive):
//!
//! ```text
//! expr         := or
//! or           := and ( "||" and )*
//! and          := unary ( "&&" unary )*
//! unary        := "!" unary | "(" expr ")" | primary
//! primary      := "*"
//!               | neighborhood
//!               | comparison
//! neighborhood := "->" target [ "->" [ INT ] ]
//! comparison   := ("element" | "relationship") "." path ("==" | "!=") value
//! path         := IDENT [ "^" | ("." IDENT) ]
//! value/target := Word | QuotedString | Int
//! ```

use crate::lexer::{tokenize, Spanned, Token};
use crate::{CompOp, Expr, QueryError};

// ---------------------------------------------------------------------------
// Valid paths
// ---------------------------------------------------------------------------

const ELEMENT_PATHS: &[&str] = &[
    "tag",
    "kind",
    "status",
    "layer",
    "parent",
    "parent^",
    "technology",
    "perspective",
    "name",
    "property",
];

const RELATIONSHIP_PATHS: &[&str] = &["kind", "status", "tag", "perspective", "property"];

fn validate_element_path(path: &[String]) -> Result<(), QueryError> {
    let first = path[0].as_str();
    if !ELEMENT_PATHS.contains(&first) {
        return Err(QueryError::UnknownPath {
            kind: "element",
            path: path.join("."),
            valid: ELEMENT_PATHS.join(", "),
        });
    }
    Ok(())
}

fn validate_relationship_path(path: &[String]) -> Result<(), QueryError> {
    let first = path[0].as_str();
    if !RELATIONSHIP_PATHS.contains(&first) {
        return Err(QueryError::UnknownPath {
            kind: "relationship",
            path: path.join("."),
            valid: RELATIONSHIP_PATHS.join(", "),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<Spanned>,
    /// Current position; tokens[pos] is always valid (last element is Eof).
    pos: usize,
}

impl Parser {
    fn new(src: &str) -> Result<Self, QueryError> {
        let tokens =
            tokenize(src).map_err(|e| QueryError::Parse { offset: 0, message: e })?;
        Ok(Self { tokens, pos: 0 })
    }

    /// Peek at the current token without consuming it.
    fn peek(&self) -> &Token {
        &self.tokens[self.pos].token
    }

    /// Byte offset of the current token in the source string.
    fn offset(&self) -> usize {
        self.tokens[self.pos].offset
    }

    /// Consume and return a clone of the current token, advancing the position.
    fn eat(&mut self) -> Token {
        let tok = self.tokens[self.pos].token.clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    // --- grammar productions -----------------------------------------------

    fn parse_expr(&mut self) -> Result<Expr, QueryError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, QueryError> {
        let mut left = self.parse_and()?;
        while *self.peek() == Token::Or {
            self.eat();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, QueryError> {
        let mut left = self.parse_unary()?;
        while *self.peek() == Token::And {
            self.eat();
            let right = self.parse_unary()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, QueryError> {
        match self.peek().clone() {
            Token::Not => {
                self.eat();
                let inner = self.parse_unary()?;
                Ok(Expr::Not(Box::new(inner)))
            }
            Token::LParen => {
                self.eat();
                let inner = self.parse_expr()?;
                if *self.peek() != Token::RParen {
                    return Err(QueryError::Parse {
                        offset: self.offset(),
                        message: format!(
                            "expected `)` to close parenthesized expression, found `{:?}`",
                            self.peek()
                        ),
                    });
                }
                self.eat();
                Ok(inner)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, QueryError> {
        match self.peek().clone() {
            Token::Star => {
                self.eat();
                Ok(Expr::Star)
            }
            Token::Arrow => self.parse_neighborhood(),
            Token::Word(ref w)
                if w.eq_ignore_ascii_case("element")
                    || w.eq_ignore_ascii_case("relationship") =>
            {
                self.parse_comparison()
            }
            _ => Err(QueryError::Parse {
                offset: self.offset(),
                message: format!(
                    "expected `*`, `->`, `element.`, `relationship.`, `!`, or `(`; \
                     found `{:?}`",
                    self.peek()
                ),
            }),
        }
    }

    // neighborhood := "->" target [ "->" [ INT ] ]
    fn parse_neighborhood(&mut self) -> Result<Expr, QueryError> {
        self.eat(); // consume leading `->`
        let target = self.parse_value("neighborhood target")?;
        let depth = if *self.peek() == Token::Arrow {
            self.eat(); // consume second `->`
            match self.peek().clone() {
                Token::Int(n) => {
                    self.eat();
                    n
                }
                _ => 1,
            }
        } else {
            1
        };
        Ok(Expr::Neighborhood { target, depth })
    }

    // comparison := ("element"|"relationship") "." path ("=="|"!=") value
    fn parse_comparison(&mut self) -> Result<Expr, QueryError> {
        let subject = match self.eat() {
            Token::Word(w) => w,
            _ => unreachable!(),
        };
        let is_element = subject.eq_ignore_ascii_case("element");

        // Mandatory "." between subject and path
        if *self.peek() != Token::Dot {
            return Err(QueryError::Parse {
                offset: self.offset(),
                message: format!(
                    "expected `.` after `{subject}`, found `{:?}`",
                    self.peek()
                ),
            });
        }
        self.eat();

        let path = self.parse_path()?;

        if is_element {
            validate_element_path(&path)?;
        } else {
            validate_relationship_path(&path)?;
        }

        let op = match self.peek().clone() {
            Token::Eq => {
                self.eat();
                CompOp::Eq
            }
            Token::Ne => {
                self.eat();
                CompOp::Ne
            }
            tok => {
                return Err(QueryError::Parse {
                    offset: self.offset(),
                    message: format!("expected `==` or `!=`, found `{tok:?}`"),
                })
            }
        };

        let value = self.parse_value("comparison value")?;

        if is_element {
            Ok(Expr::ElementComparison { path, op, value })
        } else {
            Ok(Expr::RelationshipComparison { path, op, value })
        }
    }

    /// Parse a path component: `tag`, `kind`, `parent`, `parent^`,
    /// `property.<key>`, etc.  The first identifier is normalised to lowercase.
    fn parse_path(&mut self) -> Result<Vec<String>, QueryError> {
        let first = match self.peek().clone() {
            Token::Word(w) => {
                self.eat();
                w.to_lowercase()
            }
            tok => {
                return Err(QueryError::Parse {
                    offset: self.offset(),
                    message: format!("expected path identifier, found `{tok:?}`"),
                })
            }
        };

        // `parent^` — the caret is a separate token that makes this a transitive match
        if first == "parent" && *self.peek() == Token::Caret {
            self.eat();
            return Ok(vec!["parent^".to_string()]);
        }

        // `property.<key>` — a two-segment path
        if first == "property" && *self.peek() == Token::Dot {
            self.eat(); // consume '.'
            let key = match self.peek().clone() {
                Token::Word(k) => {
                    self.eat();
                    k // property keys are case-sensitive
                }
                tok => {
                    return Err(QueryError::Parse {
                        offset: self.offset(),
                        message: format!(
                            "expected property key after `property.`, found `{tok:?}`"
                        ),
                    })
                }
            };
            return Ok(vec!["property".to_string(), key]);
        }

        Ok(vec![first])
    }

    /// Parse a bare word, double-quoted string, or integer as a value.
    fn parse_value(&mut self, context: &str) -> Result<String, QueryError> {
        match self.peek().clone() {
            Token::Word(w) => {
                self.eat();
                Ok(w)
            }
            Token::Quoted(s) => {
                self.eat();
                Ok(s)
            }
            Token::Int(n) => {
                self.eat();
                Ok(n.to_string())
            }
            tok => Err(QueryError::Parse {
                offset: self.offset(),
                message: format!(
                    "expected word or quoted string as {context}, found `{tok:?}`"
                ),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse a selector expression string into an [`Expr`] AST.
pub fn parse(src: &str) -> Result<Expr, QueryError> {
    let mut p = Parser::new(src)?;
    let expr = p.parse_expr()?;
    if *p.peek() != Token::Eof {
        return Err(QueryError::Parse {
            offset: p.offset(),
            message: format!(
                "unexpected token after expression: `{:?}`",
                p.peek()
            ),
        });
    }
    Ok(expr)
}
