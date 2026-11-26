#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::path::Path;

use argus_reviewer::arena::Language;
use argus_reviewer::config::Config;
use argus_reviewer::lexer::lex_file;
use argus_reviewer::rules::{check, FileContext};
use argus_reviewer::structure::{outline, Function, Outline};

/// `(nombre, [(parámetro, anotado)], tiene retorno, complejidad, anidada)`.
type PyShape = (String, Vec<(String, bool)>, bool, u32, bool);

fn outline_of(file: &str, source: &str) -> Outline {
    let path = Path::new(file);
    let language = Language::from_path(path).unwrap();
    let lexed = lex_file(source, language, path).unwrap();
    outline(source, &lexed, language)
}

/// `(nombre, nº de parámetros, complejidad)` de cada función, en orden de aparición.
fn summary(file: &str, source: &str) -> Vec<(String, usize, u32)> {
    outline_of(file, source)
        .functions
        .iter()
        .map(|f: &Function| {
            let name = f.name.clone().unwrap_or_else(|| "<anónima>".into());
            (name, f.params.len(), f.complexity)
        })
        .collect()
}

fn rule_ids(file: &str, source: &str, config: &Config) -> Vec<&'static str> {
    let path = Path::new(file);
    let language = Language::from_path(path).unwrap();
    let lexed = lex_file(source, language, path).unwrap();
    let ctx = FileContext::new(file, source, &lexed, language, config);
    check(&ctx).iter().map(|d| d.id).collect()
}

fn owned(items: &[(&str, usize, u32)]) -> Vec<(String, usize, u32)> {
    items
        .iter()
        .map(|&(n, p, c)| (n.to_owned(), p, c))
        .collect()
}

#[test]
fn js_function_forms_are_recognized_with_their_names() {
    let src = "\
function suma(a, b) { return a + b; }
export default function (x) { return x; }
const doble = (n: number): number => n * 2;
const triple = async n => n * 3;
const crear = function* (a) { yield a; };
const generico = <T,>(v: T): T => v;
class Caja {
  constructor(private a: string, readonly b: number) {}
  get valor(): { a: number } { return { a: 1 }; }
  async #oculto(x = () => 1) { await x(); }
}
const obj = { metodo(a) { return a; }, flecha: (b) => b };
";
    assert_eq!(
        summary("a.tsx", src),
        owned(&[
            ("suma", 2, 1),
            ("<anónima>", 1, 1),
            ("doble", 1, 1),
            ("triple", 1, 1),
            ("crear", 1, 1),
            ("generico", 1, 1),
            ("constructor", 2, 1),
            ("valor", 0, 1),
            ("#oculto", 1, 1),
            ("x", 0, 1),
            ("metodo", 1, 1),
            ("flecha", 1, 1),
        ])
    );
}

#[test]
fn calls_control_statements_and_signatures_are_not_functions() {
    let src = "\
if (a) { b(); }
while (x(1)) {}
for (const k of ks) {}
switch (v) { case 1: break; }
try { f(); } catch (e) { g(e); }
foo(a, b);
useEffect(() => { go(); }, []);
interface I { m(a: string): void; n?: (x: number) => void }
declare function f(a: string): void;
";
    let names: Vec<String> = summary("a.ts", src)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();
    assert_eq!(names, ["<anónima>", "<anónima>"]);
}

#[test]
fn complexity_counts_own_decisions_only() {
    let src = "\
function f(a, b) {
  if (a && b) { return 1; } else if (a || b) { return 2; }
  for (const x of a) { while (x) {} }
  const t = a ? 1 : 2;
  const v = a ?? b;
  const g = () => { if (a) {} if (b) {} };
  switch (a) { case 1: case 2: break; default: }
  try {} catch {}
  const o: { k?: number } = {};
  return a?.b;
}
";
    assert_eq!(summary("a.ts", src), owned(&[("f", 2, 12), ("g", 0, 3)]));
}

#[test]
fn python_functions_skip_self_and_separators() {
    let src = "\
class A:
    def m(self, a: int, *, b, **kw) -> int:
        if a and b:
            return 1
        elif a or b:
            return 2
        return [x for x in kw if x]

    async def n(cls, /, c):
        def interna(d):
            pass
        return c

def libre(x=lambda y: y, z: dict = {1: 2}): ...
";
    let outline = outline_of("a.py", src);
    let shape: Vec<PyShape> = outline
        .functions
        .iter()
        .map(|f| {
            let params = f
                .params
                .iter()
                .map(|p| (p.name.clone(), p.annotated))
                .collect();
            let name = f.name.clone().unwrap();
            (name, params, f.has_return_type, f.complexity, f.nested)
        })
        .collect();
    let p = |n: &str, a: bool| (n.to_owned(), a);
    assert_eq!(
        shape,
        [
            (
                "m".into(),
                vec![p("a", true), p("b", false), p("kw", false)],
                true,
                7,
                false
            ),
            ("n".into(), vec![p("c", false)], false, 1, false),
            ("interna".into(), vec![p("d", false)], false, 1, true),
            (
                "libre".into(),
                vec![p("x", false), p("z", true)],
                false,
                1,
                false
            ),
        ]
    );
}

#[test]
fn silent_catches_are_detected_but_documented_ones_are_not() {
    let js = "\
try { a(); } catch {}
try { a(); } catch (e) {}
try { a(); } catch { /* sin conexión: se reintenta */ }
try { a(); } catch (e) { log(e); }
p.catch(() => {});
p.catch(function () {});
p.catch((e) => console.error(e));
";
    let silent: Vec<bool> = outline_of("a.js", js)
        .catches
        .iter()
        .map(|c| c.silent)
        .collect();
    assert_eq!(silent, [true, true, false, false, true, true, false]);
    let py = "\
try:
    a()
except:
    pass
try:
    a()
except Exception as e: pass
try:
    a()
except Exception:
    # el módulo es opcional
    pass
try:
    a()
except ImportError:
    pass
try:
    a()
except Exception:
    log()
";
    let silent: Vec<bool> = outline_of("a.py", py)
        .catches
        .iter()
        .map(|c| c.silent)
        .collect();
    assert_eq!(silent, [true, true, false, false, false]);
}

#[test]
fn explicit_any_is_only_the_type_position_in_typescript() {
    let src = "\
let a: any = 1;
const b = c as any;
type T = Array<any> | any[];
function f(x: Record<string, any>): any {}
const o = { any: 1 };
const g = (p, any) => p;
o.any;
";
    let lines: Vec<u32> = outline_of("a.ts", src)
        .explicit_any
        .iter()
        .map(|r| src[..r.start as usize].matches('\n').count() as u32 + 1)
        .collect();
    assert_eq!(lines, [1, 2, 3, 3, 4, 4]);
    assert!(outline_of("a.js", src).explicit_any.is_empty());
}

#[test]
fn dynamic_eval_is_detected_in_both_languages() {
    let js = "eval(code); new Function('a', code); obj.eval(x); const evaluar = 1;";
    assert_eq!(outline_of("a.js", js).dynamic_evals.len(), 2);
    let py = "eval(s)\nexec(s)\nself.eval(s)\ndef eval(x):\n    pass\n";
    assert_eq!(outline_of("a.py", py).dynamic_evals.len(), 2);
}

#[test]
fn function_length_counts_own_lines_not_nested_callbacks() {
    let test: String = (0..20)
        .map(|i| format!("  it('caso {i}', () => {{\n    a();\n    b();\n    c();\n  }});\n"))
        .collect();
    let src = format!("describe('grupo', () => {{\n{test}}});\n");
    let ids = rule_ids("a.test.ts", &src, &Config::default());
    assert!(!ids.contains(&"ARGUS-BLOAT-002"), "{ids:?}");
    let body: String = (0..70).map(|i| format!("  const v{i} = {i};\n")).collect();
    let long = format!("function larga() {{\n{body}}}\n");
    assert!(rule_ids("a.ts", &long, &Config::default()).contains(&"ARGUS-BLOAT-002"));
}

#[test]
fn rules_respect_budgets_and_switches() {
    let src = "function f(a, b, c, d, e, g) { eval(a); try {} catch {} }\nlet x: any;\n";
    let mut ids = rule_ids("a.ts", src, &Config::default());
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "ARGUS-BLOAT-003",
            "ARGUS-SEC-001",
            "ARGUS-SEC-002",
            "ARGUS-TYPE-002"
        ]
    );
    let relaxed = Config::parse(
        "[budgets]\nmax_parameters = 6\n[typing]\nforbid_ts_explicit_any = false\n\
         [security]\nforbid_empty_catch = false\nforbid_dynamic_eval = false\n",
        Path::new("t"),
    )
    .unwrap();
    assert!(rule_ids("a.ts", src, &relaxed).is_empty());
}

#[test]
fn missing_annotations_cover_public_boundaries_only() {
    let src = "\
def publica(a, b: int):
    def interna(x):
        pass
    return b

def _privada(a):
    pass

def anotada(a: int) -> int:
    return a

def test_algo():
    pass

class C:
    def __init__(self, a: int):
        self.a = a

    def __repr__(self):
        return ''
";
    let path = Path::new("m.py");
    let lexed = lex_file(src, Language::Python, path).unwrap();
    let config = Config::default();
    let ctx = FileContext::new("m.py", src, &lexed, Language::Python, &config);
    let diagnostics = check(&ctx);
    let missing = diagnostics
        .iter()
        .find(|d| d.id == "ARGUS-TYPE-003")
        .unwrap();
    assert_eq!(missing.agent_action.as_ref().unwrap().lines, [1, 19]);
    assert!(missing.message.contains("`publica`, `__repr__`"));
}
