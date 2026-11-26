//! El propio repositorio cumple el presupuesto de tamaño que argus impone a otros.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

const MAX_FILE_LINES: usize = 350;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_source_file_exceeds_line_budget() {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty());
    let over: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let lines = fs::read_to_string(path).unwrap().lines().count();
            (lines > MAX_FILE_LINES).then(|| format!("{} ({lines} líneas)", path.display()))
        })
        .collect();
    assert!(
        over.is_empty(),
        "archivos sobre {MAX_FILE_LINES} líneas: {over:?}"
    );
}
