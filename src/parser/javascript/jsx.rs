//! Elementos JSX: `<a.b c="1" {...d} e={f}>hijos</a.b>`, `<x />` y fragmentos `<>…</>`. El lexer ya
//! emitió los nombres (con `-`), las cadenas sin escapes y el texto de los hijos como `JsxText`.

use super::{allow_in, expr, is_name, types, Completed, Marker, Parser};
use crate::lexer::javascript::looks_like_type_parameters;
use crate::syntax::SyntaxKind::*;

/// La misma decisión que el lexer: `<>` o `<nombre` que no parezca una lista de parámetros de tipo.
pub(super) fn starts_element(p: &mut Parser) -> bool {
    match p.nth(1) {
        Gt => true,
        Ident => p.adjacent(1) && !looks_like_type_parameters(p.source_after(0)),
        _ => false,
    }
}

pub(super) fn element(p: &mut Parser, m: Marker) -> Completed {
    p.bump();
    if p.eat(Gt) {
        children(p);
        closing(p);
        return m.complete(p, JsxElement);
    }
    tag_name(p);
    if p.typescript && p.at(Lt) {
        types::type_args(p);
    }
    attributes(p);
    if p.eat(Slash) {
        p.expect(Gt, "se esperaba `>`");
    } else if p.expect(Gt, "se esperaba `>`") {
        children(p);
        closing(p);
    }
    m.complete(p, JsxElement)
}

/// `div`, `a.b.C` o `svg:rect`.
fn tag_name(p: &mut Parser) {
    if is_name(p.current()) {
        p.bump();
    } else {
        p.error("se esperaba el nombre de la etiqueta");
    }
    while matches!(p.current(), Dot | Colon) && is_name(p.nth(1)) {
        p.bump();
        p.bump();
    }
}

fn attributes(p: &mut Parser) {
    loop {
        let m = p.start();
        match p.current() {
            LBrace => container(p),
            kind if is_name(kind) => {
                p.bump();
                if p.at(Colon) && is_name(p.nth(1)) {
                    p.bump();
                    p.bump();
                }
                if p.eat(Eq) {
                    value(p);
                }
            }
            _ => {
                m.abandon(p);
                break;
            }
        }
        m.complete(p, JsxAttribute);
    }
}

fn value(p: &mut Parser) {
    match p.current() {
        String => p.bump(),
        LBrace => container(p),
        Lt => {
            let m = p.start();
            element(p, m);
        }
        _ => p.error("se esperaba el valor del atributo"),
    }
}

/// `{expr}`, `{...props}` o `{/* comentario */}`.
fn container(p: &mut Parser) {
    let open = p.pos();
    p.bump();
    p.eat(Ellipsis);
    if !p.at(RBrace) {
        allow_in(p, expr::expression);
    }
    p.close_group(open, RBrace);
}

fn children(p: &mut Parser) {
    loop {
        match p.current() {
            JsxText => p.bump(),
            LBrace => container(p),
            Lt if p.nth(1) != Slash => {
                let m = p.start();
                element(p, m);
            }
            _ => break,
        }
    }
}

/// `</nombre>` o `</>`.
fn closing(p: &mut Parser) {
    p.expect(Lt, "falta la etiqueta de cierre");
    p.expect(Slash, "se esperaba `/`");
    if !p.at(Gt) {
        tag_name(p);
    }
    p.expect(Gt, "se esperaba `>`");
}
