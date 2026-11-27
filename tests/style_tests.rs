#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::path::Path;

use argus_reviewer::arena::Language;
use argus_reviewer::config::Config;
use argus_reviewer::lexer::lex_file;
use argus_reviewer::reporters::{Diagnostic, Severity};
use argus_reviewer::rules::{check, FileContext};

fn diagnostics(file: &str, source: &str, config: &Config) -> Vec<Diagnostic> {
    let path = Path::new(file);
    let language = Language::from_path(path).unwrap();
    let lexed = lex_file(source, language, path).unwrap();
    let ctx = FileContext::new(file, source, &lexed, language, config);
    check(&ctx)
}

/// `(id, línea)` de los diagnósticos de las reglas cuyo ID empieza por `prefix`.
fn found(file: &str, source: &str, prefix: &str, config: &Config) -> Vec<(&'static str, u32)> {
    diagnostics(file, source, config)
        .iter()
        .filter(|d| d.id.starts_with(prefix))
        .map(|d| (d.id, d.line_start))
        .collect()
}

fn config(text: &str) -> Config {
    Config::parse(text, Path::new("t")).unwrap()
}

#[test]
fn python_idioms_follow_pep8() {
    let src = "\
if x == None or None != y:
    pass
if x is None:
    pass
f = lambda: 1
d[k] = lambda: 1
ordenar(key=lambda v: v)
class A:
    g = lambda self: 2
";
    assert_eq!(
        found("a.py", src, "ARGUS-IDIOM", &Config::default()),
        [
            ("ARGUS-IDIOM-001", 1),
            ("ARGUS-IDIOM-001", 1),
            ("ARGUS-IDIOM-005", 5),
            ("ARGUS-IDIOM-005", 9),
        ]
    );
}

#[test]
fn js_idioms_allow_nullish_loose_equality_and_ambient_var() {
    let src = "\
if (a == b || a != null || undefined == a || a === b) {}
var x = 1;
declare var y: number;
debugger;
o.debugger;
";
    assert_eq!(
        found("a.ts", src, "ARGUS-IDIOM", &Config::default()),
        [
            ("ARGUS-IDIOM-002", 1),
            ("ARGUS-IDIOM-003", 2),
            ("ARGUS-IDIOM-004", 4),
        ]
    );
    assert!(found(
        "tipos.d.ts",
        "var z: number;\n",
        "ARGUS-IDIOM",
        &Config::default()
    )
    .is_empty());
}

fn functions(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| format!("export function {n}() {{ return 1; }}\n"))
        .collect()
}

#[test]
fn naming_follows_the_file_convention_or_the_configured_one() {
    let camel = ["leerUno", "leerDos", "leerTres", "leerCuatro", "leerCinco"];
    let mut names = camel.to_vec();
    names.extend(["leer_seis", "Componente", "run"]);
    let src = functions(&names);
    let found_default = diagnostics("a.ts", &src, &Config::default());
    let style: Vec<&Diagnostic> = found_default
        .iter()
        .filter(|d| d.id == "ARGUS-STYLE-001")
        .collect();
    assert_eq!(style.len(), 1);
    assert_eq!(style[0].line_start, 6);
    assert_eq!(style[0].severity, Severity::Suggestion);
    let action = style[0].agent_action.as_ref().unwrap();
    assert_eq!(action.target_symbol.as_deref(), Some("leer_seis"));
    assert!(action.instruction.contains("`leerSeis`"));

    let few = functions(&["leerUno", "leerDos", "leer_tres"]);
    assert!(found("a.ts", &few, "ARGUS-STYLE", &Config::default()).is_empty());

    let snake = config("[style]\njs_function_naming = \"snake_case\"\n");
    assert_eq!(found("a.ts", &few, "ARGUS-STYLE", &snake).len(), 2);
    let off = config("[style]\njs_function_naming = \"off\"\n");
    assert!(found("a.ts", &src, "ARGUS-STYLE", &off).is_empty());
}

#[test]
fn rule_levels_turn_rules_off_or_change_their_severity() {
    let src = "debugger;\nif (a == b) {}\n";
    let tuned = config("[rules]\n\"ARGUS-IDIOM-004\" = \"off\"\n\"ARGUS-IDIOM-002\" = \"error\"\n");
    let found = diagnostics("a.js", src, &tuned);
    let summary: Vec<(&str, Severity)> = found.iter().map(|d| (d.id, d.severity)).collect();
    assert_eq!(summary, [("ARGUS-IDIOM-002", Severity::Error)]);
}

/// `(primera línea, última línea, instrucción)` de cada sugerencia de bucle acumulador.
fn rewrites(file: &str, source: &str) -> Vec<(u32, u32, String)> {
    diagnostics(file, source, &Config::default())
        .into_iter()
        .filter(|d| d.id == "ARGUS-STYLE-002")
        .map(|d| {
            (
                d.line_start,
                d.line_end,
                d.agent_action.unwrap().instruction,
            )
        })
        .collect()
}

fn instruction(from: u32, to: u32, code: &str) -> String {
    format!("Sustituir las líneas {from}-{to} por `{code}`.")
}

#[test]
fn python_accumulator_loops_become_comprehensions_only_when_safe() {
    let safe = "\
r = []
for x in xs:
    r.append(x * 2)
s = set()
for x in xs:
    if x > 0:
        s.add(x)
m = {}
for k, v in d.items():
    m[k] = v.strip()
copia: list[int] = []
for x in xs: copia.append(x)
";
    assert_eq!(
        rewrites("a.py", safe),
        [
            (1, 3, instruction(1, 3, "r = [x * 2 for x in xs]")),
            (4, 7, instruction(4, 7, "s = {x for x in xs if x > 0}")),
            (
                8,
                10,
                instruction(8, 10, "m = {k: v.strip() for k, v in d.items()}")
            ),
            (11, 12, instruction(11, 12, "copia: list[int] = list(xs)")),
        ]
    );
    let unsafe_loops = "\
a = []
for x in xs:
    a.append(len(a))
b = []
for y in xs:
    b.append(y)
print(y)
c = []
for x in xs:
    if x:
        c.append(x)
    else:
        pass
e = []
for x in xs:
    e.append(x)
else:
    pass
f = []
for x in xs:
    f.append(x)
    log(x)
";
    assert!(rewrites("a.py", unsafe_loops).is_empty());
}

#[test]
fn js_accumulator_loops_become_map_and_filter_only_when_safe() {
    let safe = "\
const r: number[] = [];
for (const x of xs) {
  r.push(x * 2);
}
const pares = [];
for (const x of xs) if (x % 2 === 0) pares.push(x);
const filas = [];
for (const [k, v] of Object.entries(o)) {
  if (v) {
    filas.push({ k, v });
  }
}
";
    let found = rewrites("a.ts", safe);
    let caveat = " Si `xs` no es un array (Set, Map, generador), envolverlo en `Array.from(...)`.";
    assert_eq!(
        found,
        [
            (1, 4, instruction(1, 4, "const r: number[] = xs.map((x) => x * 2);") + caveat),
            (5, 6, instruction(5, 6, "const pares = xs.filter((x) => x % 2 === 0);") + caveat),
            (
                7,
                12,
                instruction(
                    7,
                    12,
                    "const filas = Object.entries(o).filter(([k, v]) => v).map(([k, v]) => ({ k, v }));"
                )
            ),
        ]
    );
    let unsafe_loops = "\
const a = [];
for (const x of xs) { a.push(await cargar(x)); }
const b = [];
for (const x of xs) { b.push(x, x); }
const c = [];
for (const x of xs) { if (x) c.push(x); else c.push(0); }
const e = [];
for (const x of xs) { e.push(e.length); }
";
    assert!(rewrites("a.ts", unsafe_loops).is_empty());
}
