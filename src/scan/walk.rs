//! Recorrido determinista del proyecto con la biblioteca estándar.
//!
//! Provisional: no respeta `.gitignore`. Se sustituirá por la crate `ignore` cuando el toolchain de
//! Windows pueda enlazarla (ver CLAUDE.md).

use std::fs;
use std::path::{Path, PathBuf};

use super::ScanError;
use crate::arena::Language;

/// Archivos de lenguajes reconocidos bajo `root`, ordenados. Omite enlaces simbólicos, entradas ocultas
/// (`.git`, `.venv`, ...) y las que coinciden con `excludes`.
pub(super) fn collect(root: &Path, excludes: &[String]) -> Result<Vec<PathBuf>, ScanError> {
    if root.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|source| ScanError::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let skip = file_type.is_symlink()
                || name.starts_with('.')
                || is_excluded(&display_path(root, &path), &name, excludes);
            if skip {
                continue;
            }
            if file_type.is_dir() {
                pending.push(path);
            } else if Language::from_path(&path).is_some() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Ruta relativa a `root` con `/` como separador, estable entre sistemas operativos.
pub(super) fn display_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let shown = if relative.as_os_str().is_empty() {
        Path::new(path.file_name().unwrap_or(path.as_os_str()))
    } else {
        relative
    };
    shown
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Si una ruta relativa (con `/`) quedaría fuera del recorrido de [`collect`]: algún componente oculto
/// o excluido.
pub(super) fn excluded_path(relative: &str, excludes: &[String]) -> bool {
    let parts: Vec<&str> = relative.split('/').collect();
    (1..=parts.len()).any(|n| {
        let prefix = parts.get(..n).unwrap_or_default();
        let name = prefix.last().copied().unwrap_or_default();
        name.starts_with('.') || is_excluded(&prefix.join("/"), name, excludes)
    })
}

/// Patrones: `nombre` (cualquier componente), `*.sufijo` (nombre de archivo) o `ruta/relativa`.
fn is_excluded(relative: &str, name: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|pattern| {
        let pattern = pattern.trim_end_matches('/');
        if pattern.contains('/') {
            relative == pattern || relative.starts_with(&format!("{pattern}/"))
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            name.ends_with(suffix)
        } else {
            name == pattern
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusion_patterns() {
        let excludes = [
            "node_modules".to_owned(),
            "*.generado.ts".to_owned(),
            "tests/fixtures".to_owned(),
        ];
        assert!(is_excluded("web/node_modules", "node_modules", &excludes));
        assert!(is_excluded(
            "src/api.generado.ts",
            "api.generado.ts",
            &excludes
        ));
        assert!(is_excluded("tests/fixtures/python", "python", &excludes));
        assert!(is_excluded("tests/fixtures", "fixtures", &excludes));
        assert!(!is_excluded(
            "tests/fixtures_extra",
            "fixtures_extra",
            &excludes
        ));
        assert!(!is_excluded("src/app.py", "app.py", &excludes));
    }

    #[test]
    fn display_paths_use_forward_slashes() {
        let root = Path::new("proyecto");
        assert_eq!(
            display_path(root, &root.join("src").join("app.py")),
            "src/app.py"
        );
        assert_eq!(
            display_path(Path::new("solo.py"), Path::new("solo.py")),
            "solo.py"
        );
    }
}
