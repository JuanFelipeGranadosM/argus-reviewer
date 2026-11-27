#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const LONG_LINES: usize = 70;

fn long_function(extra: &str) -> String {
    let body: String = (0..LONG_LINES)
        .map(|i| format!("  const v{i} = {i};\n"))
        .collect();
    format!("function larga() {{\n  const v = 0;\n{extra}{body}}}\n")
}

const GUARD: &str = "export function guarda() {\n  try { a(); } catch {}\n}\n";

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=argus",
            "-c",
            "user.email=argus@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Repositorio con un commit en `main`: una función larga y una captura vacía que ya existían.
fn repo(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    write(
        &root,
        "src/app.ts",
        &format!("{}{GUARD}", long_function("")),
    );
    write(
        &root,
        "src/ok.py",
        "def suma(a: int, b: int) -> int:\n    return a + b\n",
    );
    git(&root, &["add", "."]);
    git(&root, &["commit", "-q", "-m", "base"]);
    root
}

/// El agente desplaza todo con un import, alarga la función larga, añade otra captura vacía y crea
/// un archivo con `eval`. También aparecen archivos que no deben auditarse.
fn agent_edits(root: &Path) {
    let app = format!(
        "import x from 'y';\n{}{GUARD}export function otra() {{\n  try {{ b(); }} catch {{}}\n}}\n",
        long_function("  const extra = 1;\n")
    );
    write(root, "src/app.ts", &app);
    write(
        root,
        "src/nuevo.ts",
        "export const f = (x: string) => eval(x);\n",
    );
    write(root, "node_modules/dep/index.ts", "eval('1');\n");
    write(root, ".cache/tmp.ts", "eval('1');\n");
}

fn argus_diff(root: &Path, extra: &[&str]) -> (Option<i32>, Value) {
    let output: Output = Command::new(env!("CARGO_BIN_EXE_argus"))
        .arg("diff")
        .arg(root)
        .args(["--format", "agent"])
        .args(extra)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("{err}: {stderr}"))
        .unwrap();
    (output.status.code(), json)
}

fn found(json: &Value) -> Vec<(String, String, u64)> {
    json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let file = d["file"].as_str().unwrap().to_owned();
            let id = d["id"].as_str().unwrap().to_owned();
            (file, id, d["line_start"].as_u64().unwrap())
        })
        .collect()
}

fn owned(items: &[(&str, &str, u64)]) -> Vec<(String, String, u64)> {
    items
        .iter()
        .map(|&(f, i, l)| (f.to_owned(), i.to_owned(), l))
        .collect()
}

#[test]
fn only_new_or_worse_problems_are_listed() {
    let root = repo("diff_new");
    agent_edits(&root);
    let (code, json) = argus_diff(&root, &[]);
    assert_eq!(code, Some(1));
    assert_eq!(
        found(&json),
        owned(&[
            ("src/app.ts", "ARGUS-BLOAT-002", 2),
            ("src/app.ts", "ARGUS-SEC-001", 80),
            ("src/nuevo.ts", "ARGUS-SEC-002", 1),
        ])
    );
    let diff = &json["summary"]["incremental"];
    assert_eq!(diff["base"], "HEAD");
    assert_eq!(diff["files_changed"], 2);
    assert_eq!(diff["preexisting_issues"], 1);
    assert_eq!(json["summary"]["files_scanned"], 2);
}

#[test]
fn shifting_code_does_not_report_old_problems() {
    let root = repo("diff_shift");
    let app = fs::read_to_string(root.join("src/app.ts")).unwrap();
    write(&root, "src/app.ts", &format!("// cabecera\n\n{app}"));
    let (code, json) = argus_diff(&root, &[]);
    assert_eq!(code, Some(0), "{json}");
    assert!(found(&json).is_empty());
    assert_eq!(json["summary"]["incremental"]["preexisting_issues"], 2);
}

#[test]
fn a_clean_tree_reports_nothing() {
    let root = repo("diff_clean");
    let (code, json) = argus_diff(&root, &[]);
    assert_eq!(code, Some(0));
    assert_eq!(json["summary"]["incremental"]["files_changed"], 0);
    assert!(found(&json).is_empty());
}

#[test]
fn base_branch_covers_every_commit_of_the_branch() {
    let root = repo("diff_branch");
    git(&root, &["switch", "-q", "-c", "feature"]);
    agent_edits(&root);
    fs::remove_dir_all(root.join("node_modules")).unwrap();
    fs::remove_dir_all(root.join(".cache")).unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-q", "-m", "cambios"]);
    assert!(found(&argus_diff(&root, &[]).1).is_empty());
    let (code, json) = argus_diff(&root, &["--base", "main"]);
    assert_eq!(code, Some(1));
    assert_eq!(found(&json).len(), 3);
    assert_eq!(json["summary"]["incremental"]["base"], "main");
}

#[test]
fn outside_a_repository_is_a_tool_failure() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("diff_no_repo");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_argus"))
        .arg("diff")
        .arg(&dir)
        .env("GIT_CEILING_DIRECTORIES", dir.parent().unwrap())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
