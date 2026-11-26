//! Vocabulario sintáctico común: rangos de texto, tipos de token y tokens.

mod kind;
mod line_index;
mod text_range;

pub use kind::SyntaxKind;
pub use line_index::LineIndex;
pub use text_range::TextRange;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub range: TextRange,
}

impl Token {
    pub const fn new(kind: SyntaxKind, range: TextRange) -> Self {
        Self { kind, range }
    }

    pub fn text(self, source: &str) -> &str {
        self.range.slice(source)
    }
}
