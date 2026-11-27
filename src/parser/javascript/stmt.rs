//! Sentencias: bloques, declaraciones de variables, control de flujo y sentencias de expresión.

use super::{allow_in, annotation, block, class, decl, expr, is_name, module, primary, semi};
use super::{statement_kind, Parser};
use crate::syntax::SyntaxKind::{self, *};

pub(super) fn statement(p: &mut Parser) {
    let before = p.pos();
    dispatch(p);
    if p.pos() == before {
        p.error_and_bump("no se esperaba este token al inicio de una sentencia");
    }
}

fn dispatch(p: &mut Parser) {
    match p.current() {
        LBrace => block(p),
        Semicolon => single(p, EmptyStmt),
        KwConst if p.nth(1) == KwEnum => module::enum_decl(p),
        KwVar | KwConst => var_stmt(p),
        KwFunction => {
            decl::function(p, false);
        }
        KwClass => {
            let m = p.start();
            class::class(p, m);
        }
        At => decorated(p),
        KwIf => if_stmt(p),
        KwFor => for_stmt(p),
        KwWhile | KwWith => conditional_body(p),
        KwDo => do_while(p),
        KwSwitch => switch(p),
        KwTry => try_stmt(p),
        KwReturn | KwThrow => return_or_throw(p),
        KwBreak | KwContinue => jump(p),
        KwDebugger => {
            let m = p.start();
            p.bump();
            semi(p);
            m.complete(p, DebuggerStmt);
        }
        KwImport if !matches!(p.nth(1), LParen | Dot) => module::import(p),
        KwExport => {
            let m = p.start();
            module::export(p, m);
        }
        KwEnum => module::enum_decl(p),
        Ident => word_statement(p),
        _ => expression_statement(p),
    }
}

/// Decoradores antes de `class` o de `export class` (estos quedan en la exportación).
fn decorated(p: &mut Parser) {
    let m = p.start();
    while p.at(At) {
        decl::decorator(p);
    }
    if p.at(KwExport) {
        module::export(p, m);
    } else {
        class::class(p, m);
    }
}

fn single(p: &mut Parser, kind: SyntaxKind) {
    let m = p.start();
    p.bump();
    m.complete(p, kind);
}

/// Sentencias que empiezan por un identificador: `let`, `async function`, etiquetas y, en TypeScript,
/// las declaraciones de tipos.
fn word_statement(p: &mut Parser) {
    if starts_let(p) {
        var_stmt(p);
    } else if p.at_word("async") && p.nth(1) == KwFunction && !p.newline_before_nth(1) {
        decl::function(p, false);
    } else if p.nth(1) == Colon {
        let m = p.start();
        p.bump();
        p.bump();
        statement(p);
        m.complete(p, LabeledStmt);
    } else if !(p.typescript && module::ts_declaration(p)) {
        expression_statement(p);
    }
}

fn starts_let(p: &mut Parser) -> bool {
    p.at_word("let") && (matches!(p.nth(1), LBracket | LBrace) || is_name(p.nth(1)))
        && !matches!(p.nth(1), KwIn | KwInstanceof)
}

fn expression_statement(p: &mut Parser) {
    let m = p.start();
    expr::expression(p);
    semi(p);
    m.complete(p, ExprStmt);
}

fn var_stmt(p: &mut Parser) {
    let m = p.start();
    declarations(p);
    semi(p);
    m.complete(p, VarDecl);
}

/// `var|let|const a = 1, [b, c] = d`: la palabra y sus declaradores, sin el `;`.
pub(super) fn declarations(p: &mut Parser) {
    p.bump();
    loop {
        let m = p.start();
        binding(p);
        if p.typescript {
            p.eat(Bang);
        }
        if p.at(Colon) {
            annotation(p);
        }
        if p.eat(Eq) {
            expr::assignment(p);
        }
        m.complete(p, VarDeclarator);
        if !p.eat(Comma) {
            break;
        }
    }
}

/// Nombre o patrón de desestructuración (`{a, b: c}`, `[x, ...y]`), parseado como su literal.
pub(super) fn binding(p: &mut Parser) {
    match p.current() {
        LBrace | LBracket => allow_in(p, |p| {
            primary::primary(p);
        }),
        kind if is_name(kind) => single(p, NameExpr),
        _ => p.error("se esperaba un nombre o un patrón"),
    }
}

/// `( expresión )` de `if`, `while`, `switch` o `with`.
fn condition(p: &mut Parser) {
    let open = p.pos();
    if p.expect(LParen, "se esperaba `(`") {
        allow_in(p, expr::expression);
        p.close_group(open, RParen);
    }
}

fn if_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump();
    condition(p);
    statement(p);
    if p.at(KwElse) {
        let clause = p.start();
        p.bump();
        statement(p);
        clause.complete(p, ElseClause);
    }
    m.complete(p, IfStmt);
}

fn conditional_body(p: &mut Parser) {
    let m = p.start();
    let kind = statement_kind(p.current());
    p.bump();
    condition(p);
    statement(p);
    m.complete(p, kind);
}

fn do_while(p: &mut Parser) {
    let m = p.start();
    p.bump();
    statement(p);
    p.expect(KwWhile, "se esperaba `while`");
    condition(p);
    p.eat(Semicolon);
    m.complete(p, DoWhileStmt);
}

/// `for (init; test; update)`, `for (x in o)`, `for (const x of xs)` y `for await (...)`.
fn for_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump();
    p.eat(KwAwait);
    let open = p.pos();
    if p.expect(LParen, "se esperaba `(`") {
        for_head(p);
        p.close_group(open, RParen);
    }
    statement(p);
    m.complete(p, ForStmt);
}

fn for_head(p: &mut Parser) {
    let saved = std::mem::replace(&mut p.no_in, true);
    if matches!(p.current(), KwVar | KwConst) || starts_let(p) {
        let m = p.start();
        declarations(p);
        m.complete(p, VarDecl);
    } else if !p.at(Semicolon) {
        expr::expression(p);
    }
    p.no_in = saved;
    if p.at(KwIn) || p.at_word("of") {
        p.bump();
        allow_in(p, expr::expression);
        return;
    }
    p.expect(Semicolon, "se esperaba `;` en el `for`");
    if !p.at(Semicolon) {
        allow_in(p, expr::expression);
    }
    p.expect(Semicolon, "se esperaba `;` en el `for`");
    if !p.at(RParen) {
        allow_in(p, expr::expression);
    }
}

fn switch(p: &mut Parser) {
    let m = p.start();
    p.bump();
    condition(p);
    let open = p.pos();
    if p.expect(LBrace, "se esperaba `{`") {
        while matches!(p.current(), KwCase | KwDefault) {
            switch_case(p);
        }
        p.close_group(open, RBrace);
    }
    m.complete(p, SwitchStmt);
}

fn switch_case(p: &mut Parser) {
    let m = p.start();
    if p.eat(KwCase) {
        allow_in(p, expr::expression);
    } else {
        p.bump();
    }
    p.expect(Colon, "se esperaba `:`");
    while !matches!(p.current(), KwCase | KwDefault | RBrace | EndOfFile) {
        statement(p);
    }
    m.complete(p, SwitchCase);
}

fn try_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump();
    block(p);
    if p.at(KwCatch) {
        let clause = p.start();
        p.bump();
        let open = p.pos();
        if p.eat(LParen) {
            binding(p);
            if p.at(Colon) {
                annotation(p);
            }
            p.close_group(open, RParen);
        }
        block(p);
        clause.complete(p, CatchClause);
    }
    if p.at(KwFinally) {
        let clause = p.start();
        p.bump();
        block(p);
        clause.complete(p, FinallyClause);
    }
    m.complete(p, TryStmt);
}

fn return_or_throw(p: &mut Parser) {
    let m = p.start();
    let kind = statement_kind(p.current());
    p.bump();
    if !matches!(p.current(), Semicolon | RBrace | EndOfFile) && !p.newline_before() {
        expr::expression(p);
    }
    semi(p);
    m.complete(p, kind);
}

fn jump(p: &mut Parser) {
    let m = p.start();
    let kind = statement_kind(p.current());
    p.bump();
    if p.at(Ident) && !p.newline_before() {
        p.bump();
    }
    semi(p);
    m.complete(p, kind);
}
