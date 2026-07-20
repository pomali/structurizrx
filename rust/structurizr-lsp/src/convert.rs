//! Conversions between the DSL's 1-based `Pos` and the LSP's 0-based `Position`.
//!
//! Getting this off-by-one wrong is the easiest way to make diagnostics/hover
//! land one line or column away from where the user expects, so it's isolated
//! here and unit-tested rather than inlined at every call site.

use structurizr_dsl::lexer::Pos;
use tower_lsp_server::ls_types::{Position, Range};

pub fn pos_to_position(pos: Pos) -> Position {
    Position {
        line: pos.line.saturating_sub(1) as u32,
        character: pos.col.saturating_sub(1) as u32,
    }
}

pub fn position_to_pos(position: Position) -> Pos {
    Pos {
        line: position.line as usize + 1,
        col: position.character as usize + 1,
    }
}

/// A range spanning from `pos` to the end of its source line. Diagnostics only
/// carry a start position (no token length), so this gives a squiggle that
/// covers "the rest of the offending line" rather than a single character.
pub fn line_range(text: &str, pos: Pos) -> Range {
    let line_len = text
        .lines()
        .nth(pos.line.saturating_sub(1))
        .map(|l| l.chars().count())
        .unwrap_or(pos.col);
    let start = pos_to_position(pos);
    let end = Position {
        line: start.line,
        character: (line_len as u32).max(start.character),
    };
    Range { start, end }
}

/// A single-character range at `pos`, used where we resolved an exact token
/// (hover, go-to-definition) rather than just a diagnostic start point.
pub fn point_range(pos: Pos, len: usize) -> Range {
    let start = pos_to_position(pos);
    let end = Position {
        line: start.line,
        character: start.character + len as u32,
    };
    Range { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pos_position_roundtrip() {
        let pos = Pos { line: 3, col: 5 };
        let position = pos_to_position(pos);
        assert_eq!(
            position,
            Position {
                line: 2,
                character: 4
            }
        );
        assert_eq!(position_to_pos(position).line, pos.line);
        assert_eq!(position_to_pos(position).col, pos.col);
    }

    #[test]
    fn first_line_first_col_is_origin() {
        let pos = Pos { line: 1, col: 1 };
        assert_eq!(
            pos_to_position(pos),
            Position {
                line: 0,
                character: 0
            }
        );
    }

    #[test]
    fn line_range_covers_rest_of_line() {
        let text = "workspace {\n  model {\n";
        let range = line_range(text, Pos { line: 2, col: 3 });
        assert_eq!(
            range.start,
            Position {
                line: 1,
                character: 2
            }
        );
        assert_eq!(range.end.line, 1);
        assert!(range.end.character >= range.start.character);
    }
}
