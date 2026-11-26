//! Cadenas, plantillas, expresiones regulares, números y escapes `\u` de identificadores.

use super::punct::{is_ident_continue, is_line_terminator};
use crate::lexer::cursor::Cursor;
use crate::lexer::numbers::scan_number;
use crate::lexer::{LexError, LexErrorKind};
use crate::syntax::{SyntaxKind, TextRange};

fn unterminated(errors: &mut Vec<LexError>, kind: LexErrorKind, start: u32, cur: &Cursor) {
    errors.push(LexError {
        kind,
        range: TextRange::new(start, cur.offset()),
    });
}

/// Una barra invertida protege el carácter siguiente; con un salto de línea es una continuación.
fn escape(cur: &mut Cursor) {
    cur.bump();
    if !cur.eat_newline() {
        cur.bump();
    }
}

/// Precondición: el cursor está en `'` o `"`. Termina en la comilla o antes de `\n`/`\r`.
pub(super) fn string(cur: &mut Cursor, errors: &mut Vec<LexError>) -> SyntaxKind {
    let start = cur.offset();
    let quote = cur.bump();
    loop {
        match cur.first() {
            Some(c) if Some(c) == quote => {
                cur.bump();
                return SyntaxKind::String;
            }
            Some('\\') => escape(cur),
            Some('\n' | '\r') | None => {
                unterminated(errors, LexErrorKind::UnterminatedString, start, cur);
                return SyntaxKind::String;
            }
            Some(_) => {
                cur.bump();
            }
        }
    }
}

/// Un tramo de plantilla. Si empieza en `` ` `` es `NoSub` o `Head`; si empieza en el `}` que
/// cierra un `${`, es `Middle` o `Tail`. Los tramos `Head`/`Middle` terminan en `${`.
pub(super) fn template_chunk(cur: &mut Cursor, errors: &mut Vec<LexError>) -> SyntaxKind {
    let start = cur.offset();
    let opened = cur.bump() == Some('`');
    let (closed, interpolated) = if opened {
        (SyntaxKind::TemplateNoSub, SyntaxKind::TemplateHead)
    } else {
        (SyntaxKind::TemplateTail, SyntaxKind::TemplateMiddle)
    };
    loop {
        match cur.first() {
            Some('`') => {
                cur.bump();
                return closed;
            }
            Some('$') if cur.second() == Some('{') => {
                cur.bump();
                cur.bump();
                return interpolated;
            }
            Some('\\') => escape(cur),
            Some(_) => {
                cur.bump();
            }
            None => {
                unterminated(errors, LexErrorKind::UnterminatedTemplate, start, cur);
                return closed;
            }
        }
    }
}

/// Precondición: el cursor está en el `/` inicial. Dentro de `[...]` una `/` no cierra.
pub(super) fn regex(cur: &mut Cursor, errors: &mut Vec<LexError>) -> SyntaxKind {
    let start = cur.offset();
    cur.bump();
    let mut in_class = false;
    loop {
        let Some(c) = cur.first().filter(|&c| !is_line_terminator(c)) else {
            unterminated(errors, LexErrorKind::UnterminatedRegex, start, cur);
            break;
        };
        cur.bump();
        match c {
            '\\' if cur.first().is_some_and(|c| !is_line_terminator(c)) => {
                cur.bump();
            }
            '/' if !in_class => {
                cur.eat_while(is_ident_continue);
                break;
            }
            '[' => in_class = true,
            ']' => in_class = false,
            _ => {}
        }
    }
    SyntaxKind::Regex
}

/// Números con sufijo BigInt opcional (`10n`, `0xFFn`).
pub(super) fn number(cur: &mut Cursor) -> SyntaxKind {
    let kind = scan_number(cur);
    if cur.eat('n') {
        return SyntaxKind::Int;
    }
    kind
}

/// `\uXXXX` o `\u{X...}` dentro de un identificador. Siempre avanza al menos `\u`.
pub(super) fn unicode_escape(cur: &mut Cursor) {
    cur.bump();
    cur.bump();
    if cur.eat('{') {
        cur.eat_while(|c| c.is_ascii_hexdigit());
        cur.eat('}');
        return;
    }
    for _ in 0..4 {
        if !cur.first().is_some_and(|c| c.is_ascii_hexdigit()) {
            break;
        }
        cur.bump();
    }
}
