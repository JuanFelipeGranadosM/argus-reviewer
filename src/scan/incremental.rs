//! `argus diff`: analiza solo lo que cambió desde una revisión y lista solo los problemas que el cambio
//! introduce o empeora. Los que ya existían se cuentan en el resumen: el agente recibe lo que acaba de
//! romper, no el historial del proyecto, y el informe no crece con el tamaño del repositorio.

use std::fs;
use std::path::{Path, PathBuf};

use super::{analyze, analyze_source, baseline, language_enabled, walk, Outcome, ScanError};
use crate::arena::Language;
use crate::config::Config;
use crate::git::{Change, Repo};
use crate::reporters::{Diagnostic, Incremental, Report};

pub fn diff(root: &Path, config: &Config, base: &str) -> Result<Report, ScanError> {
    let scope = fs::canonicalize(root).map_err(|_| ScanError::NotFound(root.to_path_buf()))?;
    let dir = if scope.is_file() {
        scope.parent().unwrap_or(&scope)
    } else {
        &scope
    };
    let repo = Repo::discover(dir)?;
    let base_commit = repo.merge_base(base)?;
    let excludes = config.general.all_excludes();
    let mut report = Report::default();
    let mut incremental = Incremental {
        base: base.to_owned(),
        base_commit: base_commit.clone(),
        ..Incremental::default()
    };
    let files: Vec<File> = repo
        .changes(&base_commit)?
        .into_iter()
        .filter_map(|change| in_scope(&repo, &scope, change, &excludes))
        .filter(|file| language_enabled(config, file.language))
        .collect();
    let old_paths: Vec<&str> = files
        .iter()
        .filter_map(|f| f.change.base_path.as_deref())
        .collect();
    let base_texts = repo.read_all(&base_commit, &old_paths)?;
    for file in &files {
        incremental.files_changed += 1;
        let Outcome::Analyzed(current) = analyze(&file.path, &file.display, file.language, config)
        else {
            report.summary.files_generated += 1;
            continue;
        };
        report.summary.files_scanned += 1;
        let base_text = file
            .change
            .base_path
            .as_deref()
            .and_then(|old| base_texts.get(old));
        let before = base_diagnostics(file, base_text, config);
        let (fresh, preexisting) = baseline::split(current, &before, |d| touches(&file.change, d));
        incremental.preexisting_issues += preexisting;
        report.diagnostics.extend(fresh);
    }
    report.summary.incremental = Some(incremental);
    report.finalize();
    Ok(report)
}

/// Un archivo cambiado que se audita.
struct File {
    change: Change,
    path: PathBuf,
    display: String,
    language: Language,
}

/// El cambio, si cae bajo `scope`, sigue existiendo, no está excluido y es de un lenguaje conocido.
fn in_scope(repo: &Repo, scope: &Path, change: Change, excludes: &[String]) -> Option<File> {
    let path = fs::canonicalize(repo.root().join(&change.path)).ok()?;
    if !path.starts_with(scope) || !path.is_file() {
        return None;
    }
    let display = walk::display_path(scope, &path);
    if walk::excluded_path(&display, excludes) {
        return None;
    }
    let language = Language::from_path(&path)?;
    Some(File {
        change,
        path,
        display,
        language,
    })
}

/// Diagnósticos de la versión base del archivo, analizada con la misma ruta y configuración.
fn base_diagnostics(file: &File, text: Option<&String>, config: &Config) -> Vec<Diagnostic> {
    let Some(text) = text else {
        return Vec::new();
    };
    match analyze_source(text, &file.path, &file.display, file.language, config) {
        Outcome::Analyzed(diagnostics) => diagnostics,
        Outcome::Generated => Vec::new(),
    }
}

/// Si el cambio toca las líneas del diagnóstico: las de `agent_action.lines` cuando las hay (un
/// diagnóstico por archivo que enumera sitios) o, si no, su rango.
fn touches(change: &Change, diagnostic: &Diagnostic) -> bool {
    let Some(ranges) = &change.lines else {
        return true;
    };
    let hits = |line: u32| ranges.iter().any(|&(s, e)| s <= line && line <= e);
    match diagnostic.agent_action.as_ref().map(|a| a.lines.as_slice()) {
        Some(lines) if !lines.is_empty() => lines.iter().any(|&line| hits(line)),
        _ => ranges
            .iter()
            .any(|&(s, e)| s <= diagnostic.line_end && diagnostic.line_start <= e),
    }
}
