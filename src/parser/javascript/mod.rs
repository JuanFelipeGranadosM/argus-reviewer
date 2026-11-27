//! Gramática de JavaScript y TypeScript (ES2024, TypeScript 5, JSX) sobre los tokens del lexer.
//!
//! El lexer ya decidió regex frente a división y JSX frente a comparación; aquí se deciden las
//! palabras contextuales (`let`, `async`, `of`, `type`, `interface`, ...), que llegan como `Ident`.
//! Los patrones de desestructuración se parsean como literales de objeto y de array, que los cubren.
//! Las flechas y los argumentos de tipo se reconocen mirando hacia delante o con un intento
//! especulativo que se deshace ([`Parser::rewind`]) si no encaja.

mod class;
mod decl;
mod expr;
mod jsx;
mod module;
mod object_type;
mod primary;
mod stmt;
mod types;

use super::{Completed, Marker, Parser};
use crate::syntax::SyntaxKind::{self, *};

pub(super) fn file(p: &mut Parser) {
    statements(p, false);
    p.bump();
}

/// Sentencias hasta el final o, si `in_braces`, hasta la `}` que cierra el bloque.
fn statements(p: &mut Parser, in_braces: bool) {
    loop {
        match p.current() {
            EndOfFile => break,
            RBrace if in_braces => break,
            _ => stmt::statement(p),
        }
    }
}

/// `{ sentencias }`, también el cuerpo de una función: dentro vuelve a valer `in` como operador.
fn block(p: &mut Parser) {
    let m = p.start();
    let open = p.pos();
    if p.expect(LBrace, "se esperaba `{`") {
        let saved = std::mem::replace(&mut p.no_in, false);
        statements(p, true);
        p.no_in = saved;
        p.close_group(open, RBrace);
    }
    m.complete(p, Block);
}

/// Fin de sentencia con inserción automática de `;`: vale un `;`, una `}`, el final o un salto de línea.
fn semi(p: &mut Parser) {
    if p.eat(Semicolon) || matches!(p.current(), RBrace | EndOfFile) || p.newline_before() {
        return;
    }
    p.error("se esperaba `;`");
}

/// Identificador o palabra reservada: los nombres de propiedad y de miembro admiten ambos.
fn is_name(kind: SyntaxKind) -> bool {
    kind == Ident || kind.is_keyword()
}

fn starts_property_name(kind: SyntaxKind) -> bool {
    is_name(kind) || matches!(kind, String | Int | Float | PrivateName | LBracket)
}

fn name(p: &mut Parser) {
    if is_name(p.current()) {
        p.bump();
    } else {
        p.error("se esperaba un nombre");
    }
}

/// Nombre de propiedad o de miembro: identificador, palabra reservada, literal, `#privado` o `[expr]`.
fn property_name(p: &mut Parser) {
    if p.at(LBracket) {
        let open = p.pos();
        p.bump();
        allow_in(p, |p| {
            expr::assignment(p);
        });
        p.close_group(open, RBracket);
    } else if starts_property_name(p.current()) {
        p.bump();
    } else {
        p.error("se esperaba el nombre de una propiedad");
    }
}

/// Ejecuta `f` con `in` como operador, aunque se esté en la inicialización de un `for`.
fn allow_in(p: &mut Parser, f: impl FnOnce(&mut Parser)) {
    let saved = std::mem::replace(&mut p.no_in, false);
    f(p);
    p.no_in = saved;
}

/// `: tipo` de una variable, parámetro o propiedad.
fn annotation(p: &mut Parser) {
    let m = p.start();
    p.bump();
    types::type_(p);
    m.complete(p, Annotation);
}

/// Una palabra contextual que actúa como modificador porque la sigue, en la misma línea, el nombre
/// de lo que modifica (`static x`, `readonly x`, `get x()`); `get()` o `static = 1` son nombres.
fn is_modifier(p: &mut Parser, words: &[&str]) -> bool {
    p.at(Ident)
        && words.contains(&p.nth_text(0))
        && (starts_property_name(p.nth(1)) || matches!(p.nth(1), Star | LBrace | Ellipsis))
        && !p.newline_before_nth(1)
}

/// Nodo de las sentencias que se reconocen por su palabra clave.
fn statement_kind(keyword: SyntaxKind) -> SyntaxKind {
    match keyword {
        KwWhile => WhileStmt,
        KwWith => WithStmt,
        KwReturn => ReturnStmt,
        KwThrow => ThrowStmt,
        KwBreak => BreakStmt,
        KwContinue => ContinueStmt,
        _ => Error,
    }
}
