//! Clases: decoradores, herencia, `implements` y miembros con sus modificadores.

use super::decl::{decorator, signature};
use super::{
    allow_in, annotation, block, expr, is_modifier, is_name, primary, property_name, semi,
};
use super::{types, Completed, Marker, Parser};
use crate::syntax::SyntaxKind::*;

const MEMBER_MODIFIERS: &[&str] = &[
    "static",
    "public",
    "private",
    "protected",
    "readonly",
    "abstract",
    "override",
    "declare",
    "accessor",
    "async",
    "get",
    "set",
];

/// `[@dec] [abstract] class Nombre<T> extends Base<T> implements I { ... }`, sentencia o expresión.
pub(super) fn class(p: &mut Parser, m: Marker) -> Completed {
    while p.at(At) {
        decorator(p);
    }
    if p.at_word("abstract") {
        p.bump();
    }
    p.expect(KwClass, "se esperaba `class`");
    if p.at(Ident) && !p.at_word("implements") {
        p.bump();
    }
    if p.at(Lt) {
        types::type_params(p);
    }
    if p.eat(KwExtends) {
        primary::lhs(p);
        if p.at(Lt) {
            types::type_args(p);
        }
    }
    if p.at_word("implements") {
        p.bump();
        types::type_list(p);
    }
    body(p);
    m.complete(p, ClassDef)
}

fn body(p: &mut Parser) {
    let m = p.start();
    let open = p.pos();
    if p.expect(LBrace, "se esperaba `{`") {
        allow_in(p, |p| {
            while !matches!(p.current(), RBrace | EndOfFile) {
                let before = p.pos();
                member(p);
                if p.pos() == before {
                    p.error_and_bump("no se esperaba este token en el cuerpo de la clase");
                }
            }
        });
        p.close_group(open, RBrace);
    }
    m.complete(p, ClassBody);
}

fn member(p: &mut Parser) {
    if p.eat(Semicolon) {
        return;
    }
    let m = p.start();
    while p.at(At) {
        decorator(p);
    }
    while is_modifier(p, MEMBER_MODIFIERS) {
        p.bump();
    }
    p.eat(Star);
    if p.at(LBrace) {
        block(p);
        m.complete(p, MethodDef);
        return;
    }
    if index_signature(p) {
        semi(p);
        m.complete(p, PropertyDef);
        return;
    }
    property_name(p);
    if p.at(Question) || (p.at(Bang) && p.typescript) {
        p.bump();
    }
    if matches!(p.current(), LParen | Lt) {
        signature(p);
        if p.at(LBrace) {
            block(p);
        } else {
            semi(p);
        }
        m.complete(p, MethodDef);
        return;
    }
    if p.at(Colon) {
        annotation(p);
    }
    if p.eat(Eq) {
        expr::assignment(p);
    }
    semi(p);
    m.complete(p, PropertyDef);
}

/// `[clave: string]: T` de TypeScript, en clases y en tipos objeto.
pub(super) fn index_signature(p: &mut Parser) -> bool {
    if !(p.at(LBracket) && is_name(p.nth(1)) && p.nth(2) == Colon) {
        return false;
    }
    let open = p.pos();
    p.bump();
    p.bump();
    annotation(p);
    p.close_group(open, RBracket);
    if p.at(Colon) {
        annotation(p);
    }
    true
}
