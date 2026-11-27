//! Parámetros de `def` y `lambda` y parámetros de tipo (PEP 695).

use super::{expr, Parser};
use crate::syntax::SyntaxKind::*;

/// `(a, b: int = 1, /, *args, c, **kw)`.
pub(super) fn param_list(p: &mut Parser) {
    let m = p.start();
    let open = p.pos();
    p.bump();
    while !matches!(p.current(), RParen | EndOfFile) {
        param(p, true);
        if !p.eat(Comma) {
            break;
        }
    }
    if let Some(close) = p.close_of(open) {
        p.skip_to(close, "parámetro no reconocido");
    }
    p.expect(RParen, "se esperaba `)`");
    m.complete(p, ParamList);
}

/// Parámetros de una `lambda`: sin paréntesis ni anotaciones, hasta el `:`.
pub(super) fn lambda_params(p: &mut Parser) {
    let m = p.start();
    while !matches!(p.current(), Colon | LogicalNewline | EndOfFile) {
        param(p, false);
        if !p.eat(Comma) {
            break;
        }
    }
    m.complete(p, ParamList);
}

fn param(p: &mut Parser, annotated: bool) {
    let m = p.start();
    match p.current() {
        Slash => p.bump(),
        Star => {
            p.bump();
            p.eat(Ident);
        }
        DoubleStar => {
            p.bump();
            p.expect(Ident, "se esperaba un nombre tras `**`");
        }
        _ => {
            p.expect(Ident, "se esperaba el nombre del parámetro");
        }
    }
    if annotated && p.at(Colon) {
        p.bump();
        let annotation = p.start();
        if p.at(Star) {
            let starred = p.start();
            p.bump();
            expr::expression(p);
            starred.complete(p, StarredExpr);
        } else {
            expr::expression(p);
        }
        annotation.complete(p, Annotation);
    }
    if p.eat(Eq) {
        expr::expression(p);
    }
    m.complete(p, Param);
}

/// `[T, U: int, *Ts, **P = ...]`.
pub(super) fn type_params(p: &mut Parser) {
    let m = p.start();
    p.bump();
    while !matches!(p.current(), RBracket | EndOfFile) {
        let param = p.start();
        if matches!(p.current(), Star | DoubleStar) {
            p.bump();
        }
        p.expect(Ident, "se esperaba un parámetro de tipo");
        if p.eat(Colon) {
            expr::expression(p);
        }
        if p.eat(Eq) {
            expr::expression(p);
        }
        param.complete(p, TypeParam);
        if !p.eat(Comma) {
            break;
        }
    }
    p.expect(RBracket, "se esperaba `]`");
    m.complete(p, TypeParamList);
}
