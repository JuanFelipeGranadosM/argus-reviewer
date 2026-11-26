#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};

use argus_reviewer::lexer::comments::{classify_python, CommentKind};
use argus_reviewer::lexer::{python, LexErrorKind, Lexed};
use argus_reviewer::syntax::SyntaxKind::{self, *};

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
    assert_eq!(
        next as usize,
        source.len(),
        "{label}: los tokens no llegan al final"
    );
    assert_eq!(rebuilt, source, "{label}: el round-trip no es idéntico");
    assert_eq!(
        lexed.tokens.last().map(|t| t.kind),
        Some(EndOfFile),
        "{label}"
    );
}

fn lex(source: &str) -> Lexed {
    let lexed = python::lex(source).unwrap();
    assert_lossless(source, &lexed, source);
    lexed
}

fn kinds(source: &str) -> Vec<SyntaxKind> {
    lex(source)
        .tokens
        .iter()
        .map(|t| t.kind)
        .filter(|k| !k.is_trivia())
        .collect()
}

fn error_kinds(source: &str) -> Vec<LexErrorKind> {
    lex(source).errors.iter().map(|e| e.kind).collect()
}

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python");
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "py"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    files
}

#[test]
fn fixtures_round_trip_without_errors() {
    for path in fixtures() {
        let source = fs::read_to_string(&path).unwrap();
        let lexed = python::lex(&source).unwrap();
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
fn every_prefix_of_fixtures_round_trips() {
    for path in fixtures() {
        let source = fs::read_to_string(&path).unwrap();
        for (cut, _) in source.char_indices() {
            let prefix = &source[..cut];
            let lexed = python::lex(prefix).unwrap();
            assert_lossless(prefix, &lexed, &format!("{} [..{cut}]", path.display()));
        }
    }
}

#[test]
fn crlf_and_cr_newlines_round_trip() {
    let source = "if x:\r\n    y = 1\r\n\r\nz = 2\rw = 3";
    let ks = kinds(source);
    assert_eq!(ks.iter().filter(|&&k| k == LogicalNewline).count(), 4);
    assert_eq!(ks.iter().filter(|&&k| k == Indent).count(), 1);
}

#[test]
fn indentation_emits_indent_and_dedent() {
    let source = "def f():\n    if x:\n        pass\n    return 1\n";
    assert_eq!(
        kinds(source),
        [
            KwDef,
            Ident,
            LParen,
            RParen,
            Colon,
            LogicalNewline,
            Indent,
            KwIf,
            Ident,
            Colon,
            LogicalNewline,
            Indent,
            KwPass,
            LogicalNewline,
            Dedent,
            KwReturn,
            Int,
            LogicalNewline,
            Dedent,
            EndOfFile,
        ]
    );
}

#[test]
fn blank_and_comment_lines_do_not_affect_indentation() {
    let source = "if x:\n    a\n\n# c\n  \n    b\n";
    assert_eq!(
        kinds(source),
        [
            KwIf,
            Ident,
            Colon,
            LogicalNewline,
            Indent,
            Ident,
            LogicalNewline,
            Ident,
            LogicalNewline,
            Dedent,
            EndOfFile
        ]
    );
}

#[test]
fn newlines_inside_brackets_and_continuations_are_trivia() {
    assert_eq!(
        kinds("x = (\n  1,\n)\n"),
        [
            Ident,
            Eq,
            LParen,
            Int,
            Comma,
            RParen,
            LogicalNewline,
            EndOfFile
        ]
    );
    assert_eq!(
        kinds("x = 1 + \\\n    2\n"),
        [Ident, Eq, Int, Plus, Int, LogicalNewline, EndOfFile]
    );
}

#[test]
fn missing_trailing_newline_still_closes_blocks() {
    assert_eq!(
        kinds("if x:\n    y"),
        [
            KwIf,
            Ident,
            Colon,
            LogicalNewline,
            Indent,
            Ident,
            LogicalNewline,
            Dedent,
            EndOfFile
        ]
    );
}

#[test]
fn inconsistent_dedent_is_reported() {
    assert_eq!(
        error_kinds("if x:\n        a\n    b\n"),
        [LexErrorKind::InconsistentDedent]
    );
}

#[test]
fn fstrings_with_nested_quotes_are_one_token() {
    assert_eq!(
        kinds(r#"f"{x["k"]}" + f'{a!r:>{w}}'"#),
        [FString, Plus, FString, LogicalNewline, EndOfFile]
    );
    assert_eq!(
        kinds(r#"f"{", ".join(xs)}""#),
        [FString, LogicalNewline, EndOfFile]
    );
    assert_eq!(
        kinds("f'''{\n x\n}'''"),
        [FString, LogicalNewline, EndOfFile]
    );
}

#[test]
fn fstring_fields_allow_comments_and_literal_format_specs() {
    let with_comment = "f'''{\n    x  # it's \"fine\" }\n}'''\ny = 1\n";
    assert!(error_kinds(with_comment).is_empty());
    assert_eq!(
        kinds(with_comment),
        [FString, LogicalNewline, Ident, Eq, Int, LogicalNewline, EndOfFile]
    );
    let specs = r#"f"{x:'^10}" f'{n:#x}' f"{v:{w}.{p}f}" f"{a['k']:>{w}}" f"{x:=5}""#;
    assert!(error_kinds(specs).is_empty());
    assert_eq!(kinds(specs).iter().filter(|&&k| k == FString).count(), 5);
}

#[test]
fn string_prefixes_and_quotes() {
    assert_eq!(
        kinds(r#"rb'\x' """a"b""" '' BR"\"" u'u'"#),
        [
            String,
            String,
            String,
            String,
            String,
            LogicalNewline,
            EndOfFile
        ]
    );
}

#[test]
fn unterminated_string_stops_at_line_end() {
    let source = "s = 'abc\nx = 1\n";
    assert_eq!(error_kinds(source), [LexErrorKind::UnterminatedString]);
    assert!(kinds(source).ends_with(&[Ident, Eq, Int, LogicalNewline, EndOfFile]));
    assert_eq!(
        error_kinds("s = '''abc"),
        [LexErrorKind::UnterminatedString]
    );
}

#[test]
fn numbers() {
    assert_eq!(
        kinds("0xFF 1_000 3.14 .5 1. 1e5 2.5E-3 2j 1if"),
        [
            Int,
            Int,
            Float,
            Float,
            Float,
            Float,
            Float,
            Imaginary,
            Int,
            KwIf,
            LogicalNewline,
            EndOfFile
        ]
    );
}

#[test]
fn soft_keywords_are_identifiers() {
    assert_eq!(
        kinds("match type case _"),
        [Ident, Ident, Ident, Ident, LogicalNewline, EndOfFile]
    );
    assert_eq!(
        kinds("lambda None"),
        [KwLambda, KwNone, LogicalNewline, EndOfFile]
    );
}

#[test]
fn unicode_identifiers_are_single_tokens() {
    for source in [
        "x\u{e0100} = 4",
        "\u{2118} = 1",
        "nama\u{0938}\u{094d}te = 1",
        "\u{1d518}\u{1d52b} = 1",
        "µ = 2",
    ] {
        assert!(error_kinds(source).is_empty(), "{source}");
        assert_eq!(
            kinds(source),
            [Ident, Eq, Int, LogicalNewline, EndOfFile],
            "{source}"
        );
    }
}

#[test]
fn garbage_never_panics_and_round_trips() {
    for source in [
        "",
        "$?`",
        "\\x",
        "\\",
        "\u{feff}x = 1",
        "\0",
        "'''",
        "f'{",
        "f'{'",
        "f\"{x\"",
        "((((",
        "))))\n  x",
        "\t \x0c\n",
        "😀 = 1",
        "\r",
        "\r\n\r\n",
        "x\\\r\ny",
        "'\\",
        "f'\\N{",
        "0x",
        "1e",
    ] {
        lex(source);
    }
    assert_eq!(error_kinds("a $ b"), [LexErrorKind::UnexpectedChar]);
    assert_eq!(error_kinds("a \\ b"), [LexErrorKind::StrayBackslash]);
}

#[test]
fn fixture_comments_are_classified() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python/llm_soup_service.py");
    let source = fs::read_to_string(path).unwrap();
    let lexed = lex(&source);
    let mut in_header = true;
    let mut found = Vec::new();
    for token in &lexed.tokens {
        match token.kind {
            Comment => found.push((
                token.text(&source),
                classify_python(token.text(&source), in_header),
            )),
            k if !k.is_trivia() => in_header = false,
            _ => {}
        }
    }
    let kind_of = |text: &str| found.iter().find(|(t, _)| *t == text).map(|(_, k)| *k);
    assert_eq!(
        kind_of("#!/usr/bin/env python3"),
        Some(CommentKind::Directive)
    );
    assert_eq!(
        kind_of("# -*- coding: utf-8 -*-"),
        Some(CommentKind::Directive)
    );
    assert_eq!(
        kind_of("# Copyright 2026 Ejemplo S.A."),
        Some(CommentKind::License)
    );
    assert_eq!(
        kind_of("# type: ignore[attr-defined]"),
        Some(CommentKind::Directive)
    );
    assert_eq!(kind_of("# noqa: E501"), Some(CommentKind::Directive));
    assert_eq!(
        kind_of("# TODO: paginar resultados"),
        Some(CommentKind::Marker)
    );
    assert_eq!(kind_of("#: Tasa de IVA vigente"), Some(CommentKind::Doc));
    let narrative = found
        .iter()
        .filter(|(_, k)| *k == CommentKind::Narrative)
        .count();
    assert_eq!(narrative, 7, "{found:#?}");
}

/// `ARGUS_ROUNDTRIP_DIR=<carpeta> cargo test --test lexer_parser_tests -- --ignored --nocapture`
#[test]
#[ignore = "requiere ARGUS_ROUNDTRIP_DIR con un corpus Python real"]
fn external_corpus_round_trips() {
    let root = PathBuf::from(std::env::var("ARGUS_ROUNDTRIP_DIR").expect("ARGUS_ROUNDTRIP_DIR"));
    let mut stack = vec![root];
    let (mut files, mut bytes, mut with_errors) = (0usize, 0usize, Vec::new());
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "py") {
                continue;
            }
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            let lexed = python::lex(&source).unwrap();
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
