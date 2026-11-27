//! Pipeline de `argus scan`: recorrer, lexear, aplicar reglas y producir un [`Report`] determinista.

mod baseline;
mod incremental;
mod walk;

use std::fs;
use std::path::{Path, PathBuf};

use crate::arena::Language;
use crate::config::Config;
use crate::git::GitError;
use crate::lexer::lex_file;
use crate::reporters::{Diagnostic, Report};
use crate::rules::{self, FileContext, UNREADABLE_FILE};

/// Media de bytes por línea por encima de la cual un archivo se trata como minificado o generado
/// (bundles copiados a `assets/`, `*.min.js`): el código escrito a mano ronda los 30-40.
const GENERATED_BYTES_PER_LINE: usize = 300;
const GENERATED_MIN_BYTES: usize = 4096;

enum Outcome {
    Analyzed(Vec<Diagnostic>),
    Generated,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("no existe la ruta {0}")]
    NotFound(PathBuf),
    #[error("no se pudo leer {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Git(#[from] GitError),
}

pub use incremental::diff;

pub fn scan(root: &Path, config: &Config) -> Result<Report, ScanError> {
    if !root.exists() {
        return Err(ScanError::NotFound(root.to_path_buf()));
    }
    let mut report = Report::default();
    for path in walk::collect(root, &config.general.all_excludes())? {
        let Some(language) = Language::from_path(&path) else {
            continue;
        };
        if !language_enabled(config, language) {
            continue;
        }
        let display = walk::display_path(root, &path);
        match analyze(&path, &display, language, config) {
            Outcome::Analyzed(diagnostics) => {
                report.summary.files_scanned += 1;
                report.diagnostics.extend(diagnostics);
            }
            Outcome::Generated => report.summary.files_generated += 1,
        }
    }
    report.finalize();
    Ok(report)
}

fn language_enabled(config: &Config, language: Language) -> bool {
    let name = match language {
        Language::Python => "python",
        Language::JavaScript => "javascript",
        Language::TypeScript => "typescript",
    };
    config
        .general
        .languages
        .iter()
        .any(|l| l.eq_ignore_ascii_case(name))
}

fn analyze(path: &Path, display: &str, language: Language, config: &Config) -> Outcome {
    match fs::read_to_string(path) {
        Ok(source) => analyze_source(&source, path, display, language, config),
        Err(err) => Outcome::Analyzed(unreadable(display, &err.to_string(), config)),
    }
}

/// Analiza `source` como si fuera el contenido de `path` (el modo incremental pasa la versión base).
fn analyze_source(
    source: &str,
    path: &Path,
    display: &str,
    language: Language,
    config: &Config,
) -> Outcome {
    if looks_generated(source) {
        return Outcome::Generated;
    }
    let lexed = match lex_file(source, language, path) {
        Ok(lexed) => lexed,
        Err(err) => return Outcome::Analyzed(unreadable(display, &err.to_string(), config)),
    };
    let ctx = FileContext::new(display, source, &lexed, language, config);
    Outcome::Analyzed(rules::check(&ctx))
}

fn looks_generated(source: &str) -> bool {
    let lines = source.lines().count().max(1);
    source.len() >= GENERATED_MIN_BYTES && source.len() / lines > GENERATED_BYTES_PER_LINE
}

fn unreadable(display: &str, reason: &str, config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = vec![UNREADABLE_FILE.diagnostic(
        display,
        (1, 1),
        format!("No se analizó el archivo: {reason}."),
        None,
    )];
    rules::apply_levels(config, &mut diagnostics);
    diagnostics
}
