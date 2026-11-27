//! Primarios (atributos, llamadas, índices) y átomos: nombres, literales, paréntesis, listas,
//! diccionarios, conjuntos y comprensiones.

use super::expr::{self, bitwise, disjunction, named_expr, targets};
use super::{starts_expr, Completed, Parser};
use crate::syntax::SyntaxKind::{self, *};

pub(super) fn primary(p: &mut Parser) -> Option<Completed> {
    let mut lhs = atom(p)?;
    loop {
        lhs = match p.current() {
            Dot => {
                let m = lhs.precede(p);
                p.bump();
                if p.at(Ident) || p.current().is_keyword() {
                    p.bump();
                } else {
                    p.error("se esperaba un nombre de atributo");
                }
                m.complete(p, AttributeExpr)
            }
            LParen => {
                let m = lhs.precede(p);
                arg_list(p);
                m.complete(p, CallExpr)
            }
            LBracket => {
                let m = lhs.precede(p);
                subscript(p);
                m.complete(p, SubscriptExpr)
            }
            _ => return Some(lhs),
        };
    }
}

fn atom(p: &mut Parser) -> Option<Completed> {
    let m = p.start();
    let kind = match p.current() {
        Ident => {
            p.bump();
            NameExpr
        }
        Int | Float | Imaginary | KwNone | KwTrue | KwFalse | Ellipsis => {
            p.bump();
            Literal
        }
        String | FString => {
            while matches!(p.current(), String | FString) {
                p.bump();
            }
            Literal
        }
        LParen => return Some(parenthesized(p, m)),
        LBracket => return Some(list(p, m)),
        LBrace => return Some(dict_or_set(p, m)),
        _ => {
            m.abandon(p);
            missing_expression(p);
            return None;
        }
    };
    Some(m.complete(p, kind))
}

/// Error de expresión ausente. El token se consume salvo que pueda cerrar o continuar la construcción
/// que la rodea, para que esta se recupere.
fn missing_expression(p: &mut Parser) {
    let recover = matches!(
        p.current(),
        LogicalNewline
            | Semicolon
            | Colon
            | Comma
            | Eq
            | RParen
            | RBracket
            | RBrace
            | Indent
            | Dedent
            | EndOfFile
    );
    if recover {
        p.error("se esperaba una expresión");
    } else {
        p.error_and_bump("se esperaba una expresión");
    }
}

/// Cierra el grupo abierto en `open`: lo que quede antes del cierre se marca como error.
fn close_group(p: &mut Parser, open: usize, close: SyntaxKind) {
    if let Some(end) = p.close_of(open) {
        p.skip_to(end, "contenido inesperado antes del cierre");
    }
    p.expect(close, "falta el cierre del grupo");
}

fn parenthesized(p: &mut Parser, m: super::Marker) -> Completed {
    let open = p.pos();
    p.bump();
    let kind = if p.at(RParen) {
        TupleExpr
    } else if p.at(KwYield) {
        expr::yield_expr(p);
        ParenExpr
    } else {
        element(p);
        if at_comprehension(p) {
            comprehension(p);
            GeneratorExp
        } else if p.at(Comma) {
            elements(p, RParen);
            TupleExpr
        } else {
            ParenExpr
        }
    };
    close_group(p, open, RParen);
    m.complete(p, kind)
}

fn list(p: &mut Parser, m: super::Marker) -> Completed {
    let open = p.pos();
    p.bump();
    let mut kind = ListExpr;
    if !p.at(RBracket) {
        element(p);
        if at_comprehension(p) {
            comprehension(p);
            kind = ListComp;
        } else {
            elements(p, RBracket);
        }
    }
    close_group(p, open, RBracket);
    m.complete(p, kind)
}

fn dict_or_set(p: &mut Parser, m: super::Marker) -> Completed {
    let open = p.pos();
    p.bump();
    let mut kind = DictExpr;
    if !p.at(RBrace) {
        let dict = if p.at(DoubleStar) {
            double_starred(p);
            true
        } else {
            entry_or_element(p)
        };
        kind = if at_comprehension(p) {
            comprehension(p);
            if dict {
                DictComp
            } else {
                SetComp
            }
        } else if dict {
            entries(p);
            DictExpr
        } else {
            elements(p, RBrace);
            SetExpr
        };
    }
    close_group(p, open, RBrace);
    m.complete(p, kind)
}

fn double_starred(p: &mut Parser) {
    let m = p.start();
    p.bump();
    bitwise(p, 0);
    m.complete(p, DoubleStarredExpr);
}

/// Primer elemento de unas llaves: `clave: valor` (devuelve `true`) o un elemento de conjunto.
fn entry_or_element(p: &mut Parser) -> bool {
    let Some(key) = element(p) else {
        return false;
    };
    if !p.at(Colon) {
        return false;
    }
    let m = key.precede(p);
    p.bump();
    expr::expression(p);
    m.complete(p, DictEntry);
    true
}

/// Resto de entradas de un diccionario, `, k: v` o `, **otro`; admite coma final.
fn entries(p: &mut Parser) {
    while p.eat(Comma) {
        if p.at(RBrace) {
            break;
        }
        if p.at(DoubleStar) {
            double_starred(p);
        } else if !entry_or_element(p) {
            p.error("se esperaba `clave: valor`");
        }
    }
}

/// Un elemento de tupla, lista o conjunto: `*x` o una expresión con nombre.
fn element(p: &mut Parser) -> Option<Completed> {
    if p.at(Star) {
        let m = p.start();
        p.bump();
        bitwise(p, 0);
        return Some(m.complete(p, StarredExpr));
    }
    if p.at(Ident) && p.nth(1) == ColonEq {
        named_expr(p);
        return None;
    }
    let value = expr::expression(p)?;
    if !(p.in_pattern && p.at(KwAs)) {
        return Some(value);
    }
    let m = value.precede(p);
    p.bump();
    p.expect(Ident, "se esperaba un nombre tras `as`");
    Some(m.complete(p, AsPattern))
}

/// `, elemento` repetido hasta `close`; admite coma final.
fn elements(p: &mut Parser, close: SyntaxKind) {
    while p.eat(Comma) {
        if p.at(close) || !starts_expr(p.current()) {
            break;
        }
        element(p);
    }
}

fn at_comprehension(p: &mut Parser) -> bool {
    p.at(KwFor) || (p.at(KwAsync) && p.nth(1) == KwFor)
}

/// `for destinos in iterable` seguido de más `for` o de `if condición`.
fn comprehension(p: &mut Parser) {
    while at_comprehension(p) {
        let m = p.start();
        p.eat(KwAsync);
        p.bump();
        targets(p);
        p.expect(KwIn, "se esperaba `in`");
        disjunction(p);
        m.complete(p, CompFor);
        while p.at(KwIf) {
            let m = p.start();
            p.bump();
            disjunction(p);
            m.complete(p, CompIf);
        }
    }
}

/// `(a, b=1, *c, **d)` o `(x for x in xs)`.
pub(in crate::parser::python) fn arg_list(p: &mut Parser) {
    let m = p.start();
    let open = p.pos();
    p.bump();
    while !matches!(p.current(), RParen | EndOfFile) {
        argument(p);
        if !p.eat(Comma) {
            break;
        }
    }
    close_group(p, open, RParen);
    m.complete(p, ArgList);
}

fn argument(p: &mut Parser) {
    match (p.current(), p.nth(1)) {
        (Star, _) | (DoubleStar, _) => {
            let kind = if p.at(Star) {
                StarredExpr
            } else {
                DoubleStarredExpr
            };
            let m = p.start();
            p.bump();
            expr::expression(p);
            m.complete(p, kind);
        }
        (Ident, Eq) => {
            let m = p.start();
            p.bump();
            p.bump();
            expr::expression(p);
            m.complete(p, Arg);
        }
        _ => {
            let value = p.start();
            named_expr(p);
            if at_comprehension(p) {
                comprehension(p);
                value.complete(p, GeneratorExp);
            } else {
                value.abandon(p);
            }
        }
    }
}

/// `[i]`, `[a:b:c]`, `[a, b:c]`.
fn subscript(p: &mut Parser) {
    let open = p.pos();
    p.bump();
    loop {
        slice(p);
        if !p.eat(Comma) || p.at(RBracket) {
            break;
        }
    }
    close_group(p, open, RBracket);
}

fn slice(p: &mut Parser) {
    let m = p.start();
    if !p.at(Colon) {
        element(p);
    }
    if !p.at(Colon) {
        m.abandon(p);
        return;
    }
    p.bump();
    if starts_expr(p.current()) {
        expr::expression(p);
    }
    if p.eat(Colon) && starts_expr(p.current()) {
        expr::expression(p);
    }
    m.complete(p, Slice);
}
