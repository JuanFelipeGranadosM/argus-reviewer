//! Funciones y `except` de Python, delimitados por los `INDENT`/`DEDENT` del lexer.

use super::sig::Sig;
use super::{accumulators, idioms};
use super::{Catch, Function, Outline, Param};
use crate::syntax::SyntaxKind::*;

/// Tope de tokens al buscar el `:` de una cabecera (`def`, `except`).
const MAX_HEADER: usize = 512;

pub(super) fn outline(sig: &Sig) -> Outline {
    let mut outline = Outline::default();
    for i in 0..sig.len() {
        match sig.kind(i) {
            Some(KwDef) => outline.functions.extend(def(sig, i)),
            Some(KwExcept) => outline.catches.extend(except(sig, i)),
            Some(Ident) if is_dynamic_eval(sig, i) => outline.dynamic_evals.push(sig.span(i, i)),
            _ => {}
        }
        outline.idioms.extend(idioms::python(sig, i));
        outline
            .accumulators
            .extend(accumulators::python::find(sig, i));
    }
    outline
}

/// `if`, `elif`, bucles, `except`, `and` y `or` (incluidos los de comprensiones y ternarios).
pub(super) fn is_decision(sig: &Sig, i: usize) -> bool {
    matches!(
        sig.kind(i),
        Some(KwIf | KwElif | KwFor | KwWhile | KwExcept | KwAnd | KwOr)
    )
}

fn def(sig: &Sig, i: usize) -> Option<Function> {
    let name_at = i + 1;
    if !sig.is(name_at, Ident) {
        return None;
    }
    let mut open = i + 2;
    if sig.is(open, LBracket) {
        open = sig.close_of(open)? + 1;
    }
    if !sig.is(open, LParen) {
        return None;
    }
    let close = sig.close_of(open)?;
    let colon = header_colon(sig, close + 1)?;
    let last = suite_end(sig, colon);
    let first = if sig.prev(i) == Some(KwAsync) {
        i - 1
    } else {
        i
    };
    let name = Some(sig.text(name_at).to_owned());
    let has_return = sig.is(close + 1, Arrow);
    Some(
        Function::new(sig, name, (first, last))
            .with_signature(params(sig, open, close), has_return),
    )
}

/// Parámetros sin el `self`/`cls` inicial ni los separadores `*` y `/`.
fn params(sig: &Sig, open: usize, close: usize) -> Vec<Param> {
    let mut out = Vec::new();
    for (index, (first, last)) in sig.segments(open, close, false).into_iter().enumerate() {
        let separator = first == last && matches!(sig.kind(first), Some(Star | Slash));
        let mut at = first;
        while at < last && matches!(sig.kind(at), Some(Star | DoubleStar)) {
            at += 1;
        }
        let name = sig.text(at);
        if separator || (index == 0 && matches!(name, "self" | "cls")) {
            continue;
        }
        let colon = sig.find_top_level(at, last, |k| matches!(k, Colon | KwLambda));
        out.push(Param {
            name: name.to_owned(),
            annotated: colon.is_some_and(|c| sig.is(c, Colon)),
        });
    }
    out
}

/// `except:` o `except Exception:` cuyo cuerpo es solo `pass` o `...` y no explica por qué con un
/// comentario. Capturar una excepción concreta e ignorarla es una decisión; capturarlo todo, no.
fn except(sig: &Sig, i: usize) -> Option<Catch> {
    let colon = header_colon(sig, i + 1)?;
    let broad_name = matches!(sig.text(i + 1), "Exception" | "BaseException");
    let broad = colon == i + 1 || (broad_name && (colon == i + 2 || sig.is(i + 2, KwAs)));
    let last = suite_end(sig, colon);
    let only_pass = (colon + 1..=last).all(|k| {
        matches!(
            sig.kind(k),
            Some(KwPass | Ellipsis | LogicalNewline | Indent | Dedent)
        )
    });
    let documented = sig.between(colon, last).contains('#') || sig.rest_of_line(last).contains('#');
    Some(Catch {
        span: sig.span(i, last),
        silent: broad && only_pass && !documented,
    })
}

fn is_dynamic_eval(sig: &Sig, i: usize) -> bool {
    let call = sig.is(i + 1, LParen) && !matches!(sig.prev(i), Some(Dot | KwDef));
    call && matches!(sig.text(i), "eval" | "exec")
}

/// El `:` que cierra una cabecera, saltando paréntesis (anotaciones, valores por defecto).
fn header_colon(sig: &Sig, from: usize) -> Option<usize> {
    let mut j = from;
    for _ in 0..MAX_HEADER {
        match sig.kind(j)? {
            Colon => return Some(j),
            LogicalNewline => return None,
            LParen | LBracket | LBrace => j = sig.close_of(j)?,
            _ => {}
        }
        j += 1;
    }
    None
}

/// Último token del bloque que abre el `:` de `colon`: el resto de la línea o el bloque indentado.
fn suite_end(sig: &Sig, colon: usize) -> usize {
    let mut j = colon + 1;
    if sig.is(j, LogicalNewline) && sig.is(j + 1, Indent) {
        let mut depth = 0u32;
        j += 1;
        while let Some(kind) = sig.kind(j) {
            match kind {
                Indent => depth += 1,
                Dedent => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
    } else {
        while sig.kind(j).is_some_and(|k| k != LogicalNewline) {
            j += 1;
        }
    }
    (colon + 1..j)
        .rev()
        .find(|&k| !matches!(sig.kind(k), Some(LogicalNewline | Indent | Dedent)))
        .unwrap_or(colon)
}
