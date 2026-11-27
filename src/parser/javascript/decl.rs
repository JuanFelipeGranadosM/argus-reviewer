//! Funciones y sus firmas: parámetros de tipo, parámetros, tipo de retorno y decoradores.

use super::{allow_in, annotation, block, expr, is_modifier, is_name, primary, semi, stmt, types};
use super::{Completed, Parser};
use crate::syntax::SyntaxKind::*;

/// Modificadores de un parámetro de constructor en TypeScript (`constructor(private readonly x)`).
const PARAM_MODIFIERS: &[&str] = &["public", "private", "protected", "readonly", "override"];

/// `function`, `function*` o `async function`. Como sentencia de TypeScript puede no tener cuerpo
/// (sobrecargas y `declare function`).
pub(super) fn function(p: &mut Parser, expression: bool) -> Completed {
    let m = p.start();
    if p.at_word("async") {
        p.bump();
    }
    p.expect(KwFunction, "se esperaba `function`");
    p.eat(Star);
    if is_name(p.current()) {
        p.bump();
    }
    signature(p);
    if p.at(LBrace) || (expression && !p.typescript) {
        block(p);
    } else {
        semi(p);
    }
    m.complete(p, FunctionDef)
}

/// `<T>(params): Retorno`, compartida por funciones, métodos y firmas de tipos.
pub(super) fn signature(p: &mut Parser) {
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

pub(super) fn return_type(p: &mut Parser) {
    let m = p.start();
    p.bump();
    types::return_type(p);
    m.complete(p, ReturnType);
}

pub(super) fn params(p: &mut Parser) {
    let m = p.start();
    let open = p.pos();
    p.bump();
    allow_in(p, |p| {
        while !matches!(p.current(), RParen | EndOfFile) {
            param(p);
            if !p.eat(Comma) {
                break;
            }
        }
    });
    p.close_group(open, RParen);
    m.complete(p, ParamList);
}

fn param(p: &mut Parser) {
    let m = p.start();
    while p.at(At) {
        decorator(p);
    }
    while is_modifier(p, PARAM_MODIFIERS) {
        p.bump();
    }
    p.eat(Ellipsis);
    stmt::binding(p);
    p.eat(Question);
    if p.at(Colon) {
        annotation(p);
    }
    if p.eat(Eq) {
        expr::assignment(p);
    }
    m.complete(p, Param);
}

/// `@expr`, donde la expresión es una cadena de miembros y llamadas (`@Component({...})`).
pub(super) fn decorator(p: &mut Parser) {
    let m = p.start();
    p.bump();
    primary::lhs(p);
    m.complete(p, Decorator);
}
