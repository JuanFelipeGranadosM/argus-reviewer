//! Funciones de JavaScript/TypeScript: declaraciones y expresiones `function`, flechas y métodos de
//! clases u objetos.

use super::js_facts;
use super::sig::Sig;
use super::{Function, Outline, Param};
use crate::syntax::SyntaxKind::{self, *};

/// Tope de tokens al saltar una anotación de tipo o buscar la declaración de un nombre: evita
/// recorridos cuadráticos en código que no tiene la forma esperada.
const MAX_LOOKAROUND: usize = 64;

pub(super) fn outline(sig: &Sig, typescript: bool) -> Outline {
    let mut outline = Outline::default();
    for i in 0..sig.len() {
        let function = match sig.kind(i) {
            Some(KwFunction) => function_keyword(sig, i),
            Some(LParen) => arrow_with_parens(sig, i).or_else(|| method(sig, i)),
            Some(Ident) if sig.is(i + 1, FatArrow) => arrow_single(sig, i),
            _ => None,
        };
        outline.functions.extend(function);
        js_facts::collect(sig, i, typescript, &mut outline);
    }
    outline
}

fn function_keyword(sig: &Sig, i: usize) -> Option<Function> {
    let mut j = i + 1;
    if sig.is(j, Star) {
        j += 1;
    }
    let mut name = None;
    if sig.is(j, Ident) {
        name = Some(sig.text(j).to_owned());
        j += 1;
    }
    if sig.is(j, Lt) {
        j = skip_angles(sig, j)?;
    }
    if !sig.is(j, LParen) {
        return None;
    }
    let close = sig.close_of(j)?;
    let has_return = sig.is(close + 1, Colon);
    let body = if has_return {
        skip_type(sig, close + 2)
    } else {
        close + 1
    };
    if !sig.is(body, LBrace) {
        return None;
    }
    let last = sig.close_of(body)?;
    let first = async_before(sig, i);
    let name = name.or_else(|| binding_name(sig, first));
    Some(Function::new(sig, name, (first, last)).with_signature(params(sig, j, close), has_return))
}

fn arrow_with_parens(sig: &Sig, open: usize) -> Option<Function> {
    let close = sig.close_of(open)?;
    let has_return = sig.is(close + 1, Colon);
    let arrow = if has_return {
        skip_type(sig, close + 2)
    } else {
        close + 1
    };
    if !sig.is(arrow, FatArrow) {
        return None;
    }
    let mut first = open;
    if sig.prev(open) == Some(Gt) {
        first = angles_back(sig, open - 1)?;
    }
    let first = async_before(sig, first);
    let last = arrow_body_end(sig, arrow + 1)?;
    let name = binding_name(sig, first);
    Some(Function::new(sig, name, (first, last)).with_signature(params(sig, open, close), has_return))
}

/// `x => ...`. Se descarta la `T` de `(a): T => ...`, que es el tipo de retorno de otra flecha.
fn arrow_single(sig: &Sig, i: usize) -> Option<Function> {
    let return_type = sig.prev(i) == Some(Colon) && i >= 2 && sig.is(i - 2, RParen);
    if return_type || matches!(sig.prev(i), Some(Dot | QuestionDot | Pipe | Amp)) {
        return None;
    }
    let last = arrow_body_end(sig, i + 2)?;
    let first = async_before(sig, i);
    let param = Param {
        name: sig.text(i).to_owned(),
        annotated: false,
    };
    let name = binding_name(sig, first);
    Some(Function::new(sig, name, (first, last)).with_signature(vec![param], false))
}

/// `nombre(...) {` en una clase u objeto literal, incluidos getters, setters y `constructor`.
fn method(sig: &Sig, open: usize) -> Option<Function> {
    let mut name_at = open.checked_sub(1)?;
    if sig.is(name_at, Gt) {
        name_at = angles_back(sig, name_at)?.checked_sub(1)?;
    }
    let kind = sig.kind(name_at)?;
    let named = matches!(kind, Ident | String | PrivateName | Int) || kind.is_keyword();
    let generator_function = sig.prev(name_at) == Some(Star)
        && name_at >= 2
        && sig.is(name_at - 2, KwFunction);
    let declared = matches!(sig.prev(name_at), Some(Dot | QuestionDot | KwFunction));
    if !named || is_control(kind) || declared || generator_function {
        return None;
    }
    let close = sig.close_of(open)?;
    let has_return = sig.is(close + 1, Colon);
    let body = if has_return {
        skip_type(sig, close + 2)
    } else {
        close + 1
    };
    if !sig.is(body, LBrace) {
        return None;
    }
    let last = sig.close_of(body)?;
    let name = Some(clean_name(sig.text(name_at)));
    Some(Function::new(sig, name, (name_at, last)).with_signature(params(sig, open, close), has_return))
}

/// Palabras cuya `(...) {` no es un método: sentencias de control y `function`, que tiene su ruta.
fn is_control(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        KwIf | KwFor | KwWhile | KwSwitch | KwCatch | KwWith | KwFunction | KwReturn
    )
}

fn params(sig: &Sig, open: usize, close: usize) -> Vec<Param> {
    sig.segments(open, close, true)
        .into_iter()
        .map(|(first, last)| {
            let mut at = first;
            while at < last && (sig.is(at, Ellipsis) || is_modifier(sig.text(at))) {
                at += 1;
            }
            let name = match sig.kind(at) {
                Some(LBrace | LBracket) => "{…}".to_owned(),
                _ => sig.text(at).to_owned(),
            };
            let annotated = sig.find_top_level(at, last, |k| k == Colon).is_some();
            Param { name, annotated }
        })
        .collect()
}

fn is_modifier(word: &str) -> bool {
    matches!(
        word,
        "public" | "private" | "protected" | "readonly" | "override"
    )
}

/// Final de una flecha: su bloque `{...}`, o la expresión hasta el primer separador o cierre.
fn arrow_body_end(sig: &Sig, start: usize) -> Option<usize> {
    if sig.is(start, LBrace) {
        return sig.close_of(start);
    }
    let (mut j, mut last) = (start, None);
    while let Some(kind) = sig.kind(j) {
        if j > start && sig.break_before(j) && starts_statement(sig, j) {
            break;
        }
        match kind {
            LParen | LBracket | LBrace => {
                let Some(close) = sig.close_of(j) else {
                    break;
                };
                last = Some(close);
                j = close + 1;
            }
            Comma | Semicolon | RParen | RBracket | RBrace => break,
            _ => {
                last = Some(j);
                j += 1;
            }
        }
    }
    last
}

fn starts_statement(sig: &Sig, j: usize) -> bool {
    let keyword = matches!(
        sig.kind(j),
        Some(
            KwConst
                | KwVar
                | KwFunction
                | KwClass
                | KwExport
                | KwImport
                | KwReturn
                | KwIf
                | KwFor
                | KwWhile
                | KwThrow
                | KwTry
                | KwSwitch
                | KwEnum
        )
    );
    keyword || ["let", "type", "interface"].iter().any(|w| sig.is_word(j, w))
}

/// Salta la anotación de tipo que empieza en `start` y devuelve dónde termina: la `{` del cuerpo, la
/// `=>` de una flecha o el separador que cierra una declaración sin cuerpo.
fn skip_type(sig: &Sig, start: usize) -> usize {
    let (mut j, mut angle) = (start, 0u32);
    for _ in 0..MAX_LOOKAROUND {
        match sig.kind(j) {
            None => return j,
            Some(LBrace) if angle == 0 && !continues_type(sig, j) => return j,
            Some(FatArrow | Semicolon | Comma | Eq | RParen | RBracket | RBrace) if angle == 0 => {
                return j
            }
            Some(LParen | LBracket | LBrace) => j = sig.close_of(j).unwrap_or(j),
            Some(Lt) => angle += 1,
            Some(Gt) => angle = angle.saturating_sub(1),
            _ => {}
        }
        j += 1;
    }
    j
}

/// Una `{` tras `:`, `|`, `&`, ... abre un tipo objeto (`(): { a: number } {`), no el cuerpo.
fn continues_type(sig: &Sig, j: usize) -> bool {
    matches!(
        sig.prev(j),
        Some(Colon | Pipe | Amp | Lt | Comma | FatArrow | Question | LParen | LBracket)
    ) || j.checked_sub(1).is_some_and(|p| sig.is_word(p, "keyof"))
}

/// Índice tras el `>` que cierra el `<` de `open`.
fn skip_angles(sig: &Sig, open: usize) -> Option<usize> {
    let mut depth = 0u32;
    for j in open..open + MAX_LOOKAROUND {
        match sig.kind(j)? {
            Lt => depth += 1,
            Gt => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(j + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Índice del `<` que abre el `>` de `close`.
fn angles_back(sig: &Sig, close: usize) -> Option<usize> {
    let mut depth = 0u32;
    for back in 0..MAX_LOOKAROUND {
        let j = close.checked_sub(back)?;
        match sig.kind(j)? {
            Gt => depth += 1,
            Lt => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

fn async_before(sig: &Sig, i: usize) -> usize {
    match i.checked_sub(1) {
        Some(p) if sig.is_word(p, "async") => p,
        _ => i,
    }
}

/// Nombre de una función anónima según dónde se asigna: `const total = () => ...` o `total: () => ...`.
fn binding_name(sig: &Sig, first: usize) -> Option<std::string::String> {
    let before = first.checked_sub(1)?;
    let target = match sig.kind(before)? {
        Colon => before.checked_sub(1)?,
        Eq => declared_name(sig, before).or_else(|| before.checked_sub(1))?,
        _ => return None,
    };
    matches!(sig.kind(target), Some(Ident | String | PrivateName))
        .then(|| clean_name(sig.text(target)))
}

/// `const nombre: Tipo = ...`: el nombre tras `const`/`let`/`var`, saltando la anotación.
fn declared_name(sig: &Sig, eq: usize) -> Option<usize> {
    for back in 1..MAX_LOOKAROUND {
        let k = eq.checked_sub(back)?;
        if matches!(sig.kind(k), Some(KwConst | KwVar)) || sig.is_word(k, "let") {
            return Some(k + 1);
        }
        if matches!(sig.kind(k), Some(Semicolon | LBrace | RBrace)) {
            return None;
        }
    }
    None
}

fn clean_name(text: &str) -> std::string::String {
    text.trim_matches(['"', '\'', '`']).to_owned()
}
