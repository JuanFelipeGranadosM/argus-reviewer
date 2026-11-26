//! Motor de reglas. Cada regla tiene un ID estable (`ARGUS-<ÁREA>-<NNN>`) que los agentes y las
//! supresiones pueden referenciar; un ID publicado nunca se reutiliza para otra regla.

mod bloat;
mod comments;
mod functions;
mod safety;
mod typing;
mod typing_gaps;

use crate::arena::Language;
use crate::config::{Budgets, Config};
use crate::lexer::comments::{classify_js, classify_python, CommentKind};
use crate::lexer::Lexed;
use crate::reporters::{AgentAction, Diagnostic, Severity};
use crate::structure::{self, Outline};
use crate::syntax::{LineIndex, SyntaxKind, TextRange};

pub struct Rule {
    pub id: &'static str,
    pub name: &'static str,
    pub severity: Severity,
}

pub const SYNTAX_ERROR: Rule = Rule {
    id: "ARGUS-SYNTAX-001",
    name: "syntax-error",
    severity: Severity::Error,
};

pub const UNREADABLE_FILE: Rule = Rule {
    id: "ARGUS-IO-001",
    name: "unreadable-file",
    severity: Severity::Warning,
};

impl Rule {
    pub fn diagnostic(
        &self,
        file: &str,
        (line_start, line_end): (u32, u32),
        message: String,
        agent_action: Option<AgentAction>,
    ) -> Diagnostic {
        Diagnostic {
            id: self.id,
            severity: self.severity,
            rule: self.name,
            file: file.to_owned(),
            line_start,
            line_end,
            message,
            agent_action,
        }
    }
}

pub struct ClassifiedComment<'a> {
    pub text: &'a str,
    pub line: u32,
    pub kind: CommentKind,
    /// El comentario ocupa su propia línea (no va detrás de código).
    pub own_line: bool,
}

/// Todo lo que una regla necesita saber de un archivo, calculado una sola vez.
pub struct FileContext<'a> {
    pub file: &'a str,
    pub source: &'a str,
    pub lexed: &'a Lexed,
    pub language: Language,
    pub lines: LineIndex,
    pub comments: Vec<ClassifiedComment<'a>>,
    pub outline: Outline,
    pub budgets: &'a Budgets,
    pub config: &'a Config,
}

impl<'a> FileContext<'a> {
    pub fn new(
        file: &'a str,
        source: &'a str,
        lexed: &'a Lexed,
        language: Language,
        config: &'a Config,
    ) -> Self {
        let lines = LineIndex::new(source);
        let classify = match language {
            Language::Python => classify_python,
            Language::JavaScript | Language::TypeScript => classify_js,
        };
        let mut comments = Vec::new();
        let mut in_header = true;
        for token in &lexed.tokens {
            if token.kind == SyntaxKind::Comment {
                let text = token.text(source);
                let line = lines.line(token.range.start);
                let line_start = lines.line_start(line).unwrap_or(token.range.start);
                let before = TextRange::new(line_start, token.range.start).slice(source);
                comments.push(ClassifiedComment {
                    text,
                    line,
                    kind: classify(text, in_header),
                    own_line: before.trim().is_empty(),
                });
            } else if !token.kind.is_trivia() {
                in_header = false;
            }
        }
        Self {
            file,
            source,
            lexed,
            language,
            lines,
            comments,
            outline: structure::outline(source, lexed, language),
            budgets: &config.budgets,
            config,
        }
    }

    /// Líneas (primera y última) que ocupa un rango.
    pub fn line_span(&self, range: TextRange) -> (u32, u32) {
        let start = self.lines.line(range.start);
        let end = self.lines.line(range.end.saturating_sub(1)).max(start);
        (start, end)
    }

    /// Líneas que contienen al menos un token significativo (las cadenas multilínea cuentan entera).
    pub fn code_lines(&self) -> u32 {
        let mut seen = vec![false; self.lines.line_count() as usize + 2];
        for token in &self.lexed.tokens {
            if token.kind.is_trivia()
                || token.range.is_empty()
                || token.kind == SyntaxKind::LogicalNewline
            {
                continue;
            }
            let first = self.lines.line(token.range.start);
            let last = self.lines.line(token.range.end.saturating_sub(1));
            for line in first..=last {
                if let Some(slot) = seen.get_mut(line as usize) {
                    *slot = true;
                }
            }
        }
        u32::try_from(seen.iter().filter(|&&s| s).count()).unwrap_or(u32::MAX)
    }
}

pub fn check(ctx: &FileContext) -> Vec<Diagnostic> {
    let mut out = syntax_errors(ctx);
    bloat::check(ctx, &mut out);
    comments::check(ctx, &mut out);
    typing_gaps::check(ctx, &mut out);
    functions::check(ctx, &mut out);
    typing::check(ctx, &mut out);
    safety::check(ctx, &mut out);
    out
}

fn syntax_errors(ctx: &FileContext) -> Vec<Diagnostic> {
    ctx.lexed
        .errors
        .iter()
        .map(|error| {
            let start = ctx.lines.line(error.range.start);
            let end = ctx.lines.line(error.range.end.saturating_sub(1)).max(start);
            let action = AgentAction::new(
                "fix_syntax",
                "Corregir el error de sintaxis; el resto del archivo se analizó con recuperación de errores.",
            );
            SYNTAX_ERROR.diagnostic(ctx.file, (start, end), error.kind.to_string(), Some(action))
        })
        .collect()
}
