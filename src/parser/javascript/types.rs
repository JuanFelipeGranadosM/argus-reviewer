//! Tipos de TypeScript: condicionales, funciones, uniones e intersecciones, operadores (`keyof`,
//! `infer`, ...), arrays e índices, referencias genéricas, tuplas, tipos objeto y mapeados, literales
//! y plantillas. Cada `type_` es un nodo `TypeExpr`; su estructura interna queda en tokens.

use super::decl::params;
pub(super) use super::object_type::object_type;
use super::object_type::{mapped, mapped_ahead};
use super::{is_name, name, Parser};
use crate::syntax::SyntaxKind::*;

pub(super) fn type_(p: &mut Parser) {
    let m = p.start();
    non_conditional(p);
    if p.at(KwExtends) {
        p.bump();
        non_conditional(p);
        p.expect(Question, "se esperaba `?` en el tipo condicional");
        type_(p);
        p.expect(Colon, "se esperaba `:` en el tipo condicional");
        type_(p);
    }
    m.complete(p, TypeExpr);
}

/// Tipo de retorno, que admite predicados: `x is T`, `asserts x`, `asserts x is T`, `this is T`.
pub(super) fn return_type(p: &mut Parser) {
    if p.at_word("asserts") && matches!(p.nth(1), Ident | KwThis) && !p.newline_before_nth(1) {
        p.bump();
    }
    if matches!(p.current(), Ident | KwThis) && p.nth(1) == Ident && p.nth_text(1) == "is" {
        p.bump();
        p.bump();
    }
    type_(p);
}

fn non_conditional(p: &mut Parser) {
    if function_type_ahead(p) {
        function_type(p);
    } else {
        union(p);
    }
}

fn function_type_ahead(p: &mut Parser) -> bool {
    match p.current() {
        Lt | KwNew => true,
        LParen => p
            .close_of(p.pos())
            .is_some_and(|c| p.kind_at(c + 1) == FatArrow),
        _ => p.at_word("abstract") && p.nth(1) == KwNew,
    }
}

/// `(a: T) => R`, `<T>(a: T) => R`, `new (...) => R` y `abstract new (...) => R`.
fn function_type(p: &mut Parser) {
    if p.at_word("abstract") {
        p.bump();
    }
    p.eat(KwNew);
    if p.at(Lt) {
        type_params(p);
    }
    if p.at(LParen) {
        params(p);
    } else {
        p.error("se esperaba `(`");
    }
    p.expect(FatArrow, "se esperaba `=>`");
    return_type(p);
}

fn union(p: &mut Parser) {
    p.eat(Pipe);
    intersection(p);
    while p.eat(Pipe) {
        intersection(p);
    }
}

fn intersection(p: &mut Parser) {
    p.eat(Amp);
    operator_type(p);
    while p.eat(Amp) {
        operator_type(p);
    }
}

fn operator_type(p: &mut Parser) {
    let word = p.nth_text(0);
    if p.at(Ident) && matches!(word, "keyof" | "unique" | "readonly") && starts_type(p.nth(1)) {
        p.bump();
        operator_type(p);
    } else if p.at_word("infer") && p.nth(1) == Ident {
        p.bump();
        p.bump();
        if p.at(KwExtends) {
            p.bump();
            non_conditional(p);
        }
    } else {
        postfix_type(p);
    }
}

fn starts_type(kind: crate::syntax::SyntaxKind) -> bool {
    is_name(kind)
        || matches!(
            kind,
            LParen
                | LBracket
                | LBrace
                | String
                | Int
                | Float
                | Minus
                | TemplateNoSub
                | TemplateHead
        )
}

/// `T[]` y `T[K]`, sin salto de línea antes del `[`.
fn postfix_type(p: &mut Parser) {
    primary_type(p);
    while p.at(LBracket) && !p.newline_before() {
        let open = p.pos();
        p.bump();
        if !p.at(RBracket) {
            type_(p);
        }
        p.close_group(open, RBracket);
    }
}

fn primary_type(p: &mut Parser) {
    match p.current() {
        LParen => {
            let open = p.pos();
            p.bump();
            type_(p);
            p.close_group(open, RParen);
        }
        LBracket => tuple(p),
        LBrace if mapped_ahead(p) => mapped(p),
        LBrace => object_type(p),
        KwTypeof => {
            p.bump();
            if p.at(KwImport) {
                import_type(p);
            } else {
                reference(p);
            }
        }
        KwImport => import_type(p),
        String | Int | Float | TemplateNoSub => p.bump(),
        Minus => {
            p.bump();
            if !p.eat(Int) {
                p.expect(Float, "se esperaba un número");
            }
        }
        TemplateHead => template_type(p),
        kind if is_name(kind) => reference(p),
        RParen | RBracket | RBrace | Comma | Semicolon | Gt | Eq | EndOfFile => {
            p.error("se esperaba un tipo");
        }
        _ => p.error_and_bump("se esperaba un tipo"),
    }
}

/// `a.b.C<T>`.
fn reference(p: &mut Parser) {
    name(p);
    while p.at(Dot) {
        p.bump();
        name(p);
    }
    if p.at(Lt) && !p.newline_before() {
        type_args(p);
    }
}

/// `import("mod").Tipo<T>`.
fn import_type(p: &mut Parser) {
    p.bump();
    let open = p.pos();
    if p.expect(LParen, "se esperaba `(`") {
        type_(p);
        p.close_group(open, RParen);
    }
    while p.eat(Dot) {
        name(p);
    }
    if p.at(Lt) {
        type_args(p);
    }
}

fn template_type(p: &mut Parser) {
    p.bump();
    loop {
        type_(p);
        match p.current() {
            TemplateMiddle => p.bump(),
            TemplateTail => {
                p.bump();
                break;
            }
            _ => {
                p.error("plantilla sin cerrar");
                break;
            }
        }
    }
}

/// `[A, B?, ...C[]]` y con nombres: `[x: A, y?: B]`.
fn tuple(p: &mut Parser) {
    let open = p.pos();
    p.bump();
    while !matches!(p.current(), RBracket | EndOfFile) {
        p.eat(Ellipsis);
        let named =
            is_name(p.nth(0)) && (p.nth(1) == Colon || (p.nth(1) == Question && p.nth(2) == Colon));
        if named {
            p.bump();
            p.eat(Question);
            p.bump();
        }
        type_(p);
        p.eat(Question);
        if !p.eat(Comma) {
            break;
        }
    }
    p.close_group(open, RBracket);
}

/// `<A, B>` de una referencia genérica o una llamada.
pub(super) fn type_args(p: &mut Parser) {
    let m = p.start();
    p.bump();
    while !matches!(p.current(), Gt | EndOfFile) {
        type_(p);
        if !p.eat(Comma) {
            break;
        }
    }
    p.expect(Gt, "se esperaba `>`");
    m.complete(p, TypeArgs);
}

/// `<const T extends U = D, in out V>` de una declaración genérica.
pub(super) fn type_params(p: &mut Parser) {
    let m = p.start();
    p.bump();
    while !matches!(p.current(), Gt | EndOfFile) {
        let param = p.start();
        while matches!(p.current(), KwConst | KwIn) || p.at_word("out") {
            if !is_name(p.nth(1)) {
                break;
            }
            p.bump();
        }
        p.expect(Ident, "se esperaba el nombre del parámetro de tipo");
        if p.eat(KwExtends) {
            type_(p);
        }
        if p.eat(Eq) {
            type_(p);
        }
        param.complete(p, TypeParam);
        if !p.eat(Comma) {
            break;
        }
    }
    p.expect(Gt, "se esperaba `>`");
    m.complete(p, TypeParamList);
}

/// Lista de tipos separados por comas (`implements A, B`, `extends A, B`).
pub(super) fn type_list(p: &mut Parser) {
    loop {
        type_(p);
        if !p.eat(Comma) {
            break;
        }
    }
}
