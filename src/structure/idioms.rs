//! Construcciones que las guías de estilo de cada lenguaje desaconsejan sin discusión: comparar con
//! `None` por igualdad (PEP 8), `==` laxo, `var` y `debugger` en JS/TS, asignar una `lambda` a un
//! nombre (PEP 8). Solo se reconocen formas inequívocas sobre los tokens.

use super::sig::Sig;
use super::{Idiom, IdiomKind};
use crate::syntax::SyntaxKind::*;

pub(super) fn python(sig: &Sig, i: usize) -> Option<Idiom> {
    let kind = match sig.kind(i)? {
        EqEq | NotEq if sig.prev(i) == Some(KwNone) || sig.is(i + 1, KwNone) => {
            IdiomKind::NoneEquality
        }
        KwLambda if is_lambda_assignment(sig, i) => IdiomKind::LambdaAssignment,
        _ => return None,
    };
    Some(Idiom {
        kind,
        span: sig.span(i, i),
    })
}

pub(super) fn javascript(sig: &Sig, i: usize) -> Option<Idiom> {
    if matches!(sig.prev(i), Some(Dot | QuestionDot)) {
        return None;
    }
    let kind = match sig.kind(i)? {
        EqEq | NotEq if !nullish_operand(sig, i) => IdiomKind::LooseEquality,
        KwVar if !i.checked_sub(1).is_some_and(|p| sig.is_word(p, "declare")) => {
            IdiomKind::VarDeclaration
        }
        KwDebugger => IdiomKind::Debugger,
        _ => return None,
    };
    Some(Idiom {
        kind,
        span: sig.span(i, i),
    })
}

/// `x == null` y `x != undefined` comprueban a la vez `null` y `undefined`: es el único `==`
/// que admiten las guías.
fn nullish_operand(sig: &Sig, i: usize) -> bool {
    let nullish = |j: usize| sig.is(j, KwNull) || sig.is_word(j, "undefined");
    i.checked_sub(1).is_some_and(nullish) || nullish(i + 1)
}

/// `nombre = lambda ...` al inicio de una sentencia (también en el cuerpo de una clase).
fn is_lambda_assignment(sig: &Sig, i: usize) -> bool {
    let (Some(eq), Some(name)) = (i.checked_sub(1), i.checked_sub(2)) else {
        return false;
    };
    let statement_start = name.checked_sub(1).is_none_or(|p| {
        matches!(
            sig.kind(p),
            Some(LogicalNewline | Indent | Dedent | Semicolon)
        )
    });
    sig.is(eq, Eq) && sig.is(name, Ident) && statement_start
}
