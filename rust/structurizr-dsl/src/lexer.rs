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

    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace
        if c == '\n' {
            line += 1;
            col = 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            col += 1;
            i += 1;
            continue;
        }

        let pos = Pos { line, col };

        // Single-line comment: // or #
        if (c == '/' && i + 1 < chars.len() && chars[i + 1] == '/')
            || (c == '#' && !(i + 1 < chars.len() && chars[i + 1] == '{'))
        {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            continue;
        }

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
                && !(chars[i] == '/' && i + 1 < chars.len() && (chars[i + 1] == '/' || chars[i + 1] == '*'))
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
    fn tokenizes_arrow() {
        let tokens = tokenize("a -> b");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[1].token, Token::Arrow));
    }
}
