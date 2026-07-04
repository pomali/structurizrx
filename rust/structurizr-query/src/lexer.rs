/// Tokens produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `*`
    Star,
    /// `->`
    Arrow,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `&&`
    And,
    /// `||`
    Or,
    /// `!`
    Not,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `.`
    Dot,
    /// `^`
    Caret,
    /// An integer literal (used for neighborhood depth).
    Int(u32),
    /// An unquoted word: identifiers and keywords.
    Word(String),
    /// A double-quoted string (content without surrounding quotes).
    Quoted(String),
    /// End of input.
    Eof,
}

/// A token together with its byte offset in the source string.
#[derive(Debug, Clone)]
pub struct Spanned {
    pub token: Token,
    pub offset: usize,
}

/// Tokenize `src` into a flat `Vec<Spanned>`.
///
/// The final entry is always `Token::Eof`.
pub fn tokenize(src: &str) -> Result<Vec<Spanned>, String> {
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();

    while i < bytes.len() {
        // Skip whitespace.
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        let start = i;

        // Two-character tokens: ->, &&, ||, ==, !=
        if i + 1 < bytes.len() {
            let two = &bytes[i..i + 2];
            match two {
                b"->" => { out.push(Spanned { token: Token::Arrow, offset: start }); i += 2; continue; }
                b"&&" => { out.push(Spanned { token: Token::And,   offset: start }); i += 2; continue; }
                b"||" => { out.push(Spanned { token: Token::Or,    offset: start }); i += 2; continue; }
                b"==" => { out.push(Spanned { token: Token::Eq,    offset: start }); i += 2; continue; }
                b"!=" => { out.push(Spanned { token: Token::Ne,    offset: start }); i += 2; continue; }
                _ => {}
            }
        }

        // Single-character tokens.
        match bytes[i] {
            b'*' => { out.push(Spanned { token: Token::Star,   offset: start }); i += 1; continue; }
            b'(' => { out.push(Spanned { token: Token::LParen, offset: start }); i += 1; continue; }
            b')' => { out.push(Spanned { token: Token::RParen, offset: start }); i += 1; continue; }
            b'!' => { out.push(Spanned { token: Token::Not,    offset: start }); i += 1; continue; }
            b'.' => { out.push(Spanned { token: Token::Dot,    offset: start }); i += 1; continue; }
            b'^' => { out.push(Spanned { token: Token::Caret,  offset: start }); i += 1; continue; }
            _ => {}
        }

        // Double-quoted string.
        if bytes[i] == b'"' {
            i += 1; // consume opening quote
            let mut s = String::new();
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 1;
                    match bytes[i] {
                        b'"'  => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'n'  => s.push('\n'),
                        b't'  => s.push('\t'),
                        other => { s.push('\\'); s.push(other as char); }
                    }
                } else {
                    s.push(bytes[i] as char);
                }
                i += 1;
            }
            if i >= bytes.len() {
                return Err(format!("unterminated string starting at offset {start}"));
            }
            i += 1; // consume closing quote
            out.push(Spanned { token: Token::Quoted(s), offset: start });
            continue;
        }

        // Integer or word.
        if bytes[i].is_ascii_digit() {
            let mut n: u32 = 0;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                n = n * 10 + (bytes[i] - b'0') as u32;
                i += 1;
            }
            out.push(Spanned { token: Token::Int(n), offset: start });
            continue;
        }

        // Word: letters, digits, underscores, hyphens (e.g. "checkout-team").
        // A '-' is only consumed as part of a word when the *next* byte is not '>';
        // otherwise it would swallow the leading '-' of a '->' arrow token.
        let starts_word = bytes[i].is_ascii_alphanumeric()
            || bytes[i] == b'_'
            || (bytes[i] == b'-' && !(i + 1 < bytes.len() && bytes[i + 1] == b'>'));
        if starts_word {
            let mut s = String::new();
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || (bytes[i] == b'-'
                        && !(i + 1 < bytes.len() && bytes[i + 1] == b'>')))
            {
                s.push(bytes[i] as char);
                i += 1;
            }
            out.push(Spanned { token: Token::Word(s), offset: start });
            continue;
        }

        return Err(format!("unexpected character {:?} at offset {start}", bytes[i] as char));
    }

    out.push(Spanned { token: Token::Eof, offset: src.len() });
    Ok(out)
}
