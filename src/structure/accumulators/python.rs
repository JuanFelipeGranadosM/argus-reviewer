//! `r = []` seguido de `for x in xs:` cuyo cuerpo solo hace `r.append(...)` (o `s.add(...)`,
//! `d[k] = v`), opcionalmente bajo un `if`: equivale a una comprensión.

use super::{comments_note, independent, insertion, piece, Range};
use crate::structure::sig::Sig;
use crate::structure::{Accumulator, Collection};
use crate::syntax::SyntaxKind::{self, *};

/// Longitud de línea por encima de la cual una comprensión deja de ser más clara que el bucle.
const MAX_LINE: usize = 88;

/// Lo que se inserta en cada vuelta: el valor y, en un diccionario, la clave.
struct Item {
    key: Option<Range>,
    value: Range,
    /// Último token de la inserción.
    last: usize,
}

pub(in crate::structure) fn find(sig: &Sig, i: usize) -> Option<Accumulator> {
    let starts = matches!(sig.prev(i), None | Some(LogicalNewline | Indent | Dedent));
    if !starts || !sig.is(i, Ident) {
        return None;
    }
    let name = sig.text(i);
    let last = sig.len().checked_sub(1)?;
    let newline = sig.find_top_level(i + 1, last, |k| k == LogicalNewline)?;
    let eq = sig.find_top_level(i + 1, newline, |k| k == Eq)?;
    let collection = empty_collection(sig, eq + 1, newline)?;
    let for_at = newline + 1;
    if !sig.is(for_at, KwFor) {
        return None;
    }
    let in_at = stop_at(sig, for_at, last, KwIn)?;
    let colon = stop_at(sig, in_at, last, Colon)?;
    let (stmt, end) = suite(sig, colon)?;
    if sig.is(end, KwElse) {
        return None;
    }
    let (cond, add) = if sig.is(stmt, KwIf) {
        let if_colon = stop_at(sig, stmt, last, Colon)?;
        (Some((stmt + 1, if_colon - 1)), suite(sig, if_colon)?.0)
    } else {
        (None, stmt)
    };
    let item = inserted(sig, add, name, collection)?;
    let target = (for_at + 1, in_at - 1);
    let iter = (in_at + 1, colon - 1);
    if !safe(sig, (i, end), name, (target, iter), (cond, &item)) {
        return None;
    }
    let pieces = Pieces {
        head: piece(sig, (i, eq - 1))?,
        target: piece(sig, target)?,
        iter: piece(sig, iter)?,
        cond: cond
            .map(|c| piece(sig, c))
            .map_or(Some(None), |p| p.map(Some))?,
        key: item
            .key
            .map(|k| piece(sig, k))
            .map_or(Some(None), |p| p.map(Some))?,
        value: piece(sig, item.value)?,
    };
    let replacement = pieces.comprehension(collection);
    (replacement.chars().count() <= MAX_LINE).then(|| Accumulator {
        name: name.to_owned(),
        span: sig.span(i, item.last),
        replacement,
        collection,
        caveat: comments_note(sig, (i, item.last), &["#"]).map(str::to_owned),
    })
}

/// La comprensión se comporta igual que el bucle: nada nombra la colección, la variable del bucle no
/// se lee después, ni la condición ni el valor dependen del ámbito (`locals()`, `vars()`, `super()`) y
/// el bucle no está en el cuerpo de una clase, donde una comprensión no ve los atributos.
fn safe(
    sig: &Sig,
    (i, end): (usize, usize),
    name: &str,
    (target, iter): (Range, Range),
    (cond, item): (Option<Range>, &Item),
) -> bool {
    let inner = [cond, item.key, Some(item.value)];
    let all = [Some(target), Some(iter)].into_iter().chain(inner);
    all.flatten().all(|r| independent(sig, r, name))
        && !inner.into_iter().flatten().any(|r| scope_sensitive(sig, r))
        && !leaks_target(sig, target, end)
        && !in_class_body(sig, i)
}

fn scope_sensitive(sig: &Sig, (first, last): Range) -> bool {
    (first..=last).any(|j| {
        ["locals", "vars", "super"]
            .iter()
            .any(|w| sig.is_word(j, w))
            && sig.is(j + 1, LParen)
    })
}

/// La sentencia en `i` está directamente en el cuerpo de una `class`.
fn in_class_body(sig: &Sig, i: usize) -> bool {
    let mut depth = 0u32;
    let mut j = i;
    while let Some(prev) = j.checked_sub(1) {
        j = prev;
        match sig.kind(j) {
            Some(Dedent) => depth += 1,
            Some(Indent) if depth == 0 => break,
            Some(Indent) => depth -= 1,
            _ => {}
        }
    }
    if !sig.is(j, Indent) {
        return false;
    }
    let mut header = j.saturating_sub(1);
    while header > 0 && !matches!(sig.prev(header), Some(LogicalNewline | Indent | Dedent)) {
        header -= 1;
    }
    sig.is(header, KwClass)
}

/// Siguiente `kind` de primer nivel tras `from`, sin pasar de la línea lógica.
fn stop_at(sig: &Sig, from: usize, last: usize, kind: SyntaxKind) -> Option<usize> {
    sig.find_top_level(from + 1, last, |k| k == kind || k == LogicalNewline)
        .filter(|&j| sig.is(j, kind))
}

/// `[]`, `{}`, `list()`, `set()` o `dict()` ocupando exactamente `first..end`.
fn empty_collection(sig: &Sig, first: usize, end: usize) -> Option<Collection> {
    let pair = |open, close| end == first + 2 && sig.is(first, open) && sig.is(first + 1, close);
    if pair(LBracket, RBracket) {
        return Some(Collection::List);
    }
    if pair(LBrace, RBrace) {
        return Some(Collection::Dict);
    }
    let call = end == first + 3 && sig.is(first + 1, LParen) && sig.is(first + 2, RParen);
    match sig.text(first) {
        "list" if call => Some(Collection::List),
        "set" if call => Some(Collection::Set),
        "dict" if call => Some(Collection::Dict),
        _ => None,
    }
}

/// Cuerpo de una sola sentencia tras `colon`: su primer token y el índice que sigue al cuerpo.
fn suite(sig: &Sig, colon: usize) -> Option<(usize, usize)> {
    let last = sig.len().checked_sub(1)?;
    if sig.is(colon + 1, LogicalNewline) {
        let first = colon + 2;
        let end = sig
            .is(first, Indent)
            .then(|| statement_end(sig, first + 1))??;
        return sig.is(end, Dedent).then_some((first + 1, end + 1));
    }
    let first = colon + 1;
    let newline = sig.find_top_level(first, last, |k| k == LogicalNewline || k == Semicolon)?;
    sig.is(newline, LogicalNewline)
        .then_some((first, newline + 1))
}

/// Índice que sigue a la sentencia simple o al `if` (sin `elif`/`else`) que empieza en `first`.
fn statement_end(sig: &Sig, first: usize) -> Option<usize> {
    let last = sig.len().checked_sub(1)?;
    if sig.is(first, KwIf) {
        let colon = stop_at(sig, first, last, Colon)?;
        return suite(sig, colon).map(|(_, end)| end);
    }
    let newline = sig.find_top_level(first, last, |k| k == LogicalNewline || k == Semicolon)?;
    sig.is(newline, LogicalNewline).then_some(newline + 1)
}

/// `name.append(v)`, `name.add(v)` o `name[k] = v` como sentencia completa.
fn inserted(sig: &Sig, at: usize, name: &str, collection: Collection) -> Option<Item> {
    let method = match collection {
        Collection::List => "append",
        Collection::Set => "add",
        Collection::Dict => return dict_item(sig, at, name),
    };
    let (value, close) = insertion(sig, at, name, method)?;
    sig.is(close + 1, LogicalNewline).then_some(Item {
        key: None,
        value,
        last: close,
    })
}

fn dict_item(sig: &Sig, at: usize, name: &str) -> Option<Item> {
    let open = at + 1;
    let close = sig
        .close_of(open)
        .filter(|_| sig.is_word(at, name) && sig.is(open, LBracket))?;
    let last = sig.len().checked_sub(1)?;
    let newline = sig.find_top_level(close + 2, last, |k| k == LogicalNewline || k == Semicolon)?;
    let ok = sig.is(close + 1, Eq) && sig.is(newline, LogicalNewline) && newline > close + 2;
    ok.then_some(Item {
        key: Some((open + 1, close.checked_sub(1)?)),
        value: (close + 2, newline - 1),
        last: newline - 1,
    })
}

/// La variable del bucle sigue viva tras un `for` pero no tras una comprensión: si el bloque que
/// sigue la lee antes de volver a asignarla, la reescritura cambiaría el programa.
fn leaks_target(sig: &Sig, (first, last): Range, end: usize) -> bool {
    let mut names: Vec<&str> = (first..=last)
        .filter(|&j| sig.is(j, Ident))
        .map(|j| sig.text(j))
        .collect();
    let mut depth = 0u32;
    for j in end..sig.len() {
        match sig.kind(j) {
            Some(Indent) => depth += 1,
            Some(Dedent) if depth == 0 => return false,
            Some(Dedent) => depth -= 1,
            Some(Ident) if sig.prev(j) != Some(Dot) && names.contains(&sig.text(j)) => {
                if !rebinds(sig, j) {
                    return true;
                }
                names.retain(|n| *n != sig.text(j));
            }
            _ => {}
        }
    }
    false
}

/// El nombre en `j` se asigna de nuevo sin leerse: `x = ...` al inicio de una sentencia, o destino
/// de otro `for` (`for x in`, `for i, x in`).
fn rebinds(sig: &Sig, j: usize) -> bool {
    let starts = matches!(sig.prev(j), Some(LogicalNewline | Indent | Dedent));
    if starts && sig.is(j + 1, Eq) {
        return true;
    }
    let mut k = j;
    while let Some(prev) = k.checked_sub(1) {
        match sig.kind(prev) {
            Some(KwFor) => return true,
            Some(Ident | Comma | LParen | LBracket | RParen | RBracket) if j - prev < 16 => {
                k = prev
            }
            _ => return false,
        }
    }
    false
}

struct Pieces<'a> {
    head: &'a str,
    target: &'a str,
    iter: &'a str,
    cond: Option<&'a str>,
    key: Option<&'a str>,
    value: &'a str,
}

impl Pieces<'_> {
    fn comprehension(&self, collection: Collection) -> std::string::String {
        let Pieces {
            head, target, iter, ..
        } = self;
        let copy = self.cond.is_none() && self.key.is_none() && self.value == *target;
        match collection {
            Collection::List if copy => return format!("{head} = list({iter})"),
            Collection::Set if copy => return format!("{head} = set({iter})"),
            _ => {}
        }
        let item = match self.key {
            Some(key) => format!("{key}: {}", self.value),
            None => self.value.to_owned(),
        };
        let filter = self.cond.map(|c| format!(" if {c}")).unwrap_or_default();
        let (open, close) = match collection {
            Collection::List => ("[", "]"),
            Collection::Set | Collection::Dict => ("{", "}"),
        };
        format!("{head} = {open}{item} for {target} in {iter}{filter}{close}")
    }
}
