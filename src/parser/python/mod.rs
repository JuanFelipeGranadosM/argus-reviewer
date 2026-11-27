//! Gramática de Python 3 (hasta 3.14) sobre los tokens del lexer, que ya resuelve `INDENT`/`DEDENT` y
//! las líneas lógicas. Las palabras clave contextuales (`match`, `case`, `type`) son identificadores
//! y se reconocen por su posición.

mod atom;
mod compound;
mod expr;
mod params;
mod simple;

use super::{Completed, Marker, Parser};
use crate::syntax::SyntaxKind::{self, *};

pub(super) fn file(p: &mut Parser) {
    while !p.at(EndOfFile) {
        statement(p);
    }
    p.bump();
}

fn statement(p: &mut Parser) {
    let before = p.pos();
    match p.current() {
        LogicalNewline | Dedent => p.bump(),
        Indent => p.error_and_bump("indentación inesperada"),
        At => compound::decorated(p),
        KwDef => compound::function_def(p, None),
        KwAsync if matches!(p.nth(1), KwDef) => compound::function_def(p, None),
        KwAsync if matches!(p.nth(1), KwFor) => compound::for_stmt(p),
        KwAsync if matches!(p.nth(1), KwWith) => compound::with_stmt(p),
        KwClass => compound::class_def(p, None),
        KwIf => compound::if_stmt(p),
        KwWhile => compound::while_stmt(p),
        KwFor => compound::for_stmt(p),
        KwTry => compound::try_stmt(p),
        KwWith => compound::with_stmt(p),
        Ident if is_match_stmt(p) => compound::match_stmt(p),
        _ => simple_line(p),
    }
    if p.pos() == before {
        p.error_and_bump("no se esperaba este token al inicio de una sentencia");
    }
}

/// Sentencias simples separadas por `;` hasta el fin de la línea lógica.
fn simple_line(p: &mut Parser) {
    simple::statement(p);
    while p.eat(Semicolon) {
        if at_line_end(p) {
            break;
        }
        simple::statement(p);
    }
    if !p.at(LogicalNewline) && !p.at(EndOfFile) {
        p.error("se esperaba el final de la línea");
        let m = p.start();
        while !matches!(p.current(), LogicalNewline | EndOfFile) {
            p.bump();
        }
        m.complete(p, Error);
    }
    p.eat(LogicalNewline);
}

fn at_line_end(p: &mut Parser) -> bool {
    matches!(p.current(), LogicalNewline | EndOfFile | Dedent)
}

/// El cuerpo de una sentencia compuesta tras su `:`: un bloque indentado o sentencias en la misma línea.
fn block(p: &mut Parser) {
    let m = p.start();
    if p.eat(LogicalNewline) {
        if p.expect(Indent, "se esperaba un bloque indentado") {
            while !matches!(p.current(), Dedent | EndOfFile) {
                statement(p);
            }
            p.eat(Dedent);
        }
    } else {
        simple_line(p);
    }
    m.complete(p, Block);
}

/// `:` y bloque, el final de toda cabecera compuesta.
fn colon_block(p: &mut Parser) {
    p.expect(Colon, "se esperaba `:`");
    block(p);
}

/// `match sujeto:` seguido de un bloque cuyo primer token es `case`: así se distingue de un nombre
/// `match` usado como variable o función.
fn is_match_stmt(p: &mut Parser) -> bool {
    if !p.at_word("match") {
        return false;
    }
    let Some(newline) = line_end(p, p.pos() + 1) else {
        return false;
    };
    let ends_with_colon = newline
        .checked_sub(1)
        .is_some_and(|c| p.kind_at(c) == Colon);
    let case = newline + 2;
    ends_with_colon
        && p.kind_at(newline + 1) == Indent
        && p.kind_at(case) == Ident
        && p.text_at(case) == "case"
}

/// Índice del `LogicalNewline` de primer nivel a partir de `from`.
fn line_end(p: &Parser, from: usize) -> Option<usize> {
    let mut j = from;
    loop {
        match p.kind_at(j) {
            LogicalNewline => return Some(j),
            EndOfFile => return None,
            LParen | LBracket | LBrace => j = p.close_of(j)? + 1,
            _ => j += 1,
        }
    }
}

/// Puede empezar una expresión.
fn starts_expr(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        Ident
            | Int
            | Float
            | Imaginary
            | String
            | FString
            | LParen
            | LBracket
            | LBrace
            | Minus
            | Plus
            | Tilde
            | Star
            | DoubleStar
            | Ellipsis
            | KwNot
            | KwLambda
            | KwAwait
            | KwNone
            | KwTrue
            | KwFalse
            | KwYield
    )
}
