//! Pipeline de `argus scan`: recorrer, lexear, aplicar reglas y producir un [`Report`] determinista.

mod walk;

use std::fs;
use std::path::{Path, PathBuf};

use crate::arena::Language;
use crate::config::Config;
use crate::lexer::python;
use crate::reporters::{Diagnostic, Report};
use crate::rules::{self, FileContext, UNREADABLE_FILE};

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("no existe la ruta {0}")]
    NotFound(PathBuf),
    #[error("no se pudo leer {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

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
        if language != Language::Python {
            report.summary.files_unsupported += 1;
            continue;
        }
        report.summary.files_scanned += 1;
        let display = walk::display_path(root, &path);
        report.diagnostics.extend(analyze_python(&path, &display, config));
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
    config.general.languages.iter().any(|l| l.eq_ignore_ascii_case(name))
}

fn analyze_python(path: &Path, display: &str, config: &Config) -> Vec<Diagnostic> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => return vec![unreadable(display, &err.to_string())],
    };
    let lexed = match python::lex(&source) {
        Ok(lexed) => lexed,
        Err(err) => return vec![unreadable(display, &err.to_string())],
    };
    let ctx = FileContext::python(display, &source, &lexed, &config.budgets);
    rules::check_python(&ctx)
}

fn unreadable(display: &str, reason: &str) -> Diagnostic {
    UNREADABLE_FILE.diagnostic(
        display,
        (1, 1),
        format!("No se analizó el archivo: {reason}."),
        None,
    )
}
