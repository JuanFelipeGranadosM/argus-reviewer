//! Vocabulario sintáctico común: rangos de texto, tipos de token y tokens.

mod kind;
mod text_range;

pub use kind::SyntaxKind;
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
