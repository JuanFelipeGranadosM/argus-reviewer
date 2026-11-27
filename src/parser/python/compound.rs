//! Sentencias compuestas: `def`, `class`, `if`, bucles, `try`, `with` y `match`.

use super::{block, colon_block, expr, params, Marker, Parser};
use crate::syntax::SyntaxKind::*;

/// Decoradores seguidos de la `def` o `class` que decoran: quedan como hijos de esta.
pub(super) fn decorated(p: &mut Parser) {
    let m = p.start();
    while p.at(At) {
        let decorator = p.start();
        p.bump();
        expr::named_expr(p);
        p.expect(LogicalNewline, "se esperaba el final de la línea tras el decorador");
        decorator.complete(p, Decorator);
    }
    match p.current() {
        KwDef | KwAsync => function_def(p, Some(m)),
        KwClass => class_def(p, Some(m)),
        _ => {
            p.error("se esperaba `def` o `class` tras los decoradores");
            m.complete(p, Error);
        }
    }
}

pub(super) fn function_def(p: &mut Parser, decorators: Option<Marker>) {
    let m = decorators.unwrap_or_else(|| p.start());
    p.eat(KwAsync);
    p.expect(KwDef, "se esperaba `def`");
    p.expect(Ident, "se esperaba el nombre de la función");
    if p.at(LBracket) {
        params::type_params(p);
    }
    if p.at(LParen) {
        params::param_list(p);
    } else {
        p.error("se esperaba `(`");
    }
    if p.at(Arrow) {
        let returns = p.start();
        p.bump();
        expr::expression(p);
        returns.complete(p, ReturnType);
    }
    colon_block(p);
    m.complete(p, FunctionDef);
}

pub(super) fn class_def(p: &mut Parser, decorators: Option<Marker>) {
    let m = decorators.unwrap_or_else(|| p.start());
    p.bump();
    p.expect(Ident, "se esperaba el nombre de la clase");
    if p.at(LBracket) {
        params::type_params(p);
    }
    if p.at(LParen) {
        expr::arg_list(p);
    }
    colon_block(p);
    m.complete(p, ClassDef);
}

pub(super) fn if_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump();
    expr::named_expr(p);
    colon_block(p);
    while p.at(KwElif) {
        let elif = p.start();
        p.bump();
        expr::named_expr(p);
        colon_block(p);
        elif.complete(p, ElifClause);
    }
    else_clause(p);
    m.complete(p, IfStmt);
}

fn else_clause(p: &mut Parser) {
    if p.at(KwElse) {
        let m = p.start();
        p.bump();
        colon_block(p);
        m.complete(p, ElseClause);
    }
}

pub(super) fn while_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump();
    expr::named_expr(p);
    colon_block(p);
    else_clause(p);
    m.complete(p, WhileStmt);
}

pub(super) fn for_stmt(p: &mut Parser) {
    let m = p.start();
    p.eat(KwAsync);
    p.bump();
    expr::targets(p);
    p.expect(KwIn, "se esperaba `in`");
    expr::star_expressions(p);
    colon_block(p);
    else_clause(p);
    m.complete(p, ForStmt);
}

pub(super) fn try_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump();
    colon_block(p);
    while p.at(KwExcept) {
        let clause = p.start();
        p.bump();
        p.eat(Star);
        if !p.at(Colon) {
            expr::star_expressions(p);
            if p.eat(KwAs) {
                p.expect(Ident, "se esperaba un nombre tras `as`");
            }
        }
        colon_block(p);
        clause.complete(p, ExceptClause);
    }
    else_clause(p);
    if p.at(KwFinally) {
        let clause = p.start();
        p.bump();
        colon_block(p);
        clause.complete(p, FinallyClause);
    }
    m.complete(p, TryStmt);
}

pub(super) fn with_stmt(p: &mut Parser) {
    let m = p.start();
    p.eat(KwAsync);
    p.bump();
    if parenthesized_items(p) {
        p.bump();
        with_items(p);
        p.expect(RParen, "se esperaba `)`");
    } else {
        with_items(p);
    }
    colon_block(p);
    m.complete(p, WithStmt);
}

/// `with (a as b, c):` (3.9+): los paréntesis agrupan elementos, no son una expresión. Se distingue
/// de `with (a):` porque dentro hay un `as` o una coma de primer nivel y detrás viene el `:`.
fn parenthesized_items(p: &mut Parser) -> bool {
    if !p.at(LParen) {
        return false;
    }
    let open = p.pos();
    let Some(close) = p.close_of(open) else {
        return false;
    };
    if p.kind_at(close + 1) != Colon {
        return false;
    }
    let mut j = open + 1;
    while j < close {
        match p.kind_at(j) {
            KwAs | Comma => return true,
            LParen | LBracket | LBrace => j = p.close_of(j).unwrap_or(j) + 1,
            _ => j += 1,
        }
    }
    false
}

fn with_items(p: &mut Parser) {
    loop {
        let item = p.start();
        expr::expression(p);
        if p.eat(KwAs) {
            expr::target(p);
        }
        item.complete(p, WithItem);
        if !p.eat(Comma) || p.at(RParen) || p.at(Colon) {
            break;
        }
    }
}

/// `match sujeto:` con sus `case patrón [if guarda]:`. Los patrones se parsean como expresiones,
/// que los cubren (`Punto(x=0)`, `[a, *resto]`, `{"k": v, **r}`, `1 | 2`) más un `as nombre` final.
pub(super) fn match_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump();
    expr::star_expressions(p);
    p.expect(Colon, "se esperaba `:`");
    let body = p.start();
    p.expect(LogicalNewline, "se esperaba el final de la línea");
    p.expect(Indent, "se esperaba un bloque indentado");
    while p.at_word("case") {
        case_clause(p);
    }
    p.eat(Dedent);
    body.complete(p, Block);
    m.complete(p, MatchStmt);
}

fn case_clause(p: &mut Parser) {
    let m = p.start();
    p.bump();
    p.in_pattern = true;
    expr::patterns(p);
    p.in_pattern = false;
    if p.eat(KwAs) {
        p.expect(Ident, "se esperaba un nombre tras `as`");
    }
    if p.at(KwIf) {
        let guard = p.start();
        p.bump();
        expr::named_expr(p);
        guard.complete(p, Guard);
    }
    p.expect(Colon, "se esperaba `:`");
    block(p);
    m.complete(p, CaseClause);
}
