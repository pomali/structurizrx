//! Maps DSL identifiers (e.g. the `web` in `web = softwareSystem "Web App"`) to
//! their declaration position, by scanning the token stream for
//! `Word(ident) Equals Word(keyword)`.
//!
//! This is a plain token-stream scan rather than a grammar reimplementation:
//! the parser already resolves identifiers to element ids (via
//! `IdentifierRegister`), but doesn't track *where* an identifier was
//! declared. Re-tokenizing here (the lexer is cheap and already public) avoids
//! adding position-tracking to the parser just for tooling's sake.

use std::collections::HashMap;

use structurizr_dsl::lexer::{Pos, Spanned, Token};

/// Keywords that introduce an element/group declaration of the form
/// `ident = keyword ...`. Matched case-insensitively, mirroring the parser.
const DECLARATION_KEYWORDS: &[&str] = &[
    "person",
    "softwaresystem",
    "container",
    "component",
    "deploymentnode",
    "containerinstance",
    "softwaresysteminstance",
    "infrastructurenode",
    "element",
    "group",
    "deploymentenvironment",
];

/// identifier (lowercased) -> position of the identifier token in its
/// `ident = keyword ...` declaration.
pub type Declarations = HashMap<String, Pos>;

pub fn build_declarations(tokens: &[Spanned]) -> Declarations {
    let mut map = Declarations::new();
    for window in tokens.windows(3) {
        let (Token::Word(ident), Token::Equals, Token::Word(keyword)) =
            (&window[0].token, &window[1].token, &window[2].token)
        else {
            continue;
        };
        if DECLARATION_KEYWORDS
            .iter()
            .any(|k| k.eq_ignore_ascii_case(keyword))
        {
            map.entry(ident.to_lowercase()).or_insert(window[0].pos);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use structurizr_dsl::lexer::tokenize;

    #[test]
    fn finds_element_declarations() {
        let tokens = tokenize(
            r#"
            workspace {
                model {
                    user = person "User"
                    web = softwareSystem "Web App" {
                        api = container "API"
                    }
                }
            }
            "#,
        );
        let decls = build_declarations(&tokens);
        assert_eq!(decls.get("user").map(|p| p.line), Some(4));
        assert_eq!(decls.get("web").map(|p| p.line), Some(5));
        assert_eq!(decls.get("api").map(|p| p.line), Some(6));
    }

    #[test]
    fn ignores_non_declaration_equals() {
        let tokens = tokenize(r#"properties { "key" "value" }"#);
        let decls = build_declarations(&tokens);
        assert!(decls.is_empty());
    }
}
