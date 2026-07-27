//! Semantic tokens, driven by the DSL lexer.
//!
//! The TextMate grammar has to guess whether a bare word is a keyword or an
//! identifier, because it matches line by line with no notion of scope. The
//! lexer knows, so this narrows the highlighting to the three cases where that
//! guess goes wrong: keywords, identifiers, and `!directive`s.
//!
//! Everything else (strings, comments, punctuation) is deliberately *not*
//! emitted, so the TextMate grammar keeps colouring it. Strings in particular
//! would need source spans the lexer doesn't keep — `Token::Quoted` holds the
//! unescaped contents, whose length isn't the length of the source text.

use std::collections::HashSet;
use std::sync::OnceLock;

use ls_types::{SemanticToken, SemanticTokenType, SemanticTokensLegend};
use structurizr_dsl::lexer::{Spanned, Token};

use crate::index::Declarations;

/// Index into [`legend`]'s token-type list.
const KEYWORD: u32 = 0;
const VARIABLE: u32 = 1;
const MACRO: u32 = 2;

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::MACRO,
        ],
        token_modifiers: vec![],
    }
}

/// Every keyword the parser accepts anywhere, lowercased.
fn vocabulary() -> &'static HashSet<String> {
    static VOCABULARY: OnceLock<HashSet<String>> = OnceLock::new();
    VOCABULARY.get_or_init(|| {
        let mut set: HashSet<String> = structurizr_dsl::keyword_sets()
            .iter()
            .flat_map(|(_, keywords)| keywords.iter())
            .map(|kw| kw.to_lowercase())
            .collect();
        // `workspace` opens the file, so it heads no keyword set of its own.
        set.insert("workspace".to_string());
        set
    })
}

/// Encodes `tokens` as LSP semantic tokens (5 ints each, deltas from the
/// previous emitted token).
pub fn encode(tokens: &[Spanned], declarations: &Declarations) -> Vec<SemanticToken> {
    let mut out = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;

    for t in tokens {
        let (token_type, len) = match &t.token {
            Token::Directive(name) => (MACRO, name.chars().count() + 1), // include the `!`
            Token::Word(word) => {
                let len = word.chars().count();
                if declarations.contains_key(&word.to_lowercase()) {
                    (VARIABLE, len)
                } else if vocabulary().contains(&word.to_lowercase()) {
                    (KEYWORD, len)
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        let line = t.pos.line.saturating_sub(1) as u32;
        let col = t.pos.col.saturating_sub(1) as u32;
        out.push(SemanticToken {
            delta_line: line - prev_line,
            delta_start: if line == prev_line { col - prev_col } else { col },
            length: len as u32,
            token_type,
            token_modifiers_bitset: 0,
        });
        prev_line = line;
        prev_col = col;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::build_declarations;
    use structurizr_dsl::lexer::tokenize;

    fn encode_str(source: &str) -> Vec<SemanticToken> {
        let tokens = tokenize(source);
        let declarations = build_declarations(&tokens);
        encode(&tokens, &declarations)
    }

    #[test]
    fn classifies_keywords_and_identifiers() {
        let out = encode_str("model {\n    user = person \"User\"\n}\n");
        let types: Vec<u32> = out.iter().map(|t| t.token_type).collect();
        assert_eq!(types, vec![KEYWORD, VARIABLE, KEYWORD], "model, user, person");
    }

    #[test]
    fn directives_include_their_bang() {
        let out = encode_str("!include foo.dsl\n");
        assert_eq!(out[0].token_type, MACRO);
        assert_eq!(out[0].length, 8, "`!include`");
    }

    #[test]
    fn deltas_are_relative_to_the_previous_token() {
        let out = encode_str("model {\n    user = person \"User\"\n}\n");
        assert_eq!((out[0].delta_line, out[0].delta_start), (0, 0), "model");
        assert_eq!((out[1].delta_line, out[1].delta_start), (1, 4), "user");
        assert_eq!((out[2].delta_line, out[2].delta_start), (0, 7), "person");
    }

    #[test]
    fn quoted_text_is_left_to_the_textmate_grammar() {
        // "person" inside a string must not be emitted as a keyword.
        let out = encode_str("model {\n    x = person \"a person\"\n}\n");
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn an_identifier_shadowing_a_keyword_reads_as_an_identifier() {
        let out = encode_str("container = softwareSystem \"S\"\n");
        assert_eq!(out[0].token_type, VARIABLE, "the declared `container`");
    }
}
