//! Expresiones por niveles de precedencia, de menor a mayor: `lambda`, ternario, `or`, `and`, `not`,
//! comparaciones, operadores binarios (por potencia de enlace), unarios, `**`, `await` y primarios.

pub(super) use super::atom::arg_list;
use super::{atom, params, starts_expr, Completed, Parser};
use crate::syntax::SyntaxKind::{self, *};

/// `a, *b, c` sin paréntesis: una tupla si hay coma.
pub(super) fn star_expressions(p: &mut Parser) {
    let m = p.start();
    star_item(p);
    if p.at(Comma) {
        while p.eat(Comma) {
            let next = p.current();
            if !starts_expr(next) || next == KwYield {
                break;
            }
            star_item(p);
        }
        m.complete(p, TupleExpr);
    } else {
        m.abandon(p);
    }
}

/// Patrones de un `case`: como [`star_expressions`] pero sin ternario, para que el `if` sea la guarda.
pub(super) fn patterns(p: &mut Parser) {
    let m = p.start();
    pattern(p);
    if p.at(Comma) {
        while p.eat(Comma) && starts_expr(p.current()) {
            pattern(p);
        }
        m.complete(p, TupleExpr);
    } else {
        m.abandon(p);
    }
}

fn pattern(p: &mut Parser) {
    if p.at(Star) {
        let m = p.start();
        p.bump();
        bitwise(p, 0);
        m.complete(p, StarredExpr);
    } else {
        disjunction(p);
    }
}

pub(super) fn star_expressions_or_yield(p: &mut Parser) {
    if p.at(KwYield) {
        yield_expr(p);
    } else {
        star_expressions(p);
    }
}

fn star_item(p: &mut Parser) {
    if p.at(Star) {
        let m = p.start();
        p.bump();
        bitwise(p, 0);
        m.complete(p, StarredExpr);
    } else {
        named_expr(p);
    }
}

pub(super) fn yield_expr(p: &mut Parser) {
    let m = p.start();
    p.bump();
    if p.eat(KwFrom) {
        expression(p);
    } else if starts_expr(p.current()) {
        star_expressions(p);
    }
    m.complete(p, YieldExpr);
}

/// `nombre := valor` o una expresión.
pub(super) fn named_expr(p: &mut Parser) {
    if p.at(Ident) && p.nth(1) == ColonEq {
        let m = p.start();
        let name = p.start();
        p.bump();
        name.complete(p, NameExpr);
        p.bump();
        expression(p);
        m.complete(p, NamedExpr);
    } else {
        expression(p);
    }
}

pub(super) fn expression(p: &mut Parser) -> Option<Completed> {
    if p.at(KwLambda) {
        return Some(lambda(p));
    }
    let condition = disjunction(p)?;
    if !p.at(KwIf) {
        return Some(condition);
    }
    let m = condition.precede(p);
    p.bump();
    disjunction(p);
    p.expect(KwElse, "se esperaba `else` en la expresión condicional");
    expression(p);
    Some(m.complete(p, ConditionalExpr))
}

fn lambda(p: &mut Parser) -> Completed {
    let m = p.start();
    p.bump();
    if !p.at(Colon) {
        params::lambda_params(p);
    }
    p.expect(Colon, "se esperaba `:` en la lambda");
    expression(p);
    m.complete(p, LambdaExpr)
}

/// `or` y `and` encadenados, sin ternario: es el nivel de los iterables de una comprensión.
pub(super) fn disjunction(p: &mut Parser) -> Option<Completed> {
    let mut lhs = conjunction(p)?;
    while p.at(KwOr) {
        let m = lhs.precede(p);
        p.bump();
        conjunction(p);
        lhs = m.complete(p, BinaryExpr);
    }
    Some(lhs)
}

fn conjunction(p: &mut Parser) -> Option<Completed> {
    let mut lhs = inversion(p)?;
    while p.at(KwAnd) {
        let m = lhs.precede(p);
        p.bump();
        inversion(p);
        lhs = m.complete(p, BinaryExpr);
    }
    Some(lhs)
}

fn inversion(p: &mut Parser) -> Option<Completed> {
    if p.at(KwNot) {
        let m = p.start();
        p.bump();
        inversion(p);
        return Some(m.complete(p, UnaryExpr));
    }
    comparison(p)
}

fn comparison(p: &mut Parser) -> Option<Completed> {
    let mut lhs = bitwise(p, 0)?;
    loop {
        let width = match (p.current(), p.nth(1)) {
            (KwNot, KwIn) | (KwIs, KwNot) => 2,
            (Lt | Gt | EqEq | GtEq | LtEq | NotEq | KwIn | KwIs, _) => 1,
            _ => break,
        };
        let m = lhs.precede(p);
        for _ in 0..width {
            p.bump();
        }
        bitwise(p, 0);
        lhs = m.complete(p, CompareExpr);
    }
    Some(lhs)
}

/// Potencia de enlace de los operadores binarios de `|` a `*`.
fn binding_power(kind: SyntaxKind) -> Option<u8> {
    Some(match kind {
        Pipe => 1,
        Caret => 2,
        Amp => 3,
        LShift | RShift => 4,
        Plus | Minus => 5,
        Star | Slash | DoubleSlash | Percent | At => 6,
        _ => return None,
    })
}

/// Operadores binarios con potencia de enlace mayor que `min`: es el nivel de los destinos de un `for`,
/// que no pueden contener comparaciones (el `in` pertenece al `for`).
pub(super) fn bitwise(p: &mut Parser, min: u8) -> Option<Completed> {
    let mut lhs = unary(p)?;
    while let Some(power) = binding_power(p.current()).filter(|&b| b > min) {
        let m = lhs.precede(p);
        p.bump();
        bitwise(p, power);
        lhs = m.complete(p, BinaryExpr);
    }
    Some(lhs)
}

fn unary(p: &mut Parser) -> Option<Completed> {
    if matches!(p.current(), Minus | Plus | Tilde) {
        let m = p.start();
        p.bump();
        unary(p);
        return Some(m.complete(p, UnaryExpr));
    }
    let base = await_primary(p)?;
    if !p.at(DoubleStar) {
        return Some(base);
    }
    let m = base.precede(p);
    p.bump();
    unary(p);
    Some(m.complete(p, BinaryExpr))
}

fn await_primary(p: &mut Parser) -> Option<Completed> {
    if p.at(KwAwait) {
        let m = p.start();
        p.bump();
        atom::primary(p);
        return Some(m.complete(p, AwaitExpr));
    }
    atom::primary(p)
}

/// Destinos de un `for` o de un `as`: `x`, `a, *b`, `(a, b)`, `obj.attr[i]`.
pub(super) fn targets(p: &mut Parser) {
    let m = p.start();
    target(p);
    if p.at(Comma) {
        while p.eat(Comma) && (starts_expr(p.current()) && !p.at(KwIn)) {
            target(p);
        }
        m.complete(p, TupleExpr);
    } else {
        m.abandon(p);
    }
}

pub(super) fn target(p: &mut Parser) {
    if p.at(Star) {
        let m = p.start();
        p.bump();
        bitwise(p, 0);
        m.complete(p, StarredExpr);
    } else {
        bitwise(p, 0);
    }
}
