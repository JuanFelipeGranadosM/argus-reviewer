//! Expresiones de menor a mayor precedencia: secuencia, asignación (y flechas, `yield`), condicional,
//! binarias por potencia de enlace (con `as`/`satisfies`), unarias, postfijas y primarias.

use super::decl::{params, return_type};
use super::{allow_in, block, primary, types, Completed, Parser};
use crate::syntax::SyntaxKind::*;

/// Potencias de enlace: `??` y `||` las más bajas, `**` la más alta y asociativa por la derecha.
const RELATIONAL: u8 = 8;
const EXPONENT: u8 = 12;

/// `a, b, c`: una secuencia si hay coma.
pub(super) fn expression(p: &mut Parser) {
    let m = p.start();
    assignment(p);
    if p.at(Comma) {
        while p.eat(Comma) {
            assignment(p);
        }
        m.complete(p, SequenceExpr);
    } else {
        m.abandon(p);
    }
}

pub(super) fn assignment(p: &mut Parser) -> Option<Completed> {
    if let Some(arrow) = arrow(p) {
        return Some(arrow);
    }
    if p.at(KwYield) {
        return Some(yield_expr(p));
    }
    let target = conditional(p)?;
    let width = assignment_operator(p);
    if width == 0 {
        return Some(target);
    }
    let m = target.precede(p);
    for _ in 0..width {
        p.bump();
    }
    assignment(p);
    Some(m.complete(p, AssignExpr))
}

/// Tokens del operador de asignación actual (0 si no hay): `>>=` y `>>>=` llegan como `>` sueltos.
fn assignment_operator(p: &mut Parser) -> usize {
    match p.current() {
        Eq | PlusEq | MinusEq | StarEq | SlashEq | PercentEq | DoubleStarEq | AmpEq | PipeEq
        | CaretEq | LShiftEq | AmpAmpEq | PipePipeEq | QuestionQuestionEq => 1,
        Gt => match greater(p) {
            (width, None) => width,
            _ => 0,
        },
        _ => 0,
    }
}

/// Recompone los `>` adyacentes: devuelve los tokens que ocupa el operador y su potencia de enlace,
/// o `None` si es una asignación (`>>=`, `>>>=`).
fn greater(p: &mut Parser) -> (usize, Option<u8>) {
    let mut run = 1;
    while run < 3 && p.nth(run) == Gt && p.adjacent(run) {
        run += 1;
    }
    let eq = p.nth(run) == Eq && p.adjacent(run);
    match (run, eq) {
        (1, true) => (2, Some(RELATIONAL)),
        (1, false) => (1, Some(RELATIONAL)),
        (_, true) => (run + 1, None),
        (_, false) => (run, Some(9)),
    }
}

fn yield_expr(p: &mut Parser) -> Completed {
    let m = p.start();
    p.bump();
    p.eat(Star);
    let ends = matches!(
        p.current(),
        RParen | RBracket | RBrace | Comma | Semicolon | Colon | EndOfFile | Question
    );
    if !ends && !p.newline_before() {
        assignment(p);
    }
    m.complete(p, YieldExpr)
}

fn conditional(p: &mut Parser) -> Option<Completed> {
    let test = binary(p, 0)?;
    if !p.at(Question) {
        return Some(test);
    }
    let m = test.precede(p);
    p.bump();
    allow_in(p, |p| {
        assignment(p);
    });
    p.expect(Colon, "se esperaba `:` en la expresión condicional");
    assignment(p);
    Some(m.complete(p, ConditionalExpr))
}

/// Operador binario actual: tokens que ocupa y potencia de enlace.
fn operator(p: &mut Parser) -> Option<(usize, u8)> {
    let power = match p.current() {
        QuestionQuestion | PipePipe => 1,
        AmpAmp => 3,
        Pipe => 4,
        Caret => 5,
        Amp => 6,
        EqEq | NotEq | EqEqEq | NotEqEq => 7,
        Lt | LtEq | KwInstanceof => RELATIONAL,
        KwIn if !p.no_in => RELATIONAL,
        Gt => {
            let (width, power) = greater(p);
            return power.map(|power| (width, power));
        }
        LShift => 9,
        Plus | Minus => 10,
        Star | Slash | Percent => 11,
        DoubleStar => EXPONENT,
        _ => return None,
    };
    Some((1, power))
}

fn binary(p: &mut Parser, min: u8) -> Option<Completed> {
    let mut lhs = unary(p)?;
    loop {
        if RELATIONAL > min && type_operator(p) {
            let m = lhs.precede(p);
            p.bump();
            if !p.eat(KwConst) {
                types::type_(p);
            }
            lhs = m.complete(p, AsExpr);
            continue;
        }
        let Some((width, power)) = operator(p).filter(|&(_, power)| power > min) else {
            break;
        };
        let m = lhs.precede(p);
        for _ in 0..width {
            p.bump();
        }
        binary(p, if power == EXPONENT { power - 1 } else { power });
        lhs = m.complete(p, BinaryExpr);
    }
    Some(lhs)
}

/// `x as T`, `x as const` y `x satisfies T` de TypeScript, en la misma línea.
fn type_operator(p: &mut Parser) -> bool {
    p.typescript && (p.at_word("as") || p.at_word("satisfies")) && !p.newline_before()
}

fn unary(p: &mut Parser) -> Option<Completed> {
    match p.current() {
        Bang | Tilde | Plus | Minus | PlusPlus | MinusMinus | KwTypeof | KwVoid | KwDelete
        | KwAwait => {
            let m = p.start();
            p.bump();
            unary(p);
            Some(m.complete(p, UnaryExpr))
        }
        Lt if p.typescript && !p.jsx => {
            let m = p.start();
            types::type_args(p);
            unary(p);
            Some(m.complete(p, AsExpr))
        }
        _ => postfix(p),
    }
}

fn postfix(p: &mut Parser) -> Option<Completed> {
    let operand = primary::lhs(p)?;
    if matches!(p.current(), PlusPlus | MinusMinus) && !p.newline_before() {
        let m = operand.precede(p);
        p.bump();
        return Some(m.complete(p, PostfixExpr));
    }
    Some(operand)
}

/// Funciones flecha. `x =>` y `(…) =>` se ven por el token tras el nombre o el paréntesis de cierre;
/// con tipos (`(a: T): R =>`, `<T>(a) =>`) se intenta la cabecera y se deshace si no sigue `=>`.
fn arrow(p: &mut Parser) -> Option<Completed> {
    let is_async = p.at_word("async")
        && matches!(p.nth(1), Ident | LParen | Lt)
        && !p.newline_before_nth(1);
    let at = usize::from(is_async);
    match p.nth(at) {
        Ident if p.nth(at + 1) == FatArrow => Some(arrow_function(p)),
        LParen => {
            let close = p.close_of(p.pos() + at)?;
            match p.kind_at(close + 1) {
                FatArrow => Some(arrow_function(p)),
                Colon if p.typescript => speculative_arrow(p),
                _ => None,
            }
        }
        Lt if p.typescript => speculative_arrow(p),
        _ => None,
    }
}

fn speculative_arrow(p: &mut Parser) -> Option<Completed> {
    let checkpoint = p.checkpoint();
    let m = p.start();
    arrow_head(p);
    if p.at(FatArrow) && !p.errors_since(checkpoint) {
        return Some(arrow_body(p, m));
    }
    m.abandon(p);
    p.rewind(checkpoint);
    None
}

fn arrow_function(p: &mut Parser) -> Completed {
    let m = p.start();
    arrow_head(p);
    arrow_body(p, m)
}

fn arrow_head(p: &mut Parser) {
    if p.at_word("async") && p.nth(1) != FatArrow {
        p.bump();
    }
    if p.at(Ident) {
        let param = p.start();
        p.bump();
        param.complete(p, Param);
        return;
    }
    if p.at(Lt) {
        types::type_params(p);
    }
    if p.at(LParen) {
        params(p);
    } else {
        p.error("se esperaba `(`");
    }
    if p.at(Colon) {
        return_type(p);
    }
}

fn arrow_body(p: &mut Parser, m: super::Marker) -> Completed {
    p.expect(FatArrow, "se esperaba `=>`");
    if p.at(LBrace) {
        block(p);
    } else {
        assignment(p);
    }
    m.complete(p, ArrowFunction)
}
