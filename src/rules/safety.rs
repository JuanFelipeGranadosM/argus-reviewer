//! Errores silenciosos y ejecución dinámica de código de `[security]`.

use super::{FileContext, Rule};
use crate::arena::Language;
use crate::reporters::{AgentAction, Diagnostic, Severity};

pub(super) const EMPTY_CATCH: Rule = Rule {
    id: "ARGUS-SEC-001",
    name: "empty-catch",
    severity: Severity::Error,
};

pub(super) const DYNAMIC_EVAL: Rule = Rule {
    id: "ARGUS-SEC-002",
    name: "dynamic-eval",
    severity: Severity::Error,
};

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    if ctx.config.security.forbid_empty_catch {
        empty_catches(ctx, out);
    }
    if ctx.config.security.forbid_dynamic_eval {
        dynamic_evals(ctx, out);
    }
}

fn empty_catches(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let message = match ctx.language {
        Language::Python => {
            "`except` genérico que solo hace `pass`: oculta cualquier fallo, también los errores de programación."
        }
        Language::JavaScript | Language::TypeScript => {
            "Captura vacía: el error se descarta en silencio."
        }
    };
    for catch in ctx.outline.catches.iter().filter(|c| c.silent) {
        let action = AgentAction::new(
            "handle_error",
            "Registrar o propagar el error. Si ignorarlo es intencional, capturar solo el caso esperado y dejar un comentario con el motivo.",
        );
        let lines = ctx.line_span(catch.span);
        out.push(EMPTY_CATCH.diagnostic(ctx.file, lines, message.to_owned(), Some(action)));
    }
}

fn dynamic_evals(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    for range in &ctx.outline.dynamic_evals {
        let call = match range.slice(ctx.source) {
            "Function" => "new Function",
            name => name,
        };
        let action = AgentAction::new(
            "remove_dynamic_eval",
            "Sustituir por un parser explícito (`JSON.parse`, `ast.literal_eval`) o por una tabla de funciones permitidas.",
        );
        let lines = ctx.line_span(*range);
        let message = format!("`{call}(...)` ejecuta texto como código.");
        out.push(DYNAMIC_EVAL.diagnostic(ctx.file, lines, message, Some(action)));
    }
}
