//! Lectura de `.argus.toml`. Las secciones que aún no tienen reglas (`clones`, `typing`, `security`,
//! `refactor`) se aceptan y se ignoran; dentro de `general` y `budgets` una clave desconocida es un error.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const CONFIG_FILE: &str = ".argus.toml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("no se pudo leer {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} no es válido: {source}")]
    Invalid {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub budgets: Budgets,
}

/// Siempre excluidos, se configure lo que se configure: analizar dependencias o artefactos de build
/// solo produce ruido. Las entradas ocultas (`.git`, `.venv`, ...) también se omiten siempre.
pub const BUILTIN_EXCLUDES: &[&str] = &[
    "node_modules",
    "venv",
    "dist",
    "build",
    "__pycache__",
    "target",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub languages: Vec<String>,
    /// Exclusiones adicionales a [`BUILTIN_EXCLUDES`].
    pub exclude: Vec<String>,
    pub entrypoints: Vec<String>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            languages: vec!["python".into(), "javascript".into(), "typescript".into()],
            exclude: Vec::new(),
            entrypoints: Vec::new(),
        }
    }
}

impl General {
    pub fn all_excludes(&self) -> Vec<String> {
        BUILTIN_EXCLUDES
            .iter()
            .map(|e| (*e).to_owned())
            .chain(self.exclude.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budgets {
    pub max_file_lines: u32,
    pub max_function_lines: u32,
    pub max_parameters: u32,
    pub max_cyclomatic_complexity: u32,
    pub max_comment_to_code_ratio: f64,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            max_file_lines: 400,
            max_function_lines: 65,
            max_parameters: 5,
            max_cyclomatic_complexity: 12,
            max_comment_to_code_ratio: 0.25,
        }
    }
}

impl Config {
    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        toml::from_str(text).map_err(|source| ConfigError::Invalid {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text, path)
    }

    /// `explicit` tiene prioridad; si no, se usa `<root>/.argus.toml` cuando existe; si no, los valores por defecto.
    pub fn discover(root: &Path, explicit: Option<&Path>) -> Result<Self, ConfigError> {
        if let Some(path) = explicit {
            return Self::load(path);
        }
        let candidate = root.join(CONFIG_FILE);
        if root.is_dir() && candidate.is_file() {
            return Self::load(&candidate);
        }
        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_take_defaults() {
        let config = Config::parse("[budgets]\nmax_file_lines = 300\n", Path::new("t")).unwrap();
        assert_eq!(config.budgets.max_file_lines, 300);
        assert_eq!(config.budgets.max_function_lines, 65);
    }

    #[test]
    fn user_excludes_add_to_builtin_ones() {
        let config = Config::parse("[general]\nexclude = [\"pkg\"]\n", Path::new("t")).unwrap();
        let all = config.general.all_excludes();
        assert!(all.iter().any(|e| e == "node_modules"));
        assert!(all.iter().any(|e| e == "pkg"));
    }

    #[test]
    fn spec_example_sections_are_accepted() {
        let text = "[clones]\nenabled = true\n[typing]\nforbid_ts_explicit_any = true\n";
        assert!(Config::parse(text, Path::new("t")).is_ok());
    }

    #[test]
    fn typo_in_budgets_is_rejected() {
        let err = Config::parse("[budgets]\nmax_file_line = 10\n", Path::new("t"));
        assert!(matches!(err, Err(ConfigError::Invalid { .. })));
    }
}
