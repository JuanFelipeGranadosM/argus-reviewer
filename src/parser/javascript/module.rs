//! Módulos (`import`/`export`) y declaraciones propias de TypeScript: `interface`, `type`, `enum`,
//! `namespace`/`module` y `declare`.

use super::{block, class, expr, is_name, name, primary, property_name, semi, stmt, types};
use super::{Marker, Parser};
use crate::syntax::SyntaxKind::*;

pub(super) fn import(p: &mut Parser) {
    let m = p.start();
    p.bump();
    if p.at_word("type") && type_modifier(p) {
        p.bump();
    }
    if p.at(Ident) && p.nth(1) == Eq {
        p.bump();
        p.bump();
        expr::assignment(p);
        semi(p);
        m.complete(p, ImportDecl);
        return;
    }
    if p.at(Ident) && !(p.at_word("from") && p.nth(1) == String) {
        p.bump();
        p.eat(Comma);
    }
    if p.at(Star) {
        p.bump();
        as_name(p);
    } else if p.at(LBrace) {
        specifiers(p);
    }
    if p.at_word("from") {
        p.bump();
    }
    p.expect(String, "se esperaba la ruta del módulo");
    attributes(p);
    semi(p);
    m.complete(p, ImportDecl);
}

/// `type` de `import type X` / `export type { X }`: modifica si lo sigue lo importado.
fn type_modifier(p: &mut Parser) -> bool {
    match p.nth(1) {
        LBrace | Star => true,
        Ident => !(p.nth_text(1) == "from" && p.nth(2) == String),
        _ => false,
    }
}

fn as_name(p: &mut Parser) {
    if p.at_word("as") {
        p.bump();
        if !p.eat(String) {
            name(p);
        }
    } else {
        p.error("se esperaba `as`");
    }
}

/// `{ a, b as c, type D, "e" as f }`.
fn specifiers(p: &mut Parser) {
    let open = p.pos();
    p.bump();
    while !matches!(p.current(), RBrace | EndOfFile) {
        let m = p.start();
        if p.at_word("type") && (is_name(p.nth(1)) || p.nth(1) == String) && p.nth_text(1) != "as" {
            p.bump();
        }
        if !p.eat(String) {
            name(p);
        }
        if p.at_word("as") {
            as_name(p);
        }
        m.complete(p, ImportSpecifier);
        if !p.eat(Comma) {
            break;
        }
    }
    p.close_group(open, RBrace);
}

/// `with { type: "json" }` o el antiguo `assert { ... }`.
fn attributes(p: &mut Parser) {
    if (p.at(KwWith) || p.at_word("assert")) && p.nth(1) == LBrace && !p.newline_before() {
        p.bump();
        primary::primary(p);
    }
}

pub(super) fn export(p: &mut Parser, m: Marker) {
    p.bump();
    match p.current() {
        KwDefault => {
            p.bump();
            export_default(p);
        }
        Eq => {
            p.bump();
            expr::expression(p);
            semi(p);
        }
        Star => {
            p.bump();
            if p.at_word("as") {
                as_name(p);
            }
            from_clause(p);
        }
        LBrace => {
            specifiers(p);
            from_clause(p);
        }
        Ident if p.at_word("type") && matches!(p.nth(1), LBrace | Star) => {
            p.bump();
            if p.eat(Star) && p.at_word("as") {
                as_name(p);
            } else if p.at(LBrace) {
                specifiers(p);
            }
            from_clause(p);
        }
        Ident if p.at_word("as") && p.nth_text(1) == "namespace" => {
            p.bump();
            p.bump();
            name(p);
            semi(p);
        }
        KwImport => import(p),
        _ => stmt::statement(p),
    }
    m.complete(p, ExportDecl);
}

fn export_default(p: &mut Parser) {
    let declaration = match p.current() {
        KwFunction | KwClass | At => true,
        Ident => {
            (p.at_word("async") && p.nth(1) == KwFunction)
                || (p.at_word("abstract") && p.nth(1) == KwClass)
                || (p.at_word("interface") && p.nth(1) == Ident)
        }
        _ => false,
    };
    if declaration {
        stmt::statement(p);
    } else {
        expr::assignment(p);
        semi(p);
    }
}

fn from_clause(p: &mut Parser) {
    if p.at_word("from") {
        p.bump();
        p.expect(String, "se esperaba la ruta del módulo");
        attributes(p);
    }
    semi(p);
}

/// Declaraciones de TypeScript que empiezan por una palabra contextual; `false` si no lo es.
pub(super) fn ts_declaration(p: &mut Parser) -> bool {
    let next = p.nth(1);
    let same_line = !p.newline_before_nth(1);
    match p.nth_text(0) {
        "interface" if next == Ident => interface(p),
        "type" if next == Ident && same_line => type_alias(p),
        "namespace" | "module" if matches!(next, Ident | String) && same_line => namespace(p),
        "global" if next == LBrace => namespace(p),
        "abstract" if next == KwClass && same_line => {
            let m = p.start();
            class::class(p, m);
        }
        "declare" if same_line && is_name(next) => {
            let m = p.start();
            p.bump();
            stmt::statement(p);
            m.complete(p, AmbientDecl);
        }
        _ => return false,
    }
    true
}

fn interface(p: &mut Parser) {
    let m = p.start();
    p.bump();
    p.bump();
    if p.at(Lt) {
        types::type_params(p);
    }
    if p.eat(KwExtends) {
        types::type_list(p);
    }
    types::object_type(p);
    m.complete(p, InterfaceDecl);
}

fn type_alias(p: &mut Parser) {
    let m = p.start();
    p.bump();
    p.bump();
    if p.at(Lt) {
        types::type_params(p);
    }
    p.expect(Eq, "se esperaba `=`");
    types::type_(p);
    semi(p);
    m.complete(p, TypeAliasStmt);
}

/// `enum E { A = 1, B }` y `const enum`.
pub(super) fn enum_decl(p: &mut Parser) {
    let m = p.start();
    p.eat(KwConst);
    p.bump();
    name(p);
    let open = p.pos();
    if p.expect(LBrace, "se esperaba `{`") {
        while !matches!(p.current(), RBrace | EndOfFile) {
            let member = p.start();
            property_name(p);
            if p.eat(Eq) {
                expr::assignment(p);
            }
            member.complete(p, Property);
            if !p.eat(Comma) {
                break;
            }
        }
        p.close_group(open, RBrace);
    }
    m.complete(p, EnumDecl);
}

/// `namespace A.B { }`, `module "x" { }`, `declare module "x";` y `declare global { }`.
fn namespace(p: &mut Parser) {
    let m = p.start();
    let global = p.at_word("global");
    p.bump();
    if !global && !p.eat(String) {
        name(p);
        while p.eat(Dot) {
            name(p);
        }
    }
    if p.at(LBrace) {
        block(p);
    } else {
        semi(p);
    }
    m.complete(p, ModuleDecl);
}
