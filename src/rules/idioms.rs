//! Construcciones que las guías de cada lenguaje desaconsejan sin discusión (PEP 8, guía de estilo de
//! TypeScript de Google): no dependen de la escuela de programación del proyecto.

use super::{FileContext, Rule};
use crate::reporters::{AgentAction, Diagnostic, Severity};
use crate::structure::{Idiom, IdiomKind};

pub(super) const NONE_EQUALITY: Rule = Rule {
    id: "ARGUS-IDIOM-001",
    name: "none-equality",
    severity: Severity::Warning,
};

pub(super) const LOOSE_EQUALITY: Rule = Rule {
    id: "ARGUS-IDIOM-002",
    name: "loose-equality",
    severity: Severity::Warning,
};

pub(super) const VAR_DECLARATION: Rule = Rule {
    id: "ARGUS-IDIOM-003",
    name: "var-declaration",
    severity: Severity::Warning,
};

pub(super) const DEBUGGER_STATEMENT: Rule = Rule {
    id: "ARGUS-IDIOM-004",
    name: "debugger-statement",
    severity: Severity::Error,
};

pub(super) const LAMBDA_ASSIGNMENT: Rule = Rule {
    id: "ARGUS-IDIOM-005",
    name: "lambda-assignment",
    severity: Severity::Suggestion,
};

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let declarations = ctx.file.ends_with(".d.ts");
    for idiom in &ctx.outline.idioms {
        if declarations && idiom.kind == IdiomKind::VarDeclaration {
            continue;
        }
        let (rule, message, kind, instruction) = describe(ctx, idiom);
        let action = AgentAction::new(kind, instruction);
        let lines = ctx.line_span(idiom.span);
        out.push(rule.diagnostic(ctx.file, lines, message, Some(action)));
    }
}

fn describe(ctx: &FileContext, idiom: &Idiom) -> (Rule, String, &'static str, &'static str) {
    let token = idiom.span.slice(ctx.source);
    match idiom.kind {
        IdiomKind::NoneEquality => (
            NONE_EQUALITY,
            format!("`{token} None`: la igualdad se puede redefinir con `__eq__`; la identidad no."),
            "use_identity_check",
            "Usar `is None` o `is not None`.",
        ),
        IdiomKind::LooseEquality => (
            LOOSE_EQUALITY,
            format!("`{token}` convierte los tipos antes de comparar (`'' == 0` es verdadero)."),
            "use_strict_equality",
            "Usar `===` o `!==`. Para comprobar a la vez `null` y `undefined` se admite `== null`.",
        ),
        IdiomKind::VarDeclaration => (
            VAR_DECLARATION,
            "`var` tiene ámbito de función y se eleva: fuente habitual de errores en bucles y cierres."
                .to_owned(),
            "use_let_const",
            "Declarar con `const`, o con `let` si la variable se reasigna.",
        ),
        IdiomKind::Debugger => (
            DEBUGGER_STATEMENT,
            "Sentencia `debugger` olvidada: detiene la ejecución con las herramientas de desarrollo abiertas."
                .to_owned(),
            "remove_debugger",
            "Eliminar la sentencia `debugger`.",
        ),
        IdiomKind::LambdaAssignment => (
            LAMBDA_ASSIGNMENT,
            "`lambda` asignada a un nombre: con `def` la función tiene nombre en las trazas, anotaciones y docstring."
                .to_owned(),
            "use_def",
            "Reescribir como `def nombre(...):`.",
        ),
    }
}
