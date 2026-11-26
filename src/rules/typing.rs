//! Fronteras sin tipar de `[typing]`: `any` explícito en TypeScript y funciones públicas de Python sin
//! anotaciones. Se emite un diagnóstico por archivo con todas las líneas, no uno por aparición: al
//! agente le basta una instrucción y la lista de sitios.

use super::{FileContext, Rule};
use crate::arena::Language;
use crate::reporters::{AgentAction, Diagnostic, Severity};

pub(super) const EXPLICIT_ANY: Rule = Rule {
    id: "ARGUS-TYPE-002",
    name: "explicit-any",
    severity: Severity::Error,
};

pub(super) const MISSING_ANNOTATIONS: Rule = Rule {
    id: "ARGUS-TYPE-003",
    name: "missing-annotations",
    severity: Severity::Warning,
};

/// Nombres que se citan en el mensaje; el resto va en `agent_action.lines`.
const NAMES_IN_MESSAGE: usize = 5;

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    if ctx.config.typing.forbid_ts_explicit_any {
        explicit_any(ctx, out);
    }
    if ctx.language == Language::Python {
        missing_annotations(ctx, out);
    }
}

fn explicit_any(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let uses = ctx.outline.explicit_any.len();
    let mut lines: Vec<u32> = ctx
        .outline
        .explicit_any
        .iter()
        .map(|range| ctx.lines.line(range.start))
        .collect();
    lines.dedup();
    let (Some(&first), Some(&last)) = (lines.first(), lines.last()) else {
        return;
    };
    let noun = if uses == 1 { "uso" } else { "usos" };
    let message =
        format!("{uses} {noun} de `any` explícito: TypeScript deja de comprobar esos valores.");
    let action = AgentAction::new(
        "replace_any",
        "Sustituir cada `any` por el tipo concreto, por `unknown` con una validación o por un genérico.",
    )
    .with_lines(lines);
    let diagnostic = EXPLICIT_ANY.diagnostic(ctx.file, (first, last), message, Some(action));
    out.push(diagnostic.with_metric(u32::try_from(uses).unwrap_or(u32::MAX)));
}

fn missing_annotations(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let typing = &ctx.config.typing;
    let mut lines = Vec::new();
    let mut names = Vec::new();
    for function in &ctx.outline.functions {
        let Some(name) = function.name.as_deref() else {
            continue;
        };
        if function.nested || !is_public(name) {
            continue;
        }
        let params =
            typing.require_python_param_types && function.params.iter().any(|p| !p.annotated);
        let returns =
            typing.require_python_return_types && !function.has_return_type && name != "__init__";
        if params || returns {
            lines.push(ctx.lines.line(function.span.start));
            names.push(format!("`{name}`"));
        }
    }
    let (Some(&first), Some(&last)) = (lines.first(), lines.last()) else {
        return;
    };
    let count = names.len();
    let mut cited = names
        .into_iter()
        .take(NAMES_IN_MESSAGE)
        .collect::<Vec<_>>()
        .join(", ");
    if count > NAMES_IN_MESSAGE {
        cited.push_str(", …");
    }
    let noun = if count == 1 {
        "función pública"
    } else {
        "funciones públicas"
    };
    let message = format!("{count} {noun} sin anotar del todo: {cited}.");
    let action = AgentAction::new(
        "add_type_annotations",
        "Anotar todos los parámetros y el tipo de retorno de las funciones indicadas (`-> None` si no devuelven nada).",
    )
    .with_lines(lines);
    let diagnostic =
        MISSING_ANNOTATIONS.diagnostic(ctx.file, (first, last), message, Some(action));
    out.push(diagnostic.with_metric(u32::try_from(count).unwrap_or(u32::MAX)));
}

/// Frontera pública: sin `_` inicial (los dunder sí cuentan) y sin las pruebas `test*`, que no
/// tienen llamadores.
fn is_public(name: &str) -> bool {
    let dunder = name.len() > 4 && name.starts_with("__") && name.ends_with("__");
    !name.starts_with("test") && (dunder || !name.starts_with('_'))
}
