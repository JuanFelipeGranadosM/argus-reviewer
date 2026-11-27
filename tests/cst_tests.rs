#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use argus_reviewer::arena::Language;
use argus_reviewer::cst::{Element, Node};
use argus_reviewer::lexer::lex_file;
use argus_reviewer::parser::{parse, Parsed};

fn parse_as(file: &str, source: &str) -> Parsed {
    let path = Path::new(file);
    let language = Language::from_path(path).unwrap();
    let lexed = lex_file(source, language, path).unwrap();
    parse(source, &lexed, language, path)
}

fn rebuilt(parsed: &Parsed, source: &str) -> String {
    parsed.cst.root().leaves().map(|t| t.text(source)).collect()
}

/// Árbol con un nodo por línea; los tokens significativos van entre comillas y la trivia se omite.
fn dump(node: Node, source: &str, depth: usize, out: &mut String) {
    let _ = writeln!(out, "{}{:?}", "  ".repeat(depth), node.kind());
    for child in node.children() {
        match child {
            Element::Node(n) => dump(n, source, depth + 1, out),
            Element::Token(t) if !t.kind.is_trivia() && !t.range.is_empty() => {
                let _ = writeln!(out, "{}{:?}", "  ".repeat(depth + 1), t.text(source));
            }
            Element::Token(_) => {}
        }
    }
}

fn tree(file: &str, source: &str) -> String {
    let parsed = parse_as(file, source);
    assert_eq!(parsed.errors, [], "{source}");
    let mut out = String::new();
    dump(parsed.cst.root(), source, 0, &mut out);
    out
}

fn fixtures(dir: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    files
}

#[test]
fn fixtures_round_trip_through_the_tree_without_errors() {
    for path in fixtures("python").into_iter().chain(fixtures("js")) {
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = parse_as(path.to_str().unwrap(), &source);
        assert_eq!(rebuilt(&parsed, &source), source, "{}", path.display());
        assert_eq!(parsed.errors, [], "{}", path.display());
    }
}

/// Cada prefijo de cada fixture es código cortado a medias: el parser debe terminar y no perder bytes.
#[test]
fn every_prefix_of_every_fixture_parses_losslessly() {
    for path in fixtures("python").into_iter().chain(fixtures("js")) {
        let source = std::fs::read_to_string(&path).unwrap();
        let name = path.to_str().unwrap();
        let cuts = (0..source.len())
            .step_by(7)
            .filter(|&i| source.is_char_boundary(i));
        for cut in cuts {
            let prefix = &source[..cut];
            assert_eq!(
                rebuilt(&parse_as(name, prefix), prefix),
                prefix,
                "{name}:{cut}"
            );
        }
    }
}

#[test]
fn invalid_python_still_round_trips_and_reports_errors() {
    let broken = [
        "def f(:\n    pass\n",
        "x = (1, 2\ny = 3\n",
        "if x\n    pass\n",
        "  sangrado = 1\n",
        "print 'x'\n",
        "class\n",
        "@\n",
        "a = [1, 2 3]\n",
        "lambda: (yield)\n)\n]\n}\n",
    ];
    for source in broken {
        let parsed = parse_as("a.py", source);
        assert_eq!(rebuilt(&parsed, source), source);
        assert!(!parsed.errors.is_empty(), "{source:?}");
    }
}

/// Árbol esperado en `statements_and_expressions_get_their_nodes`.
const FUNCTION_TREE: &str = "\
SourceFile
  FunctionDef
    Decorator
      \"@\"
      NameExpr
        \"cache\"
      \"\\n\"
    \"async\"
    \"def\"
    \"f\"
    ParamList
      \"(\"
      Param
        \"a\"
        \":\"
        Annotation
          NameExpr
            \"int\"
      \",\"
      Param
        \"/\"
      \",\"
      Param
        \"*\"
        \"args\"
      \",\"
      Param
        \"b\"
        \"=\"
        Literal
          \"1\"
      \",\"
      Param
        \"**\"
        \"kw\"
      \")\"
    ReturnType
      \"->\"
      SubscriptExpr
        NameExpr
          \"list\"
        \"[\"
        NameExpr
          \"int\"
        \"]\"
    \":\"
    Block
      \"\\n\"
      ReturnStmt
        \"return\"
        ListComp
          \"[\"
          BinaryExpr
            NameExpr
              \"x\"
            \"*\"
            Literal
              \"2\"
          CompFor
            \"for\"
            NameExpr
              \"x\"
            \"in\"
            NameExpr
              \"a\"
          CompIf
            \"if\"
            NameExpr
              \"x\"
          \"]\"
      \"\\n\"
";

#[test]
fn statements_and_expressions_get_their_nodes() {
    let source = "\
@cache
async def f(a: int, /, *args, b=1, **kw) -> list[int]:
    return [x * 2 for x in a if x]
";
    assert_eq!(tree("a.py", source), FUNCTION_TREE);
}

#[test]
fn soft_keywords_and_modern_syntax_parse_cleanly() {
    let source = "\
match = re.match(p, s)
match cmd:
    case Point(x=0) | [a, *rest] if a:
        pass
    case {\"k\": v, **r} as m:
        pass
    case (Error() as e, [1 as uno, _]):
        pass
type Par[T] = tuple[T, T]
with (open(a) as f, open(b) as g):
    print(f'{f!r:>{w}}', *g, sep='')
try:
    pass
except* (A, B) as e:
    raise X from e
x = yield
y: int = (z := 3) if not a in b else -c ** 2
del a[1:2, ::3], b.c
";
    let text = tree("a.py", source);
    for kind in [
        "MatchStmt",
        "CaseClause",
        "Guard",
        "TypeAliasStmt",
        "WithItem",
        "ExceptClause",
        "YieldExpr",
        "NamedExpr",
        "ConditionalExpr",
        "Slice",
        "DelStmt",
    ] {
        assert!(text.contains(kind), "falta {kind}:\n{text}");
    }
    assert!(text.starts_with("SourceFile\n  AssignStmt"), "{text}");
}

/// `ARGUS_CST_DIR=<carpeta> cargo test --test cst_tests -- --ignored --nocapture`
#[test]
#[ignore = "necesita ARGUS_CST_DIR con un corpus real"]
fn corpus_round_trips_through_the_tree() {
    let Ok(dir) = std::env::var("ARGUS_CST_DIR") else {
        return;
    };
    let mut files = Vec::new();
    collect(Path::new(&dir), &mut files);
    let started = Instant::now();
    let (mut with_errors, mut bytes) = (0usize, 0usize);
    let mut messages: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for path in &files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let name = path.to_string_lossy().into_owned();
        let parsed = parse_as(&name, &source);
        assert_eq!(rebuilt(&parsed, &source), source, "{name}");
        bytes += source.len();
        if let Some(first) = parsed.errors.first() {
            with_errors += 1;
            let line = source[..first.range.start as usize].matches('\n').count() + 1;
            if std::env::var_os("ARGUS_CST_VERBOSE").is_some() {
                println!("  {name}:{line}: {}", first.message);
            }
            messages
                .entry(first.message)
                .or_default()
                .push(format!("{name}:{line}"));
        }
    }
    println!(
        "{} archivos, {} MB en {:?}; {with_errors} con errores",
        files.len(),
        bytes / 1_000_000,
        started.elapsed()
    );
    for (message, places) in &messages {
        println!("  {:5} {message}", places.len());
        for place in places.iter().take(3) {
            println!("          {place}");
        }
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let skipped = path
            .file_name()
            .is_some_and(|n| n == ".git" || n == "node_modules");
        if path.is_dir() {
            if !skipped || std::env::var_os("ARGUS_CST_NODE_MODULES").is_some() {
                collect(&path, out);
            }
        } else if Language::from_path(&path).is_some() {
            out.push(path);
        }
    }
}

/// Árbol esperado en `javascript_precedence_and_asi_build_the_right_tree`.
const JS_TREE: &str = "SourceFile
  ExprStmt
    AssignExpr
      NameExpr
        \"a\"
      \"=\"
      ConditionalExpr
        NameExpr
          \"b\"
        \"?\"
        NameExpr
          \"c\"
        \":\"
        BinaryExpr
          BinaryExpr
            NameExpr
              \"d\"
            \">\"
            \">\"
            Literal
              \"1\"
          \">\"
          \"=\"
          NameExpr
            \"e\"
  ExprStmt
    PostfixExpr
      NameExpr
        \"x\"
      \"++\"
";

#[test]
fn javascript_precedence_and_asi_build_the_right_tree() {
    assert_eq!(
        tree(
            "a.js",
            "a = b ? c : d >> 1 >= e
x++
"
        ),
        JS_TREE
    );
}

#[test]
fn typescript_declarations_and_types_parse_cleanly() {
    let source = "import type { A } from './a';
export type * from './b';
@Component({ selector: 'x' })
export abstract class C<T extends object = {}> extends Base<T> implements I {
  private readonly x!: Map<string, T[]>;
  static { init(); }
  constructor(private y?: number) { super(); }
  get z(): T | undefined { return this.x.get('k')?.[0]; }
  abstract run<U>(u: U): asserts u is string;
}
interface I extends J<K> { (a: number): void; new (): I; readonly [k: string]: unknown; m?(): void }
type M<T> = { readonly [K in keyof T as `get${K & string}`]-?: () => T[K] };
type R<T> = T extends (...args: any[]) => infer U extends object ? U : never;
declare module 'm' { export const v: unique symbol; }
const enum E { A = 1, B }
namespace N.O { export function f(): void; }
const g = <T,>(x: T): x is T => (x as unknown) satisfies T;
const h = f<string>;
let i = obj!.prop as const;
";
    let text = tree("a.ts", source);
    for kind in [
        "ImportDecl",
        "ExportDecl",
        "Decorator",
        "ClassDef",
        "PropertyDef",
        "MethodDef",
        "InterfaceDecl",
        "TypeAliasStmt",
        "AmbientDecl",
        "ModuleDecl",
        "EnumDecl",
        "ArrowFunction",
        "AsExpr",
        "InstantiationExpr",
        "NonNullExpr",
        "TypeParamList",
    ] {
        assert!(
            text.contains(kind),
            "falta {kind}:
{text}"
        );
    }
}

#[test]
fn jsx_and_regex_parse_cleanly() {
    let source = "export default function App({ items }) {
  const re = /<div>/g, half = total / 2;
  return (
    <>
      <List.Item aria-label=\"x\" {...rest} onClick={() => go(`/a/${id}`)}>
        Don't {items.map((i) => <b key={i}>{i}</b>)}
      </List.Item>
      <input disabled />
    </>
  );
}
";
    let text = tree("a.jsx", source);
    assert_eq!(text.matches("JsxElement").count(), 4, "{text}");
    assert!(
        text.contains("JsxAttribute") && text.contains("TemplateExpr"),
        "{text}"
    );
}
