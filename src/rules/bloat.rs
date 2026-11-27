use super::{FileContext, Rule};
use crate::reporters::{AgentAction, Diagnostic, Severity};

pub(super) const FILE_TOO_LONG: Rule = Rule {
    id: "ARGUS-BLOAT-001",
    name: "file-too-long",
    severity: Severity::Error,
};

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let lines = ctx.lines.line_count();
    let max = ctx.budgets.max_file_lines;
    if lines <= max {
        return;
    }
    let action = AgentAction::new(
        "split_module",
        format!(
            "Dividir el archivo en módulos cohesivos de como máximo {max} líneas y actualizar los imports que lo referencian."
        ),
    );
    let message = format!("El archivo tiene {lines} líneas; el presupuesto es {max}.");
    let diagnostic = FILE_TOO_LONG.diagnostic(ctx.file, (1, lines), message, Some(action));
    out.push(diagnostic.with_metric(lines));
}
