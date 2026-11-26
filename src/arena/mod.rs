//! Memoria plana del proyecto: todo el modelo semántico vive en vectores indexados por IDs `u32`.
//!
//! Invariante: ninguna estructura guarda referencias a otra; solo IDs. Así el modelo no tiene ciclos
//! de propiedad y puede construirse en paralelo y fusionarse.

mod interner;

use std::path::{Path, PathBuf};

pub use interner::{NameId, StringInterner};

macro_rules! define_ids {
    ($($name:ident),* $(,)?) => {
        $(
            #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
            pub struct $name(pub u32);
        )*
    };
}

define_ids!(FileId, NodeId, ScopeId, SymbolId, TypeId, ShapeId);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ArenaError {
    #[error("la arena superó la capacidad de IDs u32 para {0}")]
    Full(&'static str),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "py" | "pyi" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct FileRecord {
    pub path: PathBuf,
    pub language: Language,
    pub text: String,
}

/// Raíz del modelo semántico. Las tablas de ámbitos, símbolos, tipos y grafos se añaden en sus fases.
#[derive(Debug, Default)]
pub struct ProjectArena {
    pub files: Vec<FileRecord>,
    pub names: StringInterner,
}

impl ProjectArena {
    pub fn add_file(&mut self, record: FileRecord) -> Result<FileId, ArenaError> {
        let id = FileId(u32::try_from(self.files.len()).map_err(|_| ArenaError::Full("files"))?);
        self.files.push(record);
        Ok(id)
    }

    pub fn file(&self, id: FileId) -> Option<&FileRecord> {
        self.files.get(id.0 as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_detection_by_extension() {
        assert_eq!(
            Language::from_path(Path::new("a/b.py")),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_path(Path::new("x.tsx")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_path(Path::new("x.jsx")),
            Some(Language::JavaScript)
        );
        assert_eq!(Language::from_path(Path::new("README.md")), None);
    }

    #[test]
    fn files_get_sequential_ids() {
        let mut arena = ProjectArena::default();
        let record = |name: &str| FileRecord {
            path: PathBuf::from(name),
            language: Language::Python,
            text: String::new(),
        };
        assert_eq!(arena.add_file(record("a.py")), Ok(FileId(0)));
        assert_eq!(arena.add_file(record("b.py")), Ok(FileId(1)));
        assert_eq!(
            arena.file(FileId(1)).map(|f| f.path.as_path()),
            Some(Path::new("b.py"))
        );
    }
}
