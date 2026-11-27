//! Sentencias simples: expresiones, asignaciones, `return`, `import`, `global`, ...

use super::{at_line_end, expr, params, Parser};
use crate::syntax::SyntaxKind::{self, *};

pub(super) fn statement(p: &mut Parser) {
    match p.current() {
        KwPass => keyword(p, PassStmt),
        KwBreak => keyword(p, BreakStmt),
        KwContinue => keyword(p, ContinueStmt),
        KwReturn => {
            let m = p.start();
            p.bump();
            if !at_stmt_end(p) {
                expr::star_expressions(p);
            }
            m.complete(p, ReturnStmt);
        }
        KwRaise => {
            let m = p.start();
            p.bump();
            if !at_stmt_end(p) {
                expr::expression(p);
                if p.eat(KwFrom) {
                    expr::expression(p);
                }
            }
            m.complete(p, RaiseStmt);
        }
        KwGlobal => names(p, GlobalStmt),
        KwNonlocal => names(p, NonlocalStmt),
        KwDel => {
            let m = p.start();
            p.bump();
            expr::star_expressions(p);
            m.complete(p, DelStmt);
        }
        KwAssert => {
            let m = p.start();
            p.bump();
            expr::expression(p);
            if p.eat(Comma) {
                expr::expression(p);
            }
            m.complete(p, AssertStmt);
        }
        KwImport => import(p),
        KwFrom => import_from(p),
        Ident if is_type_alias(p) => type_alias(p),
        _ => expression_statement(p),
    }
}

fn at_stmt_end(p: &mut Parser) -> bool {
    at_line_end(p) || p.at(Semicolon)
}

fn keyword(p: &mut Parser, kind: SyntaxKind) {
    let m = p.start();
    p.bump();
    m.complete(p, kind);
}

/// `global a, b` y `nonlocal a, b`.
fn names(p: &mut Parser, kind: SyntaxKind) {
    let m = p.start();
    p.bump();
    loop {
        p.expect(Ident, "se esperaba un nombre");
        if !p.eat(Comma) {
            break;
        }
    }
    m.complete(p, kind);
}

fn import(p: &mut Parser) {
    let m = p.start();
    p.bump();
    loop {
        let alias = p.start();
        dotted_name(p);
        if p.eat(KwAs) {
            p.expect(Ident, "se esperaba un nombre tras `as`");
        }
        alias.complete(p, ImportAlias);
        if !p.eat(Comma) {
            break;
        }
    }
    m.complete(p, ImportStmt);
}

/// `from .paquete import a as b, c`, `from . import (a, b)` o `from m import *`.
fn import_from(p: &mut Parser) {
    let m = p.start();
    p.bump();
    while matches!(p.current(), Dot | Ellipsis) {
        p.bump();
    }
    if p.at(Ident) {
        dotted_name(p);
    }
    p.expect(KwImport, "se esperaba `import`");
    if !p.eat(Star) {
        let parens = p.eat(LParen);
        while p.at(Ident) {
            let alias = p.start();
            p.bump();
            if p.eat(KwAs) {
                p.expect(Ident, "se esperaba un nombre tras `as`");
            }
            alias.complete(p, ImportAlias);
            if !p.eat(Comma) {
                break;
            }
        }
        if parens {
            p.expect(RParen, "se esperaba `)`");
        }
    }
    m.complete(p, ImportFromStmt);
}

fn dotted_name(p: &mut Parser) {
    let m = p.start();
    p.expect(Ident, "se esperaba un nombre de módulo");
    while p.at(Dot) && p.nth(1) == Ident {
        p.bump();
        p.bump();
    }
    m.complete(p, DottedName);
}

/// `type Alias[T] = ...` (PEP 695): `type` es una palabra clave contextual.
fn is_type_alias(p: &mut Parser) -> bool {
    p.at_word("type") && p.nth(1) == Ident && matches!(p.nth(2), Eq | LBracket)
}

fn type_alias(p: &mut Parser) {
    let m = p.start();
    p.bump();
    p.bump();
    if p.at(LBracket) {
        params::type_params(p);
    }
    p.expect(Eq, "se esperaba `=`");
    expr::expression(p);
    m.complete(p, TypeAliasStmt);
}

fn expression_statement(p: &mut Parser) {
    let m = p.start();
    expr::star_expressions_or_yield(p);
    let kind = match p.current() {
        Eq => {
            while p.eat(Eq) {
                expr::star_expressions_or_yield(p);
            }
            AssignStmt
        }
        Colon => {
            p.bump();
            let annotation = p.start();
            expr::expression(p);
            annotation.complete(p, Annotation);
            if p.eat(Eq) {
                expr::star_expressions_or_yield(p);
            }
            AnnAssignStmt
        }
        kind if is_augmented(kind) => {
            p.bump();
            expr::star_expressions_or_yield(p);
            AugAssignStmt
        }
        _ => ExprStmt,
    };
    m.complete(p, kind);
}

fn is_augmented(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        PlusEq
            | MinusEq
            | StarEq
            | DoubleStarEq
            | SlashEq
            | DoubleSlashEq
            | PercentEq
            | AtEq
            | AmpEq
            | PipeEq
            | CaretEq
            | LShiftEq
            | RShiftEq
    )
}
