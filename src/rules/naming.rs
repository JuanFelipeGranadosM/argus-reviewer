//! Consistencia de nombres de funciones. No hay una convención universal (PEP 8 admite `mixedCase`
//! donde ya domina; React usa `PascalCase` para componentes), así que la regla no impone una: exige la
//! del propio archivo, o la que el proyecto fije en `[style]`. Es una sugerencia: nunca bloquea.

use super::{FileContext, Rule};
use crate::arena::Language;
use crate::config::Naming;
use crate::reporters::{AgentAction, Diagnostic, Severity};

pub(super) const NAMING_INCONSISTENCY: Rule = Rule {
    id: "ARGUS-STYLE-001",
    name: "naming-inconsistency",
    severity: Severity::Suggestion,
};

/// Con `auto`, la convención del archivo solo cuenta si la siguen al menos estas funciones...
const MIN_EVIDENCE: usize = 5;
/// ...y este porcentaje de las que tienen una convención reconocible.
const MIN_SHARE_PERCENT: usize = 80;

/// Nombres que impone un framework aunque no sigan la convención del archivo.
const FRAMEWORK_NAMES: &[&str] = &[
    "setUp",
    "tearDown",
    "setUpClass",
    "tearDownClass",
    "setUpModule",
    "tearDownModule",
    "asyncSetUp",
    "asyncTearDown",
];

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Case {
    Snake,
    Camel,
}

impl Case {
    fn label(self) -> &'static str {
        match self {
            Case::Snake => "snake_case",
            Case::Camel => "camelCase",
        }
    }
}

type Named<'a> = (&'a str, Case, u32);

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let setting = match ctx.language {
        Language::Python => ctx.config.style.python_function_naming,
        Language::JavaScript | Language::TypeScript => ctx.config.style.js_function_naming,
    };
    let named: Vec<Named> = ctx
        .outline
        .functions
        .iter()
        .filter_map(|f| {
            let name = f.name.as_deref()?;
            let case = case_of(name)?;
            Some((name, case, ctx.lines.line(f.span.start)))
        })
        .collect();
    let Some((expected, reason)) = expected_case(setting, &named) else {
        return;
    };
    for &(name, case, line) in named.iter().filter(|(_, case, _)| *case != expected) {
        let message = format!("`{name}` usa {}; {reason}.", case.label());
        let action = AgentAction::new(
            "rename_symbol",
            format!(
                "Renombrar a `{}` y actualizar sus usos, o fijar la convención del proyecto en `[style]` de `.argus.toml`.",
                convert(name, expected)
            ),
        )
        .with_symbol(Some(name.to_owned()));
        out.push(NAMING_INCONSISTENCY.diagnostic(ctx.file, (line, line), message, Some(action)));
    }
}

/// La convención que se exige y por qué; `None` si no hay ninguna clara.
fn expected_case(setting: Naming, named: &[Named]) -> Option<(Case, String)> {
    let fixed = |case: Case| {
        Some((
            case,
            format!("el proyecto fija {} en `[style]`", case.label()),
        ))
    };
    match setting {
        Naming::Off => None,
        Naming::SnakeCase => fixed(Case::Snake),
        Naming::CamelCase => fixed(Case::Camel),
        Naming::Auto => {
            let snake = named.iter().filter(|(_, c, _)| *c == Case::Snake).count();
            let total = named.len();
            let (case, major) = if snake * 2 >= total {
                (Case::Snake, snake)
            } else {
                (Case::Camel, total - snake)
            };
            let clear = major >= MIN_EVIDENCE && major * 100 >= MIN_SHARE_PERCENT * total;
            clear.then(|| {
                let label = case.label();
                let reason =
                    format!("el resto del archivo usa {label} ({major} de {total} funciones)");
                (case, reason)
            })
        }
    }
}

/// `snake_case` o `camelCase`; `None` para una sola palabra, `PascalCase` (clases, componentes),
/// nombres especiales (`__init__`) o mezclas (`visit_Name`), que no indican convención.
fn case_of(name: &str) -> Option<Case> {
    if (name.starts_with("__") && name.ends_with("__")) || FRAMEWORK_NAMES.contains(&name) {
        return None;
    }
    let core = name
        .trim_start_matches(['_', '#', '$'])
        .trim_end_matches('_');
    if !core.chars().next()?.is_ascii_lowercase() {
        return None;
    }
    let upper = core.chars().any(|c| c.is_ascii_uppercase());
    match (core.contains('_'), upper) {
        (true, false) => Some(Case::Snake),
        (false, true) => Some(Case::Camel),
        _ => None,
    }
}

/// `get_user_name` ⇄ `getUserName`, conservando los `_`, `#` o `$` iniciales.
fn convert(name: &str, to: Case) -> String {
    let core = name.trim_start_matches(['_', '#', '$']);
    let prefix = name.get(..name.len() - core.len()).unwrap_or_default();
    let mut out = String::from(prefix);
    let mut upper_next = false;
    for c in core.chars() {
        match to {
            Case::Camel if c == '_' => upper_next = true,
            Case::Camel if upper_next => {
                out.push(c.to_ascii_uppercase());
                upper_next = false;
            }
            Case::Snake if c.is_ascii_uppercase() => {
                out.push('_');
                out.push(c.to_ascii_lowercase());
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cases_and_conversions() {
        assert_eq!(case_of("get_user"), Some(Case::Snake));
        assert_eq!(case_of("_getUser"), Some(Case::Camel));
        for neutral in [
            "run",
            "Componente",
            "__init__",
            "visit_Name",
            "setUp",
            "MAX_X",
        ] {
            assert_eq!(case_of(neutral), None, "{neutral}");
        }
        assert_eq!(convert("_get_user_name", Case::Camel), "_getUserName");
        assert_eq!(convert("getUserName", Case::Snake), "get_user_name");
    }
}
