//! Integración mínima con git para el modo incremental: se ejecuta el binario `git` y se lee su salida.
//!
//! Las opciones de cada llamada fijan el formato (prefijos, color, renombres, rutas sin escapar) para
//! no depender de la configuración del usuario.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("no se pudo ejecutar git: {0}")]
    Spawn(std::io::Error),
    #[error("`git {args}` falló: {stderr}")]
    Failed { args: String, stderr: String },
}

/// Un archivo distinto en el árbol de trabajo respecto a la base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Ruta actual, relativa a la raíz del repositorio y con `/`.
    pub path: String,
    /// Ruta en la base (distinta si se renombró); `None` si el archivo es nuevo.
    pub base_path: Option<String>,
    /// Líneas añadidas o modificadas (rangos inclusivos); `None` si todo el archivo es nuevo.
    pub lines: Option<Vec<(u32, u32)>>,
}

pub struct Repo {
    root: PathBuf,
}

impl Repo {
    /// Repositorio que contiene `dir`.
    pub fn discover(dir: &Path) -> Result<Self, GitError> {
        let root = run(dir, &["rev-parse", "--show-toplevel"])?;
        Ok(Self {
            root: PathBuf::from(root.trim()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Commit desde el que se mide el cambio: el ancestro común de `rev` y `HEAD`. Con `HEAD` es el
    /// propio `HEAD`; con `main` es donde la rama actual se separó de `main`.
    pub fn merge_base(&self, rev: &str) -> Result<String, GitError> {
        Ok(run(&self.root, &["merge-base", rev, "HEAD"])?.trim().to_owned())
    }

    /// Archivos modificados, renombrados o nuevos (con seguimiento o sin él) desde `base`.
    pub fn changes(&self, base: &str) -> Result<Vec<Change>, GitError> {
        let diff = run(
            &self.root,
            &[
                "-c",
                "core.quotepath=off",
                "diff",
                "-U0",
                "-M",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                base,
            ],
        )?;
        let mut changes = parse_diff(&diff);
        let untracked = run(
            &self.root,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?;
        changes.extend(untracked.split('\0').filter(|p| !p.is_empty()).map(|path| Change {
            path: path.to_owned(),
            base_path: None,
            lines: None,
        }));
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        changes.dedup_by(|a, b| a.path == b.path);
        Ok(changes)
    }

    /// Contenido en `base` de cada ruta, con un solo proceso `git cat-file --batch`. Las rutas que no
    /// existen allí o no son UTF-8 no aparecen en el resultado.
    pub fn read_all(&self, base: &str, paths: &[&str]) -> Result<BTreeMap<String, String>, GitError> {
        if paths.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut child = Command::new("git")
            .args(["cat-file", "--batch"])
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(GitError::Spawn)?;
        let request: String = paths.iter().map(|p| format!("{base}:{p}\n")).collect();
        let mut stdin = child.stdin.take();
        let writer = std::thread::spawn(move || {
            stdin
                .as_mut()
                .map(|pipe| pipe.write_all(request.as_bytes()))
        });
        let mut output = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout.read_to_end(&mut output).map_err(GitError::Spawn)?;
        }
        let _ = writer.join();
        child.wait().map_err(GitError::Spawn)?;
        Ok(paths
            .iter()
            .zip(parse_batch(&output))
            .filter_map(|(path, text)| Some(((*path).to_owned(), text?)))
            .collect())
    }
}

/// Respuestas de `cat-file --batch`, en orden: `<oid> <tipo> <tamaño>\n<contenido>\n` o
/// `<objeto> missing\n`.
fn parse_batch(mut output: &[u8]) -> Vec<Option<String>> {
    let mut texts = Vec::new();
    while let Some(eol) = output.iter().position(|&b| b == b'\n') {
        let header = String::from_utf8_lossy(output.get(..eol).unwrap_or_default()).into_owned();
        output = output.get(eol + 1..).unwrap_or_default();
        let size = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse::<usize>().ok());
        let Some(size) = size.filter(|_| !header.ends_with(" missing")) else {
            texts.push(None);
            continue;
        };
        let body = output.get(..size).unwrap_or_default();
        texts.push(String::from_utf8(body.to_vec()).ok());
        output = output.get(size + 1..).unwrap_or_default();
    }
    texts
}

fn run(dir: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(GitError::Spawn)?;
    if !output.status.success() {
        return Err(GitError::Failed {
            args: args.join(" "),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|err| GitError::Failed {
        args: args.join(" "),
        stderr: format!("salida no UTF-8: {err}"),
    })
}

/// Lee un `git diff -U0`. Las cabeceras solo se interpretan antes del primer `@@` de cada archivo:
/// dentro de un hunk, una línea añadida `++i;` se ve como `+++i;` y no es una cabecera.
fn parse_diff(diff: &str) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut current: Option<Change> = None;
    let mut in_header = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            changes.extend(current.take());
            current = Some(Change {
                path: String::new(),
                base_path: None,
                lines: Some(Vec::new()),
            });
            in_header = true;
            continue;
        }
        let Some(change) = current.as_mut() else {
            continue;
        };
        if let Some(hunk) = line.strip_prefix("@@ ") {
            in_header = false;
            if let (Some(range), Some(lines)) = (added_range(hunk), change.lines.as_mut()) {
                lines.push(range);
            }
        } else if in_header {
            read_header(line, change);
        }
    }
    changes.extend(current);
    changes.retain(|c| !c.path.is_empty());
    changes
}

fn read_header(line: &str, change: &mut Change) {
    if let Some(old) = line.strip_prefix("--- ") {
        change.base_path = old.strip_prefix("a/").map(unquote);
    } else if let Some(new) = line.strip_prefix("+++ ") {
        change.path = new.strip_prefix("b/").map(unquote).unwrap_or_default();
    }
}

fn unquote(path: &str) -> String {
    path.trim_matches('"').to_owned()
}

/// Rango añadido de `-a,b +c,d @@`. Con `d = 0` solo se borró: se marcan las líneas que lo rodean.
fn added_range(hunk: &str) -> Option<(u32, u32)> {
    let plus = hunk.split_whitespace().find_map(|part| part.strip_prefix('+'))?;
    let (start, count) = match plus.split_once(',') {
        Some((start, count)) => (start.parse::<u32>().ok()?, count.parse::<u32>().ok()?),
        None => (plus.parse::<u32>().ok()?, 1),
    };
    if count == 0 {
        return Some((start.max(1), start.saturating_add(1)));
    }
    Some((start, start.saturating_add(count - 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_output_is_split_by_declared_sizes() {
        let output = b"aa blob 6\na\nb\nc\n\nx.py missing\nbb blob 2\n\xff\xfe\ncc blob 0\n\n";
        assert_eq!(
            parse_batch(output),
            [Some("a\nb\nc\n".to_owned()), None, None, Some(String::new())]
        );
    }

    #[test]
    fn diff_headers_hunks_and_renames_are_read() {
        let diff = "\
diff --git a/src/a.ts b/src/a.ts
index 1..2 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -3 +3,2 @@ function f() {
-  x;
+  ++i;
+++j;
@@ -10,2 +11,0 @@
diff --git a/viejo.py b/nuevo.py
similarity index 90%
rename from viejo.py
rename to nuevo.py
--- a/viejo.py
+++ b/nuevo.py
@@ -1 +1 @@
-a = 1
+a = 2
diff --git a/creado.py b/creado.py
new file mode 100644
--- /dev/null
+++ b/creado.py
@@ -0,0 +1,3 @@
diff --git a/borrado.py b/borrado.py
deleted file mode 100644
--- a/borrado.py
+++ /dev/null
@@ -1,2 +0,0 @@
";
        let changes = parse_diff(diff);
        type Row<'a> = (&'a str, Option<&'a str>, Vec<(u32, u32)>);
        let summary: Vec<Row> = changes
            .iter()
            .map(|c| {
                let lines = c.lines.clone().unwrap_or_default();
                (c.path.as_str(), c.base_path.as_deref(), lines)
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("src/a.ts", Some("src/a.ts"), vec![(3, 4), (11, 12)]),
                ("nuevo.py", Some("viejo.py"), vec![(1, 1)]),
                ("creado.py", None, vec![(1, 3)]),
            ]
        );
    }
}
