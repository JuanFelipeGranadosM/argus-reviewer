//! Expresiones primarias y sus sufijos: miembros, llamadas, `?.`, `!`, plantillas etiquetadas, `new`,
//! literales, plantillas, arrays, objetos y JSX.

use super::decl::{function, signature};
use super::{allow_in, block, class, expr, is_modifier, is_name, jsx, property_name, types};
use super::{Completed, Parser};
use crate::syntax::SyntaxKind::*;

const PROPERTY_MODIFIERS: &[&str] = &["get", "set", "async"];

/// Expresión de lado izquierdo: una primaria (o `new`) con todos sus sufijos.
pub(super) fn lhs(p: &mut Parser) -> Option<Completed> {
    let mut e = if p.at(KwNew) { new_expr(p) } else { primary(p)? };
    while let Some(next) = suffix(p, e, true) {
        e = next;
    }
    Some(e)
}

/// Un sufijo aplicado a `e`; `None` si no hay más. Sin `calls` (el callee de `new`) no consume `(`.
fn suffix(p: &mut Parser, e: Completed, calls: bool) -> Option<Completed> {
    let kind = match p.current() {
        Dot => {
            let m = e.precede(p);
            p.bump();
            member_name(p);
            return Some(m.complete(p, AttributeExpr));
        }
        QuestionDot => {
            let m = e.precede(p);
            p.bump();
            match p.current() {
                LParen => arguments(p),
                LBracket => index(p),
                _ => member_name(p),
            }
            return Some(m.complete(p, AttributeExpr));
        }
        LBracket => SubscriptExpr,
        LParen if calls => CallExpr,
        TemplateNoSub | TemplateHead => TemplateExpr,
        Bang if p.typescript && !p.newline_before() => NonNullExpr,
        Lt if p.typescript && calls => return generic_call(p, e),
        _ => return None,
    };
    let m = e.precede(p);
    match kind {
        SubscriptExpr => index(p),
        CallExpr => arguments(p),
        TemplateExpr => template(p),
        _ => p.bump(),
    }
    Some(m.complete(p, kind))
}

fn member_name(p: &mut Parser) {
    if p.at(PrivateName) || is_name(p.current()) {
        p.bump();
    } else {
        p.error("se esperaba el nombre de un miembro");
    }
}

fn index(p: &mut Parser) {
    let open = p.pos();
    p.bump();
    allow_in(p, expr::expression);
    p.close_group(open, RBracket);
}

/// `f<T>(x)` o la instanciación `f<T>`: el `<` abre argumentos de tipo si estos se cierran y lo que
/// sigue no puede continuar una comparación (como en TypeScript); si no, se deshace el intento.
fn generic_call(p: &mut Parser, e: Completed) -> Option<Completed> {
    let checkpoint = p.checkpoint();
    types::type_args(p);
    let call = matches!(p.current(), LParen | TemplateNoSub | TemplateHead);
    if p.errors_since(checkpoint) || !(call || ends_instantiation(p)) {
        p.rewind(checkpoint);
        return None;
    }
    let m = e.precede(p);
    if !call {
        return Some(m.complete(p, InstantiationExpr));
    }
    if p.at(LParen) {
        arguments(p);
    } else {
        template(p);
    }
    Some(m.complete(p, CallExpr))
}

fn ends_instantiation(p: &mut Parser) -> bool {
    p.newline_before()
        || matches!(
            p.current(),
            RParen
                | RBracket
                | RBrace
                | Comma
                | Semicolon
                | Colon
                | Dot
                | QuestionDot
                | EndOfFile
                | EqEq
                | EqEqEq
                | NotEq
                | NotEqEq
                | AmpAmp
                | PipePipe
                | QuestionQuestion
        )
}

pub(super) fn arguments(p: &mut Parser) {
    let m = p.start();
    let open = p.pos();
    p.bump();
    allow_in(p, |p| {
        while !matches!(p.current(), RParen | EndOfFile) {
            spread_or_assignment(p);
            if !p.eat(Comma) {
                break;
            }
        }
    });
    p.close_group(open, RParen);
    m.complete(p, ArgList);
}

fn spread_or_assignment(p: &mut Parser) {
    if p.at(Ellipsis) {
        let m = p.start();
        p.bump();
        expr::assignment(p);
        m.complete(p, SpreadElement);
    } else {
        expr::assignment(p);
    }
}

/// `new C(args)`, `new C`, `new a.b.C<T>()` y `new.target`.
fn new_expr(p: &mut Parser) -> Completed {
    let m = p.start();
    p.bump();
    if p.eat(Dot) {
        member_name(p);
        return m.complete(p, NewExpr);
    }
    let callee = if p.at(KwNew) { Some(new_expr(p)) } else { primary(p) };
    if let Some(mut callee) = callee {
        while let Some(next) = suffix(p, callee, false) {
            callee = next;
        }
    }
    if p.typescript && p.at(Lt) {
        let checkpoint = p.checkpoint();
        types::type_args(p);
        if p.errors_since(checkpoint) {
            p.rewind(checkpoint);
        }
    }
    if p.at(LParen) {
        arguments(p);
    }
    m.complete(p, NewExpr)
}

pub(super) fn primary(p: &mut Parser) -> Option<Completed> {
    let m = p.start();
    let kind = match p.current() {
        Ident if p.at_word("async") && p.nth(1) == KwFunction && !p.newline_before_nth(1) => {
            m.abandon(p);
            return Some(function(p, true));
        }
        Ident | KwThis | KwSuper | PrivateName | KwImport | KwYield => NameExpr,
        Int | Float | String | Regex | KwTrue | KwFalse | KwNull | TemplateNoSub => Literal,
        TemplateHead => {
            template(p);
            return Some(m.complete(p, TemplateExpr));
        }
        LParen => {
            let open = p.pos();
            p.bump();
            allow_in(p, expr::expression);
            p.close_group(open, RParen);
            return Some(m.complete(p, ParenExpr));
        }
        LBracket => return Some(array(p, m)),
        LBrace => return Some(object(p, m)),
        KwFunction => {
            m.abandon(p);
            return Some(function(p, true));
        }
        KwClass | At => return Some(class::class(p, m)),
        Lt if p.jsx && jsx::starts_element(p) => return Some(jsx::element(p, m)),
        _ => {
            m.abandon(p);
            missing(p);
            return None;
        }
    };
    p.bump();
    Some(m.complete(p, kind))
}

/// Falta una expresión: se registra el error y, salvo ante un cierre o separador, se consume el token.
fn missing(p: &mut Parser) {
    if matches!(
        p.current(),
        RParen | RBracket | RBrace | Semicolon | Comma | Colon | EndOfFile
    ) {
        p.error("se esperaba una expresión");
    } else {
        p.error_and_bump("se esperaba una expresión");
    }
}

/// `` `a${x}b${y}c` ``: la cabeza, cada expresión y sus tramos hasta la cola.
pub(super) fn template(p: &mut Parser) {
    if p.eat(TemplateNoSub) {
        return;
    }
    p.bump();
    loop {
        allow_in(p, expr::expression);
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

fn array(p: &mut Parser, m: super::Marker) -> Completed {
    let open = p.pos();
    p.bump();
    allow_in(p, |p| {
        while !matches!(p.current(), RBracket | EndOfFile) {
            if p.eat(Comma) {
                continue;
            }
            spread_or_assignment(p);
            if !p.at(RBracket) && !p.expect(Comma, "se esperaba `,` entre elementos") {
                break;
            }
        }
    });
    p.close_group(open, RBracket);
    m.complete(p, ArrayExpr)
}

fn object(p: &mut Parser, m: super::Marker) -> Completed {
    let open = p.pos();
    p.bump();
    allow_in(p, |p| {
        while !matches!(p.current(), RBrace | EndOfFile) {
            property(p);
            if !p.at(RBrace) && !p.expect(Comma, "se esperaba `,` entre propiedades") {
                break;
            }
        }
    });
    p.close_group(open, RBrace);
    m.complete(p, ObjectExpr)
}

/// `a`, `a: v`, `a = v` (en patrones), `...x`, `[k]: v`, métodos y `get`/`set`/`async`.
fn property(p: &mut Parser) {
    let m = p.start();
    if p.eat(Ellipsis) {
        expr::assignment(p);
        m.complete(p, SpreadElement);
        return;
    }
    while is_modifier(p, PROPERTY_MODIFIERS) {
        p.bump();
    }
    p.eat(Star);
    property_name(p);
    match p.current() {
        LParen | Lt => {
            signature(p);
            block(p);
            m.complete(p, MethodDef);
            return;
        }
        Colon | Eq => {
            p.bump();
            expr::assignment(p);
        }
        _ => {}
    }
    m.complete(p, Property);
}
