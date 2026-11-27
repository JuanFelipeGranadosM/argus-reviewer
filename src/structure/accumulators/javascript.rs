//! `const r = [];` seguido de `for (const x of xs)` cuyo cuerpo solo hace `r.push(...)`, opcionalmente
//! bajo un `if` sin `else`: equivale a `map`/`filter` sin mutación.

use super::{comments_note, find_word, independent, insertion, piece};
use crate::structure::sig::Sig;
use crate::structure::{Accumulator, Collection};
use crate::syntax::SyntaxKind::*;

/// Longitud de línea por encima de la cual la cadena de `filter`/`map` deja de ser más clara.
const MAX_LINE: usize = 100;
/// Tope de tokens de la anotación de tipo de la declaración (`const r: Fila[] = []`).
const MAX_HEAD: usize = 32;
/// Expresiones que seguro producen un array, sobre las que `.map`/`.filter` son válidos.
const ARRAY_SOURCES: &[&str] = &[
    "Object.entries(",
    "Object.keys(",
    "Object.values(",
    "Array.from(",
];

pub(in crate::structure) fn find(sig: &Sig, i: usize) -> Option<Accumulator> {
    let declares = sig.is(i, KwConst) || sig.is_word(i, "let");
    if !declares || !sig.is(i + 1, Ident) || matches!(sig.prev(i), Some(Dot | QuestionDot)) {
        return None;
    }
    let name = sig.text(i + 1);
    let eq = sig
        .find_top_level(i + 2, i + 2 + MAX_HEAD, |k| {
            matches!(k, Eq | Semicolon | Comma)
        })
        .filter(|&j| sig.is(j, Eq))?;
    if !sig.is(eq + 1, LBracket) || sig.close_of(eq + 1) != Some(eq + 2) {
        return None;
    }
    let for_at = if sig.is(eq + 3, Semicolon) {
        eq + 4
    } else {
        eq + 3
    };
    let open = for_at + 1;
    let close = sig.close_of(open).filter(|_| sig.is(for_at, KwFor))?;
    let binds = sig.is(open + 1, KwConst) || sig.is_word(open + 1, "let");
    let of_at = find_word(sig, open + 2, close.checked_sub(1)?, "of").filter(|_| binds)?;
    let (stmt, _, last) = body(sig, close + 1, name)?;
    let (cond, add) = if sig.is(stmt, KwIf) {
        let cond_close = sig.close_of(stmt + 1)?;
        let cond = (stmt + 2, cond_close.checked_sub(1)?);
        (Some(cond), body(sig, cond_close + 1, name)?.0)
    } else {
        (None, stmt)
    };
    let (value, _) = insertion(sig, add, name, "push")?;
    let target = (open + 2, of_at - 1);
    let iter = (of_at + 1, close - 1);
    let ranges = [Some(target), Some(iter), cond, Some(value)];
    if !ranges
        .into_iter()
        .flatten()
        .all(|r| independent(sig, r, name))
    {
        return None;
    }
    let cond = match cond {
        Some(c) => Some(piece(sig, c)?),
        None => None,
    };
    let head = piece(sig, (i, eq - 1))?;
    let iter = piece(sig, iter)?;
    let (target, value) = (piece(sig, target)?, piece(sig, value)?);
    let transforms = cond.is_some() || value != target;
    let replacement = chain(head, target, iter, cond, value);
    let known_array = ARRAY_SOURCES.iter().any(|s| iter.starts_with(s));
    let array_note = (transforms && !known_array).then(|| {
        format!(
            "Si `{iter}` no es un array (Set, Map, generador), envolverlo en `Array.from(...)`."
        )
    });
    let notes: Vec<std::string::String> = array_note
        .into_iter()
        .chain(comments_note(sig, (i, last), &["//", "/*"]).map(str::to_owned))
        .collect();
    let caveat = (!notes.is_empty()).then(|| notes.join(" "));
    (replacement.chars().count() <= MAX_LINE).then(|| Accumulator {
        name: name.to_owned(),
        span: sig.span(i, last),
        replacement,
        collection: Collection::List,
        caveat,
    })
}

/// Cuerpo de una sola sentencia en `at` (con llaves o sin ellas): su primer token, el índice que
/// sigue al cuerpo y su último token.
fn body(sig: &Sig, at: usize, name: &str) -> Option<(usize, usize, usize)> {
    if sig.is(at, LBrace) {
        let rbrace = sig.close_of(at)?;
        let end = statement_end(sig, at + 1, name)?;
        return (end == rbrace).then_some((at + 1, rbrace + 1, rbrace));
    }
    let end = statement_end(sig, at, name)?;
    Some((at, end, end.checked_sub(1)?))
}

/// Índice que sigue a `name.push(...)` o a un `if (...)` sin `else` que solo la contiene.
fn statement_end(sig: &Sig, first: usize, name: &str) -> Option<usize> {
    if sig.is(first, KwIf) {
        let cond_close = sig
            .close_of(first + 1)
            .filter(|_| sig.is(first + 1, LParen))?;
        let (_, end, _) = body(sig, cond_close + 1, name)?;
        return (!sig.is(end, KwElse)).then_some(end);
    }
    let (_, close) = insertion(sig, first, name, "push")?;
    Some(if sig.is(close + 1, Semicolon) {
        close + 2
    } else {
        close + 1
    })
}

/// `head = iter.filter(...).map(...)`, o la copia `[...iter]` si no transforma ni filtra.
fn chain(
    head: &str,
    target: &str,
    iter: &str,
    cond: Option<&str>,
    value: &str,
) -> std::string::String {
    let param = format!("({target})");
    let result = if value.starts_with('{') {
        format!("({value})")
    } else {
        value.to_owned()
    };
    let identity = value == target;
    match cond {
        None if identity => format!("{head} = [...{iter}];"),
        None => format!("{head} = {iter}.map({param} => {result});"),
        Some(cond) if identity => format!("{head} = {iter}.filter({param} => {cond});"),
        Some(cond) => {
            format!("{head} = {iter}.filter({param} => {cond}).map({param} => {result});")
        }
    }
}
