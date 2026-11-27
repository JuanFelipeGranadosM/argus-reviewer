//! Modelo unificado de diagnósticos. El JSON de `--format agent` es un contrato público: los campos
//! solo se añaden, nunca se renombran ni cambian de tipo.

pub mod agent;
pub mod human;

use serde::Serialize;

/// `error` bloquea (código de salida 1); `warning` señala un problema probable; `suggestion` es una
/// recomendación de estilo con varias escuelas válidas, nunca bloquea y se puede apagar o endurecer
/// por regla en `[rules]`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Suggestion,
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

    pub fn with_symbol(mut self, symbol: Option<String>) -> Self {
        self.target_symbol = symbol;
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
    /// Magnitud del problema (líneas, parámetros, complejidad, usos), o 0 si no tiene. No se publica:
    /// el modo incremental la usa para saber si un problema que ya existía ha empeorado.
    #[serde(skip)]
    pub metric: u32,
}

impl Diagnostic {
    pub fn with_metric(mut self, metric: u32) -> Self {
        self.metric = metric;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Summary {
    pub files_scanned: usize,
    /// Archivos de lenguajes reconocidos cuyo análisis no está implementado (hoy ninguno).
    pub files_unsupported: usize,
    /// Archivos minificados o generados que se omiten: no los escribió una persona ni un agente.
    pub files_generated: usize,
    pub blocking_issues: usize,
    pub warnings: usize,
    pub suggestions: usize,
    /// Solo en `argus diff`: contra qué se comparó y cuántos problemas previos no se listan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incremental: Option<Incremental>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Incremental {
    /// La revisión pedida (`HEAD`, `main`, ...).
    pub base: String,
    /// El commit con el que se comparó: el ancestro común de `base` y `HEAD`.
    pub base_commit: String,
    pub files_changed: usize,
    /// Problemas que ya existían en la base en los archivos cambiados: se cuentan, no se listan.
    pub preexisting_issues: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Report {
    pub summary: Summary,
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Ordena los diagnósticos de forma determinista y recalcula los contadores del resumen.
    pub fn finalize(&mut self) {
        self.diagnostics
            .sort_by(|a, b| (&a.file, a.line_start, a.id).cmp(&(&b.file, b.line_start, b.id)));
        let count = |severity| {
            self.diagnostics
                .iter()
                .filter(|d| d.severity == severity)
                .count()
        };
        self.summary.blocking_issues = count(Severity::Error);
        self.summary.warnings = count(Severity::Warning);
        self.summary.suggestions = count(Severity::Suggestion);
    }

    pub fn has_blocking_issues(&self) -> bool {
        self.summary.blocking_issues > 0
    }
}
