use super::{FileContext, Rule};
use crate::reporters::{AgentAction, Diagnostic, Severity};

pub(super) const BARE_SUPPRESSION: Rule = Rule {
    id: "ARGUS-TYPE-001",
    name: "bare-suppression",
    severity: Severity::Warning,
};

/// (marcador sin espacios, carácter que debe seguirle, forma correcta).
const SUPPRESSIONS: &[(&str, char, &str)] = &[
    ("type:ignore", '[', "`# type: ignore[código]`"),
    ("pyright:ignore", '[', "`# pyright: ignore[regla]`"),
    ("noqa", ':', "`# noqa: CÓDIGO`"),
];

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    for comment in &ctx.comments {
        let compact: String = comment
            .text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase();
        let Some(&(_, _, correct)) = SUPPRESSIONS
            .iter()
            .find(|(marker, required, _)| is_bare(&compact, marker, *required))
        else {
            continue;
        };
        let message = format!(
            "Supresión sin código: `{}` silencia cualquier error de la línea.",
            comment.text.trim()
        );
        let action = AgentAction::new(
            "add_suppression_code",
            format!("Usar {correct} con el código concreto, o corregir el error y quitar la supresión."),
        )
        .with_lines(vec![comment.line]);
        out.push(BARE_SUPPRESSION.diagnostic(ctx.file, (comment.line, comment.line), message, Some(action)));
    }
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
    use super::is_bare;

    #[test]
    fn detects_suppressions_without_code() {
        assert!(is_bare("#type:ignore", "type:ignore", '['));
        assert!(!is_bare("#type:ignore[attr-defined]", "type:ignore", '['));
        assert!(is_bare("#noqa", "noqa", ':'));
        assert!(!is_bare("#noqa:e501", "noqa", ':'));
        assert!(is_bare("#x#type:ignore#noqa:e1", "type:ignore", '['));
    }
}
