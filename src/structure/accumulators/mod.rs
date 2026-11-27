//! Bucles que solo llenan una colección recién creada (`r = []` + `for` + `r.append(...)`), con la
//! expresión funcional equivalente ya escrita: una comprensión en Python, `map`/`filter` en JS/TS.
//!
//! Solo se reconoce la forma segura: la colección se crea vacía justo antes del bucle, el cuerpo es
//! una única inserción (opcionalmente bajo un `if` sin `else`), nada de lo que se evalúa menciona la
//! propia colección ni usa `await`/`yield`, y el resultado cabe en una línea.

pub(super) mod javascript;
pub(super) mod python;

use super::sig::Sig;
use crate::syntax::SyntaxKind::*;

/// Rango inclusivo de tokens.
type Range = (usize, usize);

/// Texto de una pieza que irá dentro de una sola línea: `None` si está vacía o tiene saltos de línea.
fn piece<'a>(sig: &Sig<'a>, (first, last): Range) -> Option<&'a str> {
    if first > last {
        return None;
    }
    let text = sig.text_of(first, last).trim();
    (!text.is_empty() && !text.contains(['\n', '\r'])).then_some(text)
}

/// La pieza no nombra la colección (una comprensión no puede verse a sí misma a medio construir) ni
/// suspende la ejecución.
fn independent(sig: &Sig, (first, last): Range, name: &str) -> bool {
    (first..=last).all(|j| {
        !sig.is_word(j, name) && !matches!(sig.kind(j), Some(KwAwait | KwYield))
    })
}

/// Aviso para el agente si hay comentarios entre los tokens `first..=last`: la sustitución los
/// perdería. En los huecos entre tokens solo hay espacios y comentarios, así que basta el marcador.
fn comments_note(sig: &Sig, (first, last): Range, markers: &[&str]) -> Option<&'static str> {
    (first..last)
        .any(|j| {
            let gap = sig.between(j, j + 1);
            markers.iter().any(|m| gap.contains(m))
        })
        .then_some("El bucle tiene comentarios: conservarlos encima de la nueva línea.")
}

/// Primer `word` de primer nivel en `first..=last`, saltando los grupos anidados.
fn find_word(sig: &Sig, first: usize, last: usize, word: &str) -> Option<usize> {
    let mut j = first;
    while j <= last {
        if sig.is_word(j, word) {
            return Some(j);
        }
        j = sig.close_of(j).unwrap_or(j) + 1;
    }
    None
}

/// `name . method ( valor )` con un solo argumento: el rango del valor y el índice del `)`.
fn insertion(sig: &Sig, at: usize, name: &str, method: &str) -> Option<(Range, usize)> {
    let open = at + 3;
    let ok = sig.is_word(at, name)
        && sig.is(at + 1, Dot)
        && sig.is_word(at + 2, method)
        && sig.is(open, LParen);
    let close = sig.close_of(open).filter(|_| ok)?;
    let single = sig.segments(open, close, false).len() == 1;
    (single && close > open + 1).then_some(((open + 1, close - 1), close))
}
