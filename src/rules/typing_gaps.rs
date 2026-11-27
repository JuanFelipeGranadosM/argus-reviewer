use super::{FileContext, Rule};
use crate::arena::Language;
use crate::lexer::comments::js_comment_body;
use crate::reporters::{AgentAction, Diagnostic, Severity};

pub(super) const BARE_SUPPRESSION: Rule = Rule {
    id: "ARGUS-TYPE-001",
    name: "bare-suppression",
    severity: Severity::Warning,
};

/// (marcador sin espacios, carácter que debe seguirle, instrucción de remedio).
const PY_SUPPRESSIONS: &[(&str, char, &str)] = &[
    (
        "type:ignore",
        '[',
        "Usar `# type: ignore[código]` con el código concreto, o corregir el error.",
    ),
    (
        "pyright:ignore",
        '[',
        "Usar `# pyright: ignore[regla]` con la regla concreta, o corregir el error.",
    ),
    (
        "noqa",
        ':',
        "Usar `# noqa: CÓDIGO` con el código concreto, o corregir el error.",
    ),
];

const TS_IGNORE_FIX: &str =
    "Usar `// @ts-expect-error: motivo`, que falla cuando el error desaparece, o corregir el tipo.";
const TS_NOCHECK_FIX: &str =
    "Quitar `@ts-nocheck` y corregir los errores: desactiva TypeScript en todo el archivo.";
const ESLINT_FIX: &str =
    "Nombrar la regla: `eslint-disable-next-line regla -- motivo`, o corregir el aviso.";

const ESLINT_DISABLES: &[&str] = &[
    "eslint-disable-next-line",
    "eslint-disable-line",
    "eslint-disable",
];

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    for comment in &ctx.comments {
        let Some(fix) = bare_fix(ctx.language, comment.text) else {
            continue;
        };
        let message = format!(
            "Supresión genérica: `{}` silencia cualquier error, no uno concreto.",
            comment.text.trim()
        );
        let action = AgentAction::new("add_suppression_code", fix).with_lines(vec![comment.line]);
        out.push(BARE_SUPPRESSION.diagnostic(
            ctx.file,
            (comment.line, comment.line),
            message,
            Some(action),
        ));
    }
}

/// Instrucción de remedio si el comentario es una supresión sin código ni regla concreta.
fn bare_fix(language: Language, text: &str) -> Option<&'static str> {
    match language {
        Language::Python => python_bare_fix(text),
        Language::JavaScript | Language::TypeScript => js_bare_fix(text),
    }
}

fn python_bare_fix(text: &str) -> Option<&'static str> {
    let compact: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    PY_SUPPRESSIONS
        .iter()
        .find(|(marker, required, _)| is_bare(&compact, marker, *required))
        .map(|&(_, _, fix)| fix)
}

fn js_bare_fix(text: &str) -> Option<&'static str> {
    let body = js_comment_body(text).to_lowercase();
    if body.starts_with("@ts-ignore") {
        return Some(TS_IGNORE_FIX);
    }
    if body.starts_with("@ts-nocheck") {
        return Some(TS_NOCHECK_FIX);
    }
    let rules = ESLINT_DISABLES
        .iter()
        .find_map(|marker| body.strip_prefix(marker))?;
    let named = rules
        .split("--")
        .next()
        .is_some_and(|names| !names.trim().is_empty());
    (!named).then_some(ESLINT_FIX)
}

fn is_bare(compact: &str, marker: &str, required: char) -> bool {
    compact.match_indices(marker).any(|(at, _)| {
        compact
            .get(at + marker.len()..)
            .and_then(|rest| rest.chars().next())
            != Some(required)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_python_suppressions_without_code() {
        assert!(is_bare("#type:ignore", "type:ignore", '['));
        assert!(!is_bare("#type:ignore[attr-defined]", "type:ignore", '['));
        assert!(is_bare("#noqa", "noqa", ':'));
        assert!(!is_bare("#noqa:e501", "noqa", ':'));
        assert!(is_bare("#x#type:ignore#noqa:e1", "type:ignore", '['));
    }

    #[test]
    fn detects_js_suppressions_without_rule() {
        let bare = [
            "// @ts-ignore",
            "/* @ts-nocheck */",
            "// eslint-disable-next-line",
            "/* eslint-disable */",
            "// eslint-disable-line -- motivo",
        ];
        for text in bare {
            assert!(js_bare_fix(text).is_some(), "{text}");
        }
        let specific = [
            "// @ts-expect-error: la lib no tipa esto",
            "// eslint-disable-next-line no-console",
            "/* eslint-disable react-hooks/exhaustive-deps -- motivo */",
            "// Obtener el usuario",
        ];
        for text in specific {
            assert!(js_bare_fix(text).is_none(), "{text}");
        }
    }
}
