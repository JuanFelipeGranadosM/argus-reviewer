#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use argus_reviewer::config::Config;
use argus_reviewer::reporters::Severity;
use argus_reviewer::scan::scan;
use serde_json::Value;

const SOUP: &str = "\
# Importar modulos
import os

def procesar(payload):
    # Obtener el total
    total = payload['total']
    # Obtener el iva
    iva = payload.get('iva', 0)
    # Calcular el resultado
    resultado = total * (1 + iva)
    # Retornar el resultado
    return resultado
";

fn project(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&root);
    let long: String = (0..450).map(|i| format!("x{i} = {i}\n")).collect();
    let files: [(&str, &str); 8] = [
        ("soup.py", SOUP),
        ("long.py", &long),
        ("clean.py", "def suma(a: int, b: int) -> int:\n    return a + b\n"),
        ("bare.py", "x = f()  # type: ignore\nimport os  # noqa\ny = g()  # type: ignore[call-arg]\n"),
        ("pkg/broken.py", "s = 'sin cerrar\nok = 1\n"),
        ("web/app.ts", "export const a = 1;\n"),
        ("node_modules/dep/index.py", "# x\n# x\n# x\n# x\n# x\n# x\n"),
        (".venv/lib/site.py", "s = 'roto\n"),
    ];
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    root
}

fn argus(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_argus")).args(args).output().unwrap()
}

#[test]
fn scan_reports_each_rule_once_per_file() {
    let root = project("scan_rules");
    let report = scan(&root, &Config::default()).unwrap();
    assert_eq!(report.summary.files_scanned, 5);
    assert_eq!(report.summary.files_unsupported, 1);
    let found: Vec<(&str, &str, u32)> = report
        .diagnostics
        .iter()
        .map(|d| (d.file.as_str(), d.id, d.line_start))
        .collect();
    assert_eq!(
        found,
        [
            ("bare.py", "ARGUS-TYPE-001", 1),
            ("bare.py", "ARGUS-TYPE-001", 2),
            ("long.py", "ARGUS-BLOAT-001", 1),
            ("pkg/broken.py", "ARGUS-SYNTAX-001", 1),
            ("soup.py", "ARGUS-SOUP-001", 1),
        ]
    );
    assert_eq!(report.summary.blocking_issues, 2);
    assert_eq!(report.summary.warnings, 3);
    let soup = report.diagnostics.iter().find(|d| d.file == "soup.py").unwrap();
    assert_eq!(soup.severity, Severity::Warning);
    assert_eq!(soup.agent_action.as_ref().unwrap().lines, [1, 5, 7, 9, 11]);
}

#[test]
fn explanatory_comment_blocks_are_not_soup() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scan_explained");
    fs::create_dir_all(&dir).unwrap();
    let block = "# El proveedor corta los mensajes a 4068 bytes, así que dejamos margen\n\
                 # para el marcador de nivel y no partimos caracteres UTF-8 a la mitad.\n";
    let text: String = (0..8).map(|i| format!("{block}x{i} = {i}\n")).collect();
    let file = dir.join("explained.py");
    fs::write(&file, text).unwrap();
    let report = scan(&file, &Config::default()).unwrap();
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
}

#[test]
fn agent_format_follows_contract_and_exits_one_on_blocking_issues() {
    let root = project("scan_agent");
    let output = argus(&["scan", root.to_str().unwrap(), "--format", "agent"]);
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["summary"]["files_scanned"], 5);
    assert_eq!(json["summary"]["blocking_issues"], 2);
    let first = &json["diagnostics"][0];
    for key in ["id", "severity", "rule", "file", "line_start", "line_end", "message"] {
        assert!(!first[key].is_null(), "falta {key}: {first}");
    }
    assert_eq!(first["severity"], "warning");
    assert_eq!(first["agent_action"]["type"], "add_suppression_code");
    assert!(first["agent_action"]["instruction"].is_string());
}

#[test]
fn output_is_deterministic() {
    let root = project("scan_determinism");
    let run = || argus(&["scan", root.to_str().unwrap(), "--format", "agent"]).stdout;
    assert_eq!(run(), run());
}

#[test]
fn clean_file_exits_zero() {
    let root = project("scan_clean");
    let output = argus(&["scan", root.join("clean.py").to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 archivos analizados"));
}

#[test]
fn tool_failures_exit_two() {
    assert_eq!(argus(&["scan", "no/existe/en/ningun/lado"]).status.code(), Some(2));
    let root = project("scan_bad_config");
    fs::write(root.join(".argus.toml"), "[budgets]\nmax_file_line = 1\n").unwrap();
    let output = argus(&["scan", root.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains(".argus.toml"));
}

#[test]
fn project_config_is_discovered() {
    let root = project("scan_config");
    fs::write(
        root.join(".argus.toml"),
        "[general]\nexclude = [\"pkg\"]\n[budgets]\nmax_file_lines = 500\n",
    )
    .unwrap();
    let report = scan(&root, &Config::discover(&root, None).unwrap()).unwrap();
    assert_eq!(report.summary.blocking_issues, 0);
    assert_eq!(report.summary.files_scanned, 4);
}
