//! Hechos de JavaScript/TypeScript que no son funciones: capturas de errores, `any` explícito,
//! evaluación dinámica de código y puntos de decisión para la complejidad.

use super::sig::Sig;
use super::{Catch, Outline};
use crate::syntax::SyntaxKind::*;

pub(super) fn collect(sig: &Sig, i: usize, typescript: bool, outline: &mut Outline) {
    match sig.kind(i) {
        Some(KwCatch) if after_dot(sig, i) => outline.catches.extend(promise_catch(sig, i)),
        Some(KwCatch) => outline.catches.extend(catch_clause(sig, i)),
        Some(Ident) if typescript && is_explicit_any(sig, i) => {
            outline.explicit_any.push(sig.span(i, i));
        }
        Some(Ident) if is_dynamic_eval(sig, i) => outline.dynamic_evals.push(sig.span(i, i)),
        _ => {}
    }
}

/// Cuenta `if`, bucles, `case`, `catch`, `&&`, `||`, `??` (y sus asignaciones) y el `?` ternario.
pub(super) fn is_decision(sig: &Sig, i: usize) -> bool {
    if after_dot(sig, i) {
        return false;
    }
    match sig.kind(i) {
        Some(
            KwIf | KwFor | KwWhile | KwCase | KwCatch | AmpAmp | PipePipe | QuestionQuestion
            | AmpAmpEq | PipePipeEq | QuestionQuestionEq,
        ) => true,
        Some(Question) => !matches!(
            sig.kind(i + 1),
            Some(Colon | RParen | Comma | Eq | RBracket)
        ),
        _ => false,
    }
}

fn after_dot(sig: &Sig, i: usize) -> bool {
    matches!(sig.prev(i), Some(Dot | QuestionDot))
}

/// `catch (e) { }` sin nada dentro. Un comentario cuenta como explicación y no se marca.
fn catch_clause(sig: &Sig, i: usize) -> Option<Catch> {
    let mut j = i + 1;
    if sig.is(j, LParen) {
        j = sig.close_of(j)? + 1;
    }
    if !sig.is(j, LBrace) {
        return None;
    }
    let close = sig.close_of(j)?;
    let silent = close == j + 1 && sig.between(j, close).trim().is_empty();
    Some(Catch {
        span: sig.span(i, close),
        silent,
    })
}

/// `.catch(() => {})` o `.catch(function () {})`: la promesa rechazada se traga en silencio.
fn promise_catch(sig: &Sig, i: usize) -> Option<Catch> {
    if !sig.is(i + 1, LParen) {
        return None;
    }
    let close = sig.close_of(i + 1)?;
    let rbrace = close.checked_sub(1)?;
    let lbrace = rbrace.checked_sub(1)?;
    let empty_block = sig.is(lbrace, LBrace)
        && sig.close_of(lbrace) == Some(rbrace)
        && sig.between(lbrace, rbrace).trim().is_empty();
    let silent = empty_block && matches!(sig.prev(lbrace), Some(FatArrow | RParen));
    Some(Catch {
        span: sig.span(i, close),
        silent,
    })
}

/// `any` en posición de tipo: `x: any`, `as any`, `Array<any>`, `A | any`, `type T = any`. No se
/// marca un nombre `any` (`{ any: 1 }`, `(a, any) => ...`) ni una propiedad `x.any`: tras una coma
/// solo es tipo dentro de `<...>` o `[...]`, o si sigue un operador de tipos.
fn is_explicit_any(sig: &Sig, i: usize) -> bool {
    if !sig.is_word(i, "any") || after_dot(sig, i) {
        return false;
    }
    let type_follows = matches!(sig.kind(i + 1), Some(Gt | RBracket | LBracket | Pipe | Amp));
    match sig.prev(i) {
        Some(Colon | Lt | Pipe | Amp | FatArrow | Eq) => true,
        Some(Comma) => type_follows,
        Some(Ident) => i.checked_sub(1).is_some_and(|p| sig.is_word(p, "as")),
        _ => false,
    }
}

/// `eval(...)` y `new Function(...)`, que ejecutan texto como código.
fn is_dynamic_eval(sig: &Sig, i: usize) -> bool {
    if !sig.is(i + 1, LParen) || after_dot(sig, i) {
        return false;
    }
    sig.is_word(i, "eval") || (sig.is_word(i, "Function") && sig.prev(i) == Some(KwNew))
}
