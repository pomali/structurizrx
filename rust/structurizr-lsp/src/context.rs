//! Works out *where in the grammar* the cursor is, so completion can offer the
//! keywords that are actually legal there.
//!
//! The parser is single-pass and consumes its token stream, so there is no
//! syntax tree to walk. Instead this re-scans the token stream up to the cursor
//! and tracks the brace stack — enough to name the enclosing block, which is
//! the only thing `keyword_sets()` is keyed by.

use structurizr_dsl::lexer::{Pos, Spanned, Token};

/// The grammatical position of the cursor.
#[derive(Debug, Default, PartialEq)]
pub struct CompletionContext {
    /// Lowercased keyword of the innermost enclosing block, e.g. `model` or
    /// `softwaresystem`. `None` at the top level of the file.
    pub block: Option<String>,
    /// The cursor directly follows a `->`, so the only sensible completion is
    /// the destination element.
    pub after_arrow: bool,
}

impl CompletionContext {
    /// The `keyword_sets()` key whose keywords are legal in this block, or
    /// `None` when we don't recognise the block and should fall back to
    /// offering everything rather than wrongly offering nothing.
    pub fn keyword_set(&self) -> Option<&'static str> {
        let block = self.block.as_deref()?;
        Some(match block {
            "workspace" => "workspace",
            "views" => "views",
            // A group/enterprise/environment body holds the same declarations
            // as `model` itself.
            "model" | "group" | "enterprise" | "deploymentenvironment" => "model",
            "softwaresystem" => "softwareSystem",
            "container" => "container",
            "deploymentnode" => "deploymentNode",
            "person" | "component" | "infrastructurenode" | "containerinstance"
            | "softwaresysteminstance" | "element" => "element",
            _ => return None,
        })
    }

    /// Whether element identifiers are worth offering here. They are noise in
    /// the `workspace` body (which takes only keywords) and at the top level.
    pub fn wants_identifiers(&self) -> bool {
        if self.after_arrow {
            return true;
        }
        match self.block.as_deref() {
            None | Some("workspace") => false,
            Some(_) => true,
        }
    }
}

/// Scans `tokens` up to `cursor` and reports the enclosing block.
pub fn context_at(tokens: &[Spanned], cursor: Pos) -> CompletionContext {
    let mut stack: Vec<String> = Vec::new();
    // The most recent word on the current statement — for `web = softwareSystem
    // "Web" {` that is `softwareSystem`, which is what names the block.
    let mut last_word: Option<String> = None;
    let mut statement_has_arrow = false;
    let mut last_token: Option<&Token> = None;
    let mut last_line = 0usize;

    for t in tokens.iter().take_while(|t| before(t.pos, cursor)) {
        // A relationship is a single-line statement; a stale arrow from an
        // earlier line must not make the next block look like a relationship.
        if t.pos.line != last_line {
            statement_has_arrow = false;
            last_line = t.pos.line;
        }
        match &t.token {
            Token::OpenBrace => {
                let name = if statement_has_arrow {
                    "relationship".to_string()
                } else {
                    last_word.take().unwrap_or_default()
                };
                stack.push(name);
                last_word = None;
                statement_has_arrow = false;
            }
            Token::CloseBrace => {
                stack.pop();
                last_word = None;
                statement_has_arrow = false;
            }
            Token::Word(w) => last_word = Some(w.to_lowercase()),
            Token::Arrow => statement_has_arrow = true,
            _ => {}
        }
        last_token = Some(&t.token);
    }

    CompletionContext {
        block: stack.pop(),
        after_arrow: matches!(last_token, Some(Token::Arrow)),
    }
}

fn before(pos: Pos, cursor: Pos) -> bool {
    (pos.line, pos.col) < (cursor.line, cursor.col)
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_dsl::lexer::tokenize;

    const DSL: &str = r#"workspace "w" {
    model {
        user = person "User"
        web = softwareSystem "Web" {
            api = container "API" {

            }
        }
        user ->
    }
    views {

    }
}
"#;

    fn at(line: usize, col: usize) -> CompletionContext {
        context_at(&tokenize(DSL), Pos { line, col })
    }

    #[test]
    fn inside_model_offers_model_keywords() {
        assert_eq!(at(3, 9).keyword_set(), Some("model"));
    }

    #[test]
    fn inside_software_system_offers_its_own_keywords() {
        assert_eq!(at(5, 13).keyword_set(), Some("softwareSystem"));
    }

    #[test]
    fn inside_container_offers_container_keywords() {
        assert_eq!(at(6, 17).keyword_set(), Some("container"));
    }

    #[test]
    fn inside_views_offers_view_keywords() {
        assert_eq!(at(12, 9).keyword_set(), Some("views"));
    }

    #[test]
    fn top_level_has_no_block() {
        assert_eq!(context_at(&tokenize(DSL), Pos { line: 1, col: 1 }).block, None);
    }

    #[test]
    fn after_an_arrow_wants_identifiers() {
        let ctx = at(9, 17);
        assert!(ctx.after_arrow);
        assert!(ctx.wants_identifiers());
    }

    #[test]
    fn workspace_body_does_not_want_identifiers() {
        let ctx = context_at(&tokenize("workspace \"w\" {\n    \n}\n"), Pos { line: 2, col: 5 });
        assert_eq!(ctx.keyword_set(), Some("workspace"));
        assert!(!ctx.wants_identifiers());
    }

    #[test]
    fn unknown_block_falls_back_to_everything() {
        let ctx = context_at(
            &tokenize("workspace {\n views {\n  styles {\n   \n  }\n }\n}"),
            Pos { line: 4, col: 4 },
        );
        assert_eq!(ctx.block.as_deref(), Some("styles"));
        assert_eq!(ctx.keyword_set(), None);
    }
}
