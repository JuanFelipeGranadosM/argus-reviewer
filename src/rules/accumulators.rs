//! Bucles que solo llenan una colección: la sugerencia trae la expresión funcional equivalente ya
//! escrita, para que el agente la aplique sin volver a leer el código. Es una sugerencia porque hay
//! equipos que prefieren el bucle explícito; se apaga con `[rules]`.

use super::{FileContext, Rule};
use crate::arena::Language;
use crate::reporters::{AgentAction, Diagnostic, Severity};
use crate::structure::Collection;

pub(super) const ACCUMULATOR_LOOP: Rule = Rule {
    id: "ARGUS-STYLE-002",
    name: "accumulator-loop",
    severity: Severity::Suggestion,
};

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    for accumulator in &ctx.outline.accumulators {
        let name = &accumulator.name;
        let form = match (ctx.language, accumulator.collection) {
            (Language::Python, Collection::List) => "una comprensión de lista",
            (Language::Python, Collection::Set) => "una comprensión de conjunto",
            (Language::Python, Collection::Dict) => "una comprensión de diccionario",
            (Language::JavaScript | Language::TypeScript, _) => "una expresión sin mutación",
        };
        let message = format!(
            "`{name}` se llena con un bucle que solo inserta: equivale a {form}, más corta y sin estado intermedio."
        );
        let (start, end) = ctx.line_span(accumulator.span);
        let mut instruction = format!(
            "Sustituir las líneas {start}-{end} por `{}`.",
            accumulator.replacement
        );
        if let Some(caveat) = &accumulator.caveat {
            instruction.push(' ');
            instruction.push_str(caveat);
        }
        let action = AgentAction::new("rewrite_as_expression", instruction)
            .with_symbol(Some(name.clone()));
        out.push(ACCUMULATOR_LOOP.diagnostic(ctx.file, (start, end), message, Some(action)));
    }
}
