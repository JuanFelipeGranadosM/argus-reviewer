//! Literales de cadena: prefijos `r`/`b`/`u`/`f`/`t` y combinaciones, comillas simples y triples,
//! y f-strings con anidamiento PEP 701 (`f"{x["clave"]}"`). La f-string completa es un único token;
//! su estructura interna la descompone el parser.

use crate::lexer::cursor::Cursor;
use crate::lexer::{LexError, LexErrorKind};
use crate::syntax::{SyntaxKind, TextRange};

fn is_prefix(prefix: &[u8]) -> bool {
    match prefix {
        [] => true,
        [a] => matches!(a.to_ascii_lowercase(), b'r' | b'u' | b'b' | b'f' | b't'),
        [a, b] => matches!(
            (a.to_ascii_lowercase(), b.to_ascii_lowercase()),
            (b'r', b'b') | (b'b', b'r') | (b'r', b'f') | (b'f', b'r') | (b'r', b't') | (b't', b'r')
        ),
        _ => false,
    }
}

/// Longitud del prefijo si `rest` empieza un literal de cadena.
pub(super) fn prefix_len(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    (0..=2).find(|&len| {
        bytes.get(len).is_some_and(|&q| q == b'\'' || q == b'"')
            && bytes.get(..len).is_some_and(is_prefix)
    })
}

#[derive(Copy, Clone)]
struct Flavor {
    quote: char,
    triple: bool,
    raw: bool,
    formatted: bool,
}

/// Precondición: `prefix_len(cur.rest())` es `Some`.
pub(super) fn lex_string(cur: &mut Cursor, errors: &mut Vec<LexError>) -> SyntaxKind {
    let start = cur.offset();
    let plen = prefix_len(cur.rest()).unwrap_or(0);
    let prefix = cur.rest().get(..plen).unwrap_or_default().to_ascii_lowercase();
    (0..plen).for_each(|_| {
        cur.bump();
    });
    let Some(quote) = cur.bump() else {
        return SyntaxKind::Error;
    };
    let triple = cur.first() == Some(quote) && cur.second() == Some(quote);
    if triple {
        cur.bump();
        cur.bump();
    }
    let flavor = Flavor {
        quote,
        triple,
        raw: prefix.contains('r'),
        formatted: prefix.contains('f') || prefix.contains('t'),
    };
    if !scan_body(cur, flavor, errors) {
        errors.push(LexError {
            kind: LexErrorKind::UnterminatedString,
            range: TextRange::new(start, cur.offset()),
        });
    }
    if flavor.formatted {
        SyntaxKind::FString
    } else {
        SyntaxKind::String
    }
}

fn closes(cur: &Cursor, flavor: Flavor) -> bool {
    let mut chars = cur.rest().chars();
    let q = Some(flavor.quote);
    if flavor.triple {
        chars.next() == q && chars.next() == q && chars.next() == q
    } else {
        chars.next() == q
    }
}

/// Devuelve `false` si la cadena no se cierra antes del fin de línea (comillas simples) o de archivo.
fn scan_body(cur: &mut Cursor, flavor: Flavor, errors: &mut Vec<LexError>) -> bool {
    loop {
        if closes(cur, flavor) {
            let quotes = if flavor.triple { 3 } else { 1 };
            (0..quotes).for_each(|_| {
                cur.bump();
            });
            return true;
        }
        match cur.first() {
            None => return false,
            Some('\n' | '\r') if !flavor.triple => return false,
            Some('\\') => escape(cur, flavor),
            Some('{') if flavor.formatted => {
                cur.bump();
                if !cur.eat('{') {
                    replacement_field(cur, errors);
                }
            }
            Some(_) => {
                cur.bump();
            }
        }
    }
}

/// Una barra invertida protege el carácter siguiente de cerrar la cadena, también en cadenas crudas.
fn escape(cur: &mut Cursor, flavor: Flavor) {
    cur.bump();
    if !flavor.raw && cur.first() == Some('N') && cur.second() == Some('{') {
        cur.eat_while(|c| c != '}' && c != flavor.quote && c != '\n');
        cur.eat('}');
        return;
    }
    if flavor.formatted && cur.first() == Some('{') {
        return;
    }
    if !cur.eat_newline() {
        cur.bump();
    }
}

/// Consume el contenido de `{...}` tras la llave de apertura, incluidas cadenas anidadas.
fn replacement_field(cur: &mut Cursor, errors: &mut Vec<LexError>) {
    let mut depth = 0u32;
    while let Some(c) = cur.first() {
        match c {
            '}' if depth == 0 => {
                cur.bump();
                return;
            }
            '{' | '(' | '[' => {
                depth = depth.saturating_add(1);
                cur.bump();
            }
            '}' | ')' | ']' => {
                depth = depth.saturating_sub(1);
                cur.bump();
            }
            _ if prefix_len(cur.rest()).is_some() => {
                lex_string(cur, errors);
            }
            _ => {
                cur.bump();
            }
        }
    }
}
