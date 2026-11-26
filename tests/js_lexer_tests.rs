#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};

use argus_reviewer::lexer::comments::{classify_js, CommentKind};
use argus_reviewer::lexer::javascript::{self, Dialect};
use argus_reviewer::lexer::{LexErrorKind, Lexed};
use argus_reviewer::syntax::SyntaxKind::{self, *};

const TS: Dialect = Dialect { jsx: false };
const TSX: Dialect = Dialect { jsx: true };

fn assert_lossless(source: &str, lexed: &Lexed, label: &str) {
    let mut next = 0u32;
    let mut rebuilt = std::string::String::with_capacity(source.len());
    for token in &lexed.tokens {
        assert_eq!(
            token.range.start, next,
            "{label}: hueco o solape en {token:?}"
        );
        next = token.range.end;
        rebuilt.push_str(token.text(source));
    }
    assert_eq!(next as usize, source.len(), "{label}: no llega al final");
    assert_eq!(rebuilt, source, "{label}: el round-trip no es idéntico");
    assert_eq!(
        lexed.tokens.last().map(|t| t.kind),
        Some(EndOfFile),
        "{label}"
    );
}

fn lex(source: &str, dialect: Dialect) -> Lexed {
    let lexed = javascript::lex(source, dialect).unwrap();
    assert_lossless(source, &lexed, source);
    lexed
}

fn tokens(source: &str, dialect: Dialect) -> Vec<(SyntaxKind, std::string::String)> {
    lex(source, dialect)
        .tokens
        .iter()
        .filter(|t| !t.kind.is_trivia() && t.kind != EndOfFile)
        .map(|t| (t.kind, t.text(source).to_owned()))
        .collect()
}

fn kinds(source: &str, dialect: Dialect) -> Vec<SyntaxKind> {
    tokens(source, dialect).into_iter().map(|(k, _)| k).collect()
}

fn texts_of(source: &str, dialect: Dialect, kind: SyntaxKind) -> Vec<std::string::String> {
    tokens(source, dialect)
        .into_iter()
        .filter(|(k, _)| *k == kind)
        .map(|(_, t)| t)
        .collect()
}

fn error_kinds(source: &str, dialect: Dialect) -> Vec<LexErrorKind> {
    lex(source, dialect).errors.iter().map(|e| e.kind).collect()
}

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/js");
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert!(files.len() >= 3);
    files
}

#[test]
fn fixtures_round_trip_without_errors() {
    for path in fixtures() {
        let source = fs::read_to_string(&path).unwrap();
        let lexed = javascript::lex(&source, Dialect::from_path(&path)).unwrap();
        assert_lossless(&source, &lexed, &path.display().to_string());
        assert!(
            lexed.errors.is_empty(),
            "{}: {:?}",
            path.display(),
            lexed.errors
        );
    }
}

#[test]
fn every_prefix_of_every_fixture_is_lossless() {
    for path in fixtures() {
        let source = fs::read_to_string(&path).unwrap();
        let dialect = Dialect::from_path(&path);
        for (end, _) in source.char_indices() {
            let prefix = &source[..end];
            let lexed = javascript::lex(prefix, dialect).unwrap();
            assert_lossless(prefix, &lexed, &format!("{}[..{end}]", path.display()));
        }
    }
}

#[test]
fn dialect_follows_extension() {
    for (file, jsx) in [
        ("a.ts", false),
        ("a.mts", false),
        ("a.cts", false),
        ("a.tsx", true),
        ("a.js", true),
        ("a.jsx", true),
        ("a.mjs", true),
    ] {
        assert_eq!(Dialect::from_path(Path::new(file)).jsx, jsx, "{file}");
    }
}

#[test]
fn slash_is_regex_or_division_by_context() {
    let src = "a / b / c; x = /re/g; f(/a/); (a) / 2; [a][0] / 2; i++ / 2; \
               if (x) /b/.test(y); while (x) /d/; return /c/; o.default / 2";
    assert_eq!(
        texts_of(src, TS, Regex),
        ["/re/g", "/a/", "/b/", "/d/", "/c/"]
    );
    assert_eq!(texts_of(src, TS, Slash).len(), 6);
}

#[test]
fn regex_classes_and_escapes_do_not_end_the_literal() {
    assert_eq!(
        texts_of(r"x = /[/\]]+\/a/u;", TS, Regex),
        [r"/[/\]]+\/a/u"]
    );
}

#[test]
fn templates_nest_and_resume_after_braces() {
    let src = "`a${`b${c}`}d${ { x: 1 }.x }e`";
    assert_eq!(
        kinds(src, TS),
        [
            TemplateHead,
            TemplateHead,
            Ident,
            TemplateTail,
            TemplateMiddle,
            LBrace,
            Ident,
            Colon,
            Int,
            RBrace,
            Dot,
            Ident,
            TemplateTail,
        ]
    );
    assert_eq!(kinds(r"`\${no}`", TS), [TemplateNoSub]);
}

#[test]
fn numbers_cover_bigint_separators_and_leading_dot() {
    let src = "0xFFn 10n 1_000 .5 1e-3 2.5E+10 0o17 0b1 a?.5:1";
    assert_eq!(
        texts_of(src, TS, Int),
        ["0xFFn", "10n", "1_000", "0o17", "0b1", "1"]
    );
    assert_eq!(texts_of(src, TS, Float), [".5", "1e-3", "2.5E+10", ".5"]);
    assert_eq!(texts_of(src, TS, Question).len(), 1);
}

#[test]
fn private_names_and_unicode_identifiers_are_single_tokens() {
    assert_eq!(
        tokens("#x in o; this.#y; \\u0061b; café", TS)
            .into_iter()
            .filter(|(k, _)| matches!(k, PrivateName | Ident))
            .map(|(_, t)| t)
            .collect::<Vec<_>>(),
        ["#x", "o", "#y", "\\u0061b", "café"]
    );
}

#[test]
fn greater_than_is_always_split_for_the_parser() {
    assert_eq!(kinds("a >> b", TS), [Ident, Gt, Gt, Ident]);
    assert_eq!(
        kinds("Map<A<B>>", TS),
        [Ident, Lt, Ident, Lt, Ident, Gt, Gt]
    );
    assert_eq!(kinds("a >= b", TS), [Ident, Gt, Eq, Ident]);
}

#[test]
fn keywords_after_a_dot_are_property_names() {
    assert_eq!(texts_of("o.default / 2", TS, Slash).len(), 1);
    assert_eq!(kinds("o.default", TS), [Ident, Dot, KwDefault]);
}

#[test]
fn non_null_assertion_ends_an_expression() {
    assert_eq!(
        texts_of("h = alto! / 12 + x!.y / 2; !/a/.test(s)", TS, Slash).len(),
        2
    );
    assert_eq!(texts_of("!/a/.test(s)", TS, Regex), ["/a/"]);
}

#[test]
fn yield_and_await_are_followed_by_expressions() {
    let src = "function* g() { yield /[^<>]/g; await /b/; }";
    assert_eq!(texts_of(src, TSX, Regex), ["/[^<>]/g", "/b/"]);
    assert!(texts_of(src, TSX, JsxText).is_empty());
}

#[test]
fn jsx_text_keeps_apostrophes_and_braces_open_code() {
    let src = "x = <p className=\"n\">Don't {a / 2} it's</p>;";
    assert!(error_kinds(src, TSX).is_empty());
    assert_eq!(texts_of(src, TSX, JsxText), ["Don't ", " it's"]);
    assert_eq!(texts_of(src, TSX, Slash).len(), 2);
}

#[test]
fn jsx_nests_through_attributes_fragments_and_self_closing() {
    let src = "<>\n  <A b={<C d='e' />} f>{xs.map((x) => <B key={x}>{x}</B>)}</A>\n</>";
    let lexed = lex(src, TSX);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    assert_eq!(
        texts_of(src, TSX, JsxText),
        ["\n  ", "\n"]
    );
    assert_eq!(texts_of(src, TSX, String), ["'e'"]);
}

#[test]
fn tsx_type_parameters_are_not_jsx() {
    for src in [
        "const f = <T,>(v: T) => v;",
        "const f = <T extends unknown>(v: T) => v;",
        "const f = <T = string>(v: T) => v;",
    ] {
        assert!(error_kinds(src, TSX).is_empty(), "{src}");
        assert!(texts_of(src, TSX, JsxText).is_empty(), "{src}");
    }
}

#[test]
fn type_assertions_in_ts_are_comparisons() {
    let src = "const n = <number>(<unknown>\"3\");";
    assert!(error_kinds(src, TS).is_empty());
    assert!(texts_of(src, TS, JsxText).is_empty());
}

#[test]
fn unterminated_constructs_report_and_still_cover_the_input() {
    for (src, dialect, expected) in [
        ("a = 'x\nb", TS, LexErrorKind::UnterminatedString),
        ("/* nunca", TS, LexErrorKind::UnterminatedComment),
        ("x = /abc\ny", TS, LexErrorKind::UnterminatedRegex),
        ("`a${b", TS, LexErrorKind::UnterminatedTemplate),
        ("`abc", TS, LexErrorKind::UnterminatedTemplate),
        ("x = <div>hola", TSX, LexErrorKind::UnterminatedJsx),
    ] {
        assert!(
            error_kinds(src, dialect).contains(&expected),
            "{src}: {:?}",
            error_kinds(src, dialect)
        );
    }
}

#[test]
fn garbage_never_panics_and_stays_lossless() {
    for src in [
        "\u{0}\u{1}§ @@ ¿? ] } ) ``` ${ <<<>>> </ <a b=",
        "}}}}`${`${`${",
        "<a {...p} b={<c>{</c>}>",
        "\\u{zz} #",
        "\r\n\u{2028}\u{2029}\t\u{feff}",
    ] {
        lex(src, TSX);
        lex(src, TS);
    }
}

#[test]
fn hashbang_and_comments_are_classified() {
    let src = "#!/usr/bin/env node\n/** Doc. */\n// @ts-expect-error TS2345\n/*! MIT */\n// x\n";
    let comments: Vec<CommentKind> = lex(src, TS)
        .tokens
        .iter()
        .filter(|t| t.kind == Comment)
        .map(|t| classify_js(t.text(src), false))
        .collect();
    assert_eq!(
        comments,
        [
            CommentKind::Directive,
            CommentKind::Doc,
            CommentKind::Directive,
            CommentKind::License,
            CommentKind::Narrative,
        ]
    );
}

/// `ARGUS_JS_ROUNDTRIP_DIR=<carpeta> cargo test --test js_lexer_tests -- --ignored --nocapture`
#[test]
#[ignore = "requiere ARGUS_JS_ROUNDTRIP_DIR con un corpus JS/TS real"]
fn external_corpus_round_trips() {
    let var = std::env::var("ARGUS_JS_ROUNDTRIP_DIR").expect("ARGUS_JS_ROUNDTRIP_DIR");
    let mut stack = vec![PathBuf::from(var)];
    let (mut files, mut bytes, mut with_errors) = (0usize, 0usize, Vec::new());
    let exts = ["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !exts.contains(&ext) {
                continue;
            }
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            let lexed = javascript::lex(&source, Dialect::from_path(&path)).unwrap();
            assert_lossless(&source, &lexed, &path.display().to_string());
            files += 1;
            bytes += source.len();
            if let Some(err) = lexed.errors.first() {
                with_errors.push(format!(
                    "{} @{}: {}",
                    path.display(),
                    err.range.start,
                    err.kind
                ));
            }
        }
    }
    println!(
        "{files} archivos, {bytes} bytes, {} con errores de lexer",
        with_errors.len()
    );
    for line in &with_errors {
        println!("  {line}");
    }
    assert!(files > 0);
}
