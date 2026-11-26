//! Presupuestos por función de `[budgets]`: longitud, número de parámetros y complejidad ciclomática.

use super::{FileContext, Rule};
use crate::reporters::{AgentAction, Diagnostic, Severity};
use crate::structure::Function;

pub(super) const FUNCTION_TOO_LONG: Rule = Rule {
    id: "ARGUS-BLOAT-002",
    name: "function-too-long",
    severity: Severity::Error,
};

pub(super) const TOO_MANY_PARAMETERS: Rule = Rule {
    id: "ARGUS-BLOAT-003",
    name: "too-many-parameters",
    severity: Severity::Error,
};

pub(super) const HIGH_COMPLEXITY: Rule = Rule {
    id: "ARGUS-BLOAT-004",
    name: "high-complexity",
    severity: Severity::Error,
};

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let functions = &ctx.outline.functions;
    let spans: Vec<(u32, u32)> = functions.iter().map(|f| ctx.line_span(f.span)).collect();
    for (index, (function, &lines)) in functions.iter().zip(&spans).enumerate() {
        let inner = functions
            .iter()
            .zip(&spans)
            .skip(index + 1)
            .take_while(|(other, _)| contains(function, other))
            .map(|(_, &span)| span);
        let own = own_lines(lines, inner);
        length(ctx, function, (lines, own), out);
        parameters(ctx, function, lines, out);
        complexity(ctx, function, lines, out);
    }
}

fn contains(outer: &Function, inner: &Function) -> bool {
    outer.span.start <= inner.span.start && inner.span.end <= outer.span.end
}

/// Líneas de `lines` que no son interiores a una función anidada. La primera y la última línea de
/// cada anidada siguen contando, porque comparten línea con el código que la llama
/// (`it('...', () => {` y `});`).
fn own_lines((start, end): (u32, u32), inner: impl Iterator<Item = (u32, u32)>) -> u32 {
    let mut interiors: Vec<(u32, u32)> = inner
        .filter(|&(s, e)| e > s + 1)
        .map(|(s, e)| (s + 1, e - 1))
        .collect();
    interiors.sort_unstable();
    let (mut covered, mut reach) = (0, 0);
    for (s, e) in interiors {
        let s = s.max(reach + 1);
        if e >= s {
            covered += e - s + 1;
            reach = e;
        }
    }
    (end - start + 1).saturating_sub(covered)
}

/// `` `nombre` `` o «La función anónima de la línea N», para encabezar los mensajes.
fn subject(function: &Function, line: u32) -> String {
    function.name.as_ref().map_or_else(
        || format!("La función anónima de la línea {line}"),
        |name| format!("`{name}`"),
    )
}

/// Cuenta las líneas propias: un `describe(...)` que solo agrupa pruebas no es una función larga.
fn length(
    ctx: &FileContext,
    function: &Function,
    (lines, count): ((u32, u32), u32),
    out: &mut Vec<Diagnostic>,
) {
    let max = ctx.budgets.max_function_lines;
    if count <= max {
        return;
    }
    let who = subject(function, lines.0);
    let action = AgentAction::new(
        "extract_function",
        format!(
            "Extraer los bloques cohesivos de {who} a funciones con nombre propio hasta dejarla en {max} líneas o menos."
        ),
    )
    .with_symbol(function.name.clone());
    let message = format!(
        "{who} tiene {count} líneas propias, sin contar funciones anidadas; el presupuesto es {max}."
    );
    out.push(FUNCTION_TOO_LONG.diagnostic(ctx.file, lines, message, Some(action)));
}

fn parameters(
    ctx: &FileContext,
    function: &Function,
    lines: (u32, u32),
    out: &mut Vec<Diagnostic>,
) {
    let max = ctx.budgets.max_parameters;
    let count = u32::try_from(function.params.len()).unwrap_or(u32::MAX);
    if count <= max {
        return;
    }
    let who = subject(function, lines.0);
    let action = AgentAction::new(
        "introduce_parameter_object",
        "Agrupar los parámetros relacionados en un objeto (interfaz, dataclass o TypedDict) o dividir la función.",
    )
    .with_symbol(function.name.clone());
    let message = format!("{who} recibe {count} parámetros; el máximo es {max}.");
    let header = (lines.0, lines.0);
    out.push(TOO_MANY_PARAMETERS.diagnostic(ctx.file, header, message, Some(action)));
}

fn complexity(
    ctx: &FileContext,
    function: &Function,
    lines: (u32, u32),
    out: &mut Vec<Diagnostic>,
) {
    let max = ctx.budgets.max_cyclomatic_complexity;
    let value = function.complexity;
    if value <= max {
        return;
    }
    let who = subject(function, lines.0);
    let action = AgentAction::new(
        "reduce_complexity",
        format!(
            "Reducir las ramas de {who} con retornos tempranos, condiciones extraídas a funciones con nombre o una tabla de despacho, hasta {max} o menos."
        ),
    )
    .with_symbol(function.name.clone());
    let message = format!("{who} tiene complejidad ciclomática {value}; el máximo es {max}.");
    out.push(HIGH_COMPLEXITY.diagnostic(ctx.file, lines, message, Some(action)));
}
