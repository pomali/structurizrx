/// A lexer token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A plain word (identifier or keyword).
    Word(String),
    /// A quoted string (contents without quotes).
    Quoted(String),
    /// A text block `"""..."""`.
    TextBlock(String),
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `=`
    Equals,
    /// `->`
    Arrow,
    /// A `!directive` token.
    Directive(String),
}

/// Position in source.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// A token with position.
#[derive(Debug, Clone)]
pub struct Spanned {
    pub token: Token,
    pub pos: Pos,
}

/// Tokenize DSL source text into a flat list of spanned tokens.
pub fn tokenize(source: &str) -> Vec<Spanned> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    let mut line = 1usize;
    let mut col = 1usize;
    // Whether anything other than whitespace has been seen on the current line.
    // `//` and `#` only start a comment at the beginning of a line, so that
    // unquoted urls (`themes https://example.com/theme.json`) survive intact.
    let mut line_has_content = false;

    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace
        if c == '\n' {
            line += 1;
            col = 1;
            i += 1;
            line_has_content = false;
            continue;
        }
        if c.is_whitespace() {
            col += 1;
            i += 1;
            continue;
        }

        // Line continuation: backslash at end of line joins it with the next.
        if c == '\\' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '\n' && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == '\n' {
                i = j + 1;
                line += 1;
                col = 1;
                continue;
            }
        }

        let pos = Pos { line, col };

        // Single-line comment. Two forms both run to end of line:
        //
        //   * At line start (`!line_has_content`): `//` or `#` as the first
        //     non-whitespace, matching upstream Structurizr's comment rule.
        //     `#{` is variable interpolation, never a comment.
        //   * Inline (after content, at a token boundary — a `#` or `//` only
        //     reaches the top of this loop preceded by whitespace, so we don't
        //     need to test the preceding char):
        //       - `#` starts a comment only when the next char is whitespace or
        //         end-of-line, so `color #ffffff` and `#{var}` stay tokens
        //         while `auto # layout note` is a comment.
        //       - `//` always starts a comment here; unquoted urls
        //         (`https://…`) keep their `//` mid-word, so it never surfaces
        //         at a token boundary.
        let starts_line_comment = if line_has_content {
            (c == '/' && i + 1 < chars.len() && chars[i + 1] == '/')
                || (c == '#' && (i + 1 >= chars.len() || chars[i + 1].is_whitespace()))
        } else {
            (c == '/' && i + 1 < chars.len() && chars[i + 1] == '/')
                || (c == '#' && !(i + 1 < chars.len() && chars[i + 1] == '{'))
        };
        if starts_line_comment {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            continue;
        }

        line_has_content = true;

        // Multi-line comment: /* ... */
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            col += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                i += 1;
            }
            i += 2; // skip */
            col += 2;
            continue;
        }

        // Text block: """..."""
        if c == '"' && i + 2 < chars.len() && chars[i + 1] == '"' && chars[i + 2] == '"' {
            i += 3;
            col += 3;
            let mut text = String::new();
            while i + 2 < chars.len()
                && !(chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"')
            {
                if chars[i] == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                text.push(chars[i]);
                i += 1;
            }
            i += 3; // skip closing """
            col += 3;
            tokens.push(Spanned {
                token: Token::TextBlock(text.trim().to_string()),
                pos,
            });
            continue;
        }

        // Quoted string: "..."
        if c == '"' {
            i += 1;
            col += 1;
            let mut text = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    col += 1;
                    text.push(chars[i]);
                } else if chars[i] == '\n' {
                    // unterminated string — just break
                    break;
                } else {
                    text.push(chars[i]);
                }
                i += 1;
                col += 1;
            }
            if i < chars.len() {
                i += 1; // skip closing "
                col += 1;
            }
            tokens.push(Spanned {
                token: Token::Quoted(text),
                pos,
            });
            continue;
        }

        // Open/close brace
        if c == '{' {
            tokens.push(Spanned { token: Token::OpenBrace, pos });
            i += 1;
            col += 1;
            continue;
        }
        if c == '}' {
            tokens.push(Spanned { token: Token::CloseBrace, pos });
            i += 1;
            col += 1;
            continue;
        }

        // Equals
        if c == '=' && !(i + 1 < chars.len() && chars[i + 1] == '>') {
            tokens.push(Spanned { token: Token::Equals, pos });
            i += 1;
            col += 1;
            continue;
        }

        // Arrow ->
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '>' {
            tokens.push(Spanned { token: Token::Arrow, pos });
            i += 2;
            col += 2;
            continue;
        }

        // Directive: !keyword
        if c == '!' {
            i += 1;
            col += 1;
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '{' && chars[i] != '}' {
                i += 1;
                col += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(Spanned {
                token: Token::Directive(word),
                pos,
            });
            continue;
        }

        // Word/identifier
        if !c.is_whitespace() {
            let start = i;
            while i < chars.len()
                && !chars[i].is_whitespace()
                && chars[i] != '{'
                && chars[i] != '}'
                && chars[i] != '"'
                && chars[i] != '='
                && !(chars[i] == '-' && i + 1 < chars.len() && chars[i + 1] == '>')
                // `/*` still ends a word, but `//` does not — it is part of urls.
                && !(chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*')
            {
                i += 1;
                col += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if !word.is_empty() {
                tokens.push(Spanned {
                    token: Token::Word(word),
                    pos,
                });
            }
            continue;
        }

        i += 1;
        col += 1;
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_simple() {
        let tokens = tokenize(r#"workspace "Hello" { }"#);
        assert_eq!(tokens.len(), 4);
    }

    #[test]
    fn skips_line_comments() {
        let tokens = tokenize("// comment\nworkspace");
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn skips_block_comments() {
        let tokens = tokenize("/* hello */ workspace");
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn indented_line_comments_are_skipped() {
        let tokens = tokenize("workspace\n    // comment\n    # another\n    model");
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn double_slash_inside_a_line_is_not_a_comment() {
        // Theme and icon urls are written unquoted, so `//` mid-line has to
        // stay part of the word.
        let tokens = tokenize("themes https://example.com/theme.json");
        assert_eq!(tokens.len(), 2);
        assert!(
            matches!(&tokens[1].token, Token::Word(w) if w == "https://example.com/theme.json"),
            "got {:?}",
            tokens[1].token
        );
    }

    #[test]
    fn hash_inside_a_line_is_not_a_comment() {
        let tokens = tokenize("background #1168bd");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[1].token, Token::Word(w) if w == "#1168bd"));
    }

    #[test]
    fn inline_hash_followed_by_space_is_a_comment() {
        // `#` at a token boundary, followed by whitespace, comments the rest of
        // the line — the keyword before it survives.
        let tokens = tokenize("auto # this is a layout note");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].token, Token::Word(w) if w == "auto"));
    }

    #[test]
    fn inline_hash_color_survives_alongside_inline_comment() {
        let tokens = tokenize("background #1168bd # brand blue");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0].token, Token::Word(w) if w == "background"));
        assert!(matches!(&tokens[1].token, Token::Word(w) if w == "#1168bd"));
    }

    #[test]
    fn inline_hash_interpolation_is_not_a_comment() {
        // `#{` at a token boundary is variable interpolation, not a comment:
        // the next char is `{`, not whitespace, so the rest of the line is
        // still tokenized (the word reader splits on `{` as it always has).
        let tokens = tokenize("name #{env}");
        assert!(tokens.len() > 1, "got {tokens:?}");
        assert!(matches!(&tokens[1].token, Token::Word(w) if w == "#"));
    }

    #[test]
    fn trailing_bare_hash_is_a_comment() {
        // A `#` as the last char on a line (nothing after it) is a comment.
        let tokens = tokenize("auto #");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].token, Token::Word(w) if w == "auto"));
    }

    #[test]
    fn inline_double_slash_is_a_comment_but_urls_survive() {
        let tokens = tokenize("auto // layout note\nthemes https://example.com/t.json");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[0].token, Token::Word(w) if w == "auto"));
        assert!(matches!(&tokens[1].token, Token::Word(w) if w == "themes"));
        assert!(
            matches!(&tokens[2].token, Token::Word(w) if w == "https://example.com/t.json"),
            "got {:?}",
            tokens[2].token
        );
    }

    #[test]
    fn tokenizes_arrow() {
        let tokens = tokenize("a -> b");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[1].token, Token::Arrow));
    }
}
