//! Tipos objeto de TypeScript (`{ a: T; m(): U }`, también el cuerpo de las interfaces) y tipos
//! mapeados (`{ [K in keyof T]?: V }`).

use super::class::index_signature;
use super::decl::signature;
use super::{annotation, is_modifier, property_name, types, Parser};
use crate::syntax::SyntaxKind::*;

/// `{ [K in T as U]?: V }`, con `+`/`-` y `readonly` opcionales.
pub(super) fn mapped_ahead(p: &mut Parser) -> bool {
    let mut at = 1;
    if matches!(p.nth(at), Plus | Minus) {
        at += 1;
    }
    if p.nth(at) == Ident && p.text_at(p.pos() + at) == "readonly" {
        at += 1;
    }
    p.nth(at) == LBracket && p.nth(at + 1) == Ident && p.nth(at + 2) == KwIn
}

pub(super) fn mapped(p: &mut Parser) {
    let open = p.pos();
    p.bump();
    if matches!(p.current(), Plus | Minus) {
        p.bump();
    }
    if p.at_word("readonly") {
        p.bump();
    }
    let bracket = p.pos();
    p.bump();
    p.bump();
    p.bump();
    types::type_(p);
    if p.at_word("as") {
        p.bump();
        types::type_(p);
    }
    p.close_group(bracket, RBracket);
    if matches!(p.current(), Plus | Minus) {
        p.bump();
    }
    p.eat(Question);
    if p.at(Colon) {
        annotation(p);
    }
    if !p.eat(Semicolon) {
        p.eat(Comma);
    }
    p.close_group(open, RBrace);
}

/// `{ a: T; b?(): U; readonly [k: string]: V; new (): W; (x): Y }` de tipos e interfaces.
pub(super) fn object_type(p: &mut Parser) {
    let m = p.start();
    let open = p.pos();
    if p.expect(LBrace, "se esperaba `{`") {
        while !matches!(p.current(), RBrace | EndOfFile) {
            let before = p.pos();
            type_member(p);
            if p.pos() == before {
                p.error_and_bump("no se esperaba este token en el tipo");
            }
        }
        p.close_group(open, RBrace);
    }
    m.complete(p, TypeExpr);
}

fn type_member(p: &mut Parser) {
    let m = p.start();
    let construct = p.at(KwNew) && matches!(p.nth(1), LParen | Lt);
    if construct {
        p.bump();
    }
    let kind = if construct || matches!(p.current(), LParen | Lt) {
        signature(p);
        MethodDef
    } else {
        while is_modifier(p, &["readonly", "get", "set"]) {
            p.bump();
        }
        if index_signature(p) {
            PropertyDef
        } else {
            property_name(p);
            p.eat(Question);
            if matches!(p.current(), LParen | Lt) {
                signature(p);
                MethodDef
            } else {
                if p.at(Colon) {
                    annotation(p);
                }
                PropertyDef
            }
        }
    };
    if !p.eat(Semicolon) {
        p.eat(Comma);
    }
    m.complete(p, kind);
}
