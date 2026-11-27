//! Lectura de `.argus.toml`. Las secciones que aún no tienen reglas (`clones`, `refactor`) se aceptan
//! y se ignoran; dentro de las demás una clave desconocida es un error. En `typing` y `security` se
//! aceptan todas las claves de la especificación aunque alguna aún no tenga regla.

use std::collections::BTreeMap;
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
    #[error("{path}: `[rules]` nombra una regla que no existe: {id}")]
    UnknownRule { path: PathBuf, id: String },
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub budgets: Budgets,
    pub typing: Typing,
    pub security: Security,
    pub style: Style,
    /// Nivel de cada regla por ID (`"ARGUS-IDIOM-002" = "off"`): cada equipo apaga, suaviza o
    /// endurece las reglas que no encajan con su forma de programar.
    pub rules: BTreeMap<String, Level>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Off,
    Suggestion,
    Warning,
    Error,
}

/// Convención de nombres de funciones. `auto` la deduce de cada archivo (la consistencia dentro de un
/// módulo es lo que más importa, PEP 8); una convención explícita se exige en todo el proyecto.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub enum Naming {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "snake_case")]
    SnakeCase,
    #[serde(rename = "camelCase")]
    CamelCase,
    #[serde(rename = "off")]
    Off,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub python_function_naming: Naming,
    pub js_function_naming: Naming,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Typing {
    pub require_python_param_types: bool,
    pub require_python_return_types: bool,
    pub forbid_ts_explicit_any: bool,
    pub check_unawaited_async: bool,
    pub infer_structural_shapes: bool,
}

impl Default for Typing {
    fn default() -> Self {
        Self {
            require_python_param_types: true,
            require_python_return_types: true,
            forbid_ts_explicit_any: true,
            check_unawaited_async: true,
            infer_structural_shapes: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Security {
    pub forbid_empty_catch: bool,
    pub forbid_interpolated_sql: bool,
    pub forbid_dynamic_eval: bool,
}

impl Default for Security {
    fn default() -> Self {
        Self {
            forbid_empty_catch: true,
            forbid_interpolated_sql: true,
            forbid_dynamic_eval: true,
        }
    }
}

impl Config {
    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|source| ConfigError::Invalid {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        match config.rules.keys().find(|id| !crate::rules::is_known(id)) {
            Some(id) => Err(ConfigError::UnknownRule {
                path: path.to_path_buf(),
                id: id.clone(),
            }),
            None => Ok(config),
        }
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
    fn typing_and_security_switches_are_read() {
        let text = "[typing]
forbid_ts_explicit_any = false
[security]
forbid_dynamic_eval = false
";
        let config = Config::parse(text, Path::new("t")).unwrap();
        assert!(!config.typing.forbid_ts_explicit_any);
        assert!(config.typing.require_python_param_types);
        assert!(!config.security.forbid_dynamic_eval);
        assert!(config.security.forbid_empty_catch);
    }

    #[test]
    fn rule_levels_and_style_are_read_and_unknown_ids_rejected() {
        let text = "[rules]\n\"ARGUS-IDIOM-002\" = \"off\"\n[style]\njs_function_naming = \"camelCase\"\n";
        let config = Config::parse(text, Path::new("t")).unwrap();
        assert_eq!(config.rules.get("ARGUS-IDIOM-002"), Some(&Level::Off));
        assert_eq!(config.style.js_function_naming, Naming::CamelCase);
        assert_eq!(config.style.python_function_naming, Naming::Auto);
        let typo = Config::parse("[rules]\n\"ARGUS-IDIOM-999\" = \"off\"\n", Path::new("t"));
        assert!(matches!(typo, Err(ConfigError::UnknownRule { .. })));
    }

    #[test]
    fn typo_in_budgets_is_rejected() {
        let err = Config::parse("[budgets]\nmax_file_line = 10\n", Path::new("t"));
        assert!(matches!(err, Err(ConfigError::Invalid { .. })));
    }
}
