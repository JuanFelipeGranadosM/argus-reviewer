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
}
