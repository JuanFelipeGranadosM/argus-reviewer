//! Taxonomía de comentarios. `argus clean` solo puede eliminar comentarios `Narrative`.

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CommentKind {
    /// Documentación (`#:` de Sphinx en Python; JSDoc en JS/TS).
    Doc,
    /// Instrucción para una herramienta: `type: ignore`, `noqa`, shebang, cookie de codificación, ...
    Directive,
    /// Cabecera de licencia o copyright.
    License,
    /// Deuda registrada: `TODO`, `FIXME`, `HACK`, `XXX`, `NOTE`.
    Marker,
    Narrative,
}

const PY_DIRECTIVES: &[&str] = &[
    "type:", "noqa", "pragma:", "pylint:", "mypy:", "pyright:", "pyre-", "fmt:", "isort:", "ruff:",
    "nosec", "yapf:", "black:",
];

const LICENSE_WORDS: &[&str] = &[
    "copyright",
    "license",
    "licence",
    "all rights reserved",
    "(c)",
];

const MARKERS: &[&str] = &["TODO", "FIXME", "HACK", "XXX", "NOTE"];

/// Prefijos (en minúsculas) de comentarios JS/TS dirigidos a herramientas.
const JS_DIRECTIVES: &[&str] = &[
    "@ts-",
    "eslint-disable",
    "eslint-enable",
    "eslint-env",
    "eslint ",
    "global ",
    "globals ",
    "prettier-ignore",
    "biome-ignore",
    "istanbul ignore",
    "c8 ignore",
    "v8 ignore",
    "@jsx",
    "#__pure__",
    "@__pure__",
    "#__no_side_effects__",
    "webpackchunkname",
    "webpackignore",
    "webpackmode",
    "webpackprefetch",
    "webpackpreload",
    "webpackexports",
    "@vite-ignore",
    "deno-lint-ignore",
    "deno-fmt-ignore",
    "tslint:",
    "jshint",
    "@flow",
    "@noflow",
    "@refresh",
    "<reference",
    "<amd-module",
    "# sourcemappingurl",
    "@ sourcemappingurl",
    "@vitest-environment",
    "@jest-environment",
];

/// Texto de un comentario JS sin delimitadores (`//`, `///`, `/*`, `/**`, `*/`) ni asteriscos iniciales.
pub fn js_comment_body(text: &str) -> &str {
    let inner = match text.strip_prefix("//") {
        Some(rest) => rest.trim_start_matches('/'),
        None => {
            let open = text.strip_prefix("/*").unwrap_or(text);
            open.strip_suffix("*/").unwrap_or(open)
        }
    };
    inner.trim().trim_start_matches(['*', '!']).trim()
}

/// Clasifica un comentario de JavaScript/TypeScript. `text` incluye los delimitadores.
pub fn classify_js(text: &str, in_header: bool) -> CommentKind {
    if text.starts_with("#!") {
        return CommentKind::Directive;
    }
    if text.starts_with("/*!") {
        return CommentKind::License;
    }
    let body = js_comment_body(text);
    let lower = body.to_lowercase();
    if JS_DIRECTIVES.iter().any(|d| lower.starts_with(d)) {
        return CommentKind::Directive;
    }
    if lower.starts_with("@license") || lower.starts_with("@preserve") || is_license(&lower, in_header)
    {
        return CommentKind::License;
    }
    if text.starts_with("/**") && !text.starts_with("/**/") {
        return CommentKind::Doc;
    }
    if starts_with_marker(body) {
        return CommentKind::Marker;
    }
    CommentKind::Narrative
}

/// Clasifica un comentario `#` de Python. `text` incluye el `#`; `in_header` indica que el comentario
/// aparece antes del primer token de código del archivo.
pub fn classify_python(text: &str, in_header: bool) -> CommentKind {
    if text.starts_with("#!") {
        return CommentKind::Directive;
    }
    if text.starts_with("#:") {
        return CommentKind::Doc;
    }
    let body = text.trim_start_matches('#').trim();
    let lower = body.to_lowercase();
    if PY_DIRECTIVES.iter().any(|d| lower.starts_with(d)) {
        return CommentKind::Directive;
    }
    if in_header && (lower.contains("coding:") || lower.contains("coding=")) {
        return CommentKind::Directive;
    }
    if is_license(&lower, in_header) {
        return CommentKind::License;
    }
    if starts_with_marker(body) {
        return CommentKind::Marker;
    }
    CommentKind::Narrative
}

fn is_license(lower: &str, in_header: bool) -> bool {
    lower.contains("spdx-license-identifier")
        || (in_header && LICENSE_WORDS.iter().any(|w| lower.contains(w)))
}

fn starts_with_marker(body: &str) -> bool {
    MARKERS.iter().any(|marker| {
        body.strip_prefix(marker)
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directives_are_preserved() {
        for text in [
            "#!/usr/bin/env python3",
            "# type: ignore[arg-type]",
            "# noqa: E501",
            "# pragma: no cover",
            "# pylint: disable=too-many-locals",
            "# fmt: off",
        ] {
            assert_eq!(
                classify_python(text, false),
                CommentKind::Directive,
                "{text}"
            );
        }
        assert_eq!(
            classify_python("# -*- coding: utf-8 -*-", true),
            CommentKind::Directive
        );
    }

    #[test]
    fn license_only_in_header_unless_spdx() {
        assert_eq!(
            classify_python("# Copyright 2026 Acme", true),
            CommentKind::License
        );
        assert_eq!(
            classify_python("# Copyright 2026 Acme", false),
            CommentKind::Narrative
        );
        assert_eq!(
            classify_python("# SPDX-License-Identifier: MIT", false),
            CommentKind::License
        );
    }

    #[test]
    fn markers_need_uppercase_word() {
        assert_eq!(
            classify_python("# TODO: paginar", false),
            CommentKind::Marker
        );
        assert_eq!(
            classify_python("# FIXME(juan) roto", false),
            CommentKind::Marker
        );
        assert_eq!(
            classify_python("# note that this is slow", false),
            CommentKind::Narrative
        );
        assert_eq!(
            classify_python("# TODOS los usuarios", false),
            CommentKind::Narrative
        );
    }

    #[test]
    fn llm_soup_is_narrative() {
        assert_eq!(
            classify_python("# Validar usuario", false),
            CommentKind::Narrative
        );
        assert_eq!(
            classify_python("# Return the result", false),
            CommentKind::Narrative
        );
        assert_eq!(
            classify_python("#: Tasa de IVA vigente", false),
            CommentKind::Doc
        );
    }

    #[test]
    fn js_comments_are_classified() {
        let cases = [
            ("// @ts-expect-error: tipos de la lib", CommentKind::Directive),
            ("// eslint-disable-next-line no-console", CommentKind::Directive),
            ("/* webpackChunkName: \"vista\" */", CommentKind::Directive),
            ("/// <reference types=\"vite/client\" />", CommentKind::Directive),
            ("#!/usr/bin/env node", CommentKind::Directive),
            ("/*! lib v1 | MIT */", CommentKind::License),
            ("/** Calcula el subtotal. */", CommentKind::Doc),
            ("/**\n * @param x valor\n */", CommentKind::Doc),
            ("// TODO: paginar", CommentKind::Marker),
            ("/*\n * FIXME: roto\n */", CommentKind::Marker),
            ("// Obtener el usuario", CommentKind::Narrative),
            ("/**/", CommentKind::Narrative),
        ];
        for (text, kind) in cases {
            assert_eq!(classify_js(text, false), kind, "{text}");
        }
        assert_eq!(
            classify_js("// Copyright 2026 Acme", true),
            CommentKind::License
        );
    }
}
