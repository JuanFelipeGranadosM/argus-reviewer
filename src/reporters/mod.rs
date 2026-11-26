//! Modelo unificado de diagnósticos. El JSON de `--format agent` es un contrato público: los campos
//! solo se añaden, nunca se renombran ni cambian de tipo.

pub mod agent;
pub mod human;

use serde::Serialize;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// Instrucción de remediación lista para que un agente la ejecute en un solo turno.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentAction {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<u32>,
    pub instruction: String,
}

impl AgentAction {
    pub fn new(kind: &'static str, instruction: impl Into<String>) -> Self {
        Self {
            kind,
            target_symbol: None,
            target_file: None,
            lines: Vec::new(),
            instruction: instruction.into(),
        }
    }

    pub fn with_lines(mut self, lines: Vec<u32>) -> Self {
        self.lines = lines;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diagnostic {
    pub id: &'static str,
    pub severity: Severity,
    pub rule: &'static str,
    pub file: String,
    pub line_start: u32,
    pub line_end: u32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_action: Option<AgentAction>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Summary {
    pub files_scanned: usize,
    /// Archivos de lenguajes reconocidos cuyo análisis aún no está implementado (JS/TS en esta versión).
    pub files_unsupported: usize,
    pub blocking_issues: usize,
    pub warnings: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Report {
    pub summary: Summary,
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Ordena los diagnósticos de forma determinista y recalcula los contadores del resumen.
    pub fn finalize(&mut self) {
        self.diagnostics.sort_by(|a, b| {
            (&a.file, a.line_start, a.id).cmp(&(&b.file, b.line_start, b.id))
        });
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        self.summary.blocking_issues = errors;
        self.summary.warnings = self.diagnostics.len() - errors;
    }

    pub fn has_blocking_issues(&self) -> bool {
        self.summary.blocking_issues > 0
    }
}
