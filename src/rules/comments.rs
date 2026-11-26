use std::collections::HashSet;

use super::{ClassifiedComment, FileContext, Rule};
use crate::lexer::comments::CommentKind;
use crate::reporters::{AgentAction, Diagnostic, Severity};

pub(super) const COMMENT_SOUP: Rule = Rule {
    id: "ARGUS-SOUP-001",
    name: "comment-soup",
    severity: Severity::Warning,
};

/// Con menos comentarios sospechosos que esto el ratio no es significativo (archivos diminutos).
const MIN_SUSPECT_COMMENTS: usize = 5;

/// La sopa LLM son frases cortas que parafrasean la línea siguiente ("# Obtener el total").
const MAX_SUSPECT_WORDS: usize = 6;

/// Un comentario es sospechoso si es narrativo, corto y no forma parte de un bloque: varias líneas
/// seguidas de comentario (explicaciones del porqué) o de comentarios finales (anotaciones de tablas)
/// las escriben personas.
fn suspects(comments: &[ClassifiedComment]) -> Vec<u32> {
    let lines: HashSet<(bool, u32)> = comments.iter().map(|c| (c.own_line, c.line)).collect();
    comments
        .iter()
        .filter(|c| c.kind == CommentKind::Narrative)
        .filter(|c| {
            let in_block = lines.contains(&(c.own_line, c.line.saturating_sub(1)))
                || lines.contains(&(c.own_line, c.line + 1));
            let words = c.text.trim_start_matches('#').split_whitespace().count();
            !in_block && words <= MAX_SUSPECT_WORDS
        })
        .map(|c| c.line)
        .collect()
}

pub(super) fn check(ctx: &FileContext, out: &mut Vec<Diagnostic>) {
    let suspect_lines = suspects(&ctx.comments);
    let code = ctx.code_lines().max(1);
    let ratio = suspect_lines.len() as f64 / f64::from(code);
    let max = ctx.budgets.max_comment_to_code_ratio;
    if suspect_lines.len() < MIN_SUSPECT_COMMENTS || ratio <= max {
        return;
    }
    let span = (
        suspect_lines.first().copied().unwrap_or(1),
        suspect_lines.last().copied().unwrap_or(1),
    );
    let message = format!(
        "{} comentarios narrativos cortos y aislados para {code} líneas de código (ratio {ratio:.2}; máximo {max:.2}).",
        suspect_lines.len()
    );
    let action = AgentAction::new(
        "remove_comments",
        "Eliminar los comentarios que repiten lo que el código ya dice y conservar los que explican el porqué. \
         No tocar docstrings, directivas, licencias ni TODO/FIXME.",
    )
    .with_lines(suspect_lines);
    out.push(COMMENT_SOUP.diagnostic(ctx.file, span, message, Some(action)));
}
