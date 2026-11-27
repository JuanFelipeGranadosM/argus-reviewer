//! Lexers sin pérdida.
//!
//! Invariante: los tokens producidos cubren todos los bytes de la fuente, en orden, de forma contigua
//! y sin solapes. Los tokens de ancho cero (`Indent`, `Dedent`, `LogicalNewline` final, `EndOfFile`)
//! no cubren bytes. Ante entrada inválida se emite un token `Error` y un [`LexError`], y se sigue.

pub mod comments;
mod cursor;
pub mod javascript;
mod numbers;
pub mod python;

use std::path::Path;

use crate::arena::Language;
use crate::syntax::{TextRange, Token};

#[derive(Copy, Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LexErrorKind {
    #[error("cadena sin cerrar")]
    UnterminatedString,
    #[error("comentario de bloque sin cerrar")]
    UnterminatedComment,
    #[error("expresión regular sin cerrar")]
    UnterminatedRegex,
    #[error("plantilla (template literal) sin cerrar")]
    UnterminatedTemplate,
    #[error("elemento JSX sin cerrar")]
    UnterminatedJsx,
    #[error("carácter inesperado")]
    UnexpectedChar,
    #[error("la desindentación no coincide con ningún nivel de indentación anterior")]
    InconsistentDedent,
    #[error("barra invertida fuera de una continuación de línea")]
    StrayBackslash,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub range: TextRange,
}

#[derive(Debug, Default)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum SourceError {
    #[error("el archivo ocupa {0} bytes; el máximo admitido es 4 GiB")]
    TooLarge(usize),
}

/// Lexea `source` con el lexer de `language`; `path` decide el dialecto (JSX o no) en JS/TS.
pub fn lex_file(source: &str, language: Language, path: &Path) -> Result<Lexed, SourceError> {
    match language {
        Language::Python => python::lex(source),
        Language::JavaScript | Language::TypeScript => {
            javascript::lex(source, javascript::Dialect::from_path(path))
        }
    }
}

fn check_size(source: &str) -> Result<(), SourceError> {
    u32::try_from(source.len())
        .map(|_| ())
        .map_err(|_| SourceError::TooLarge(source.len()))
}
