//! Tokens significativos de un archivo (sin espacios ni comentarios) con sus pares de `()`, `[]` y `{}`.

use crate::lexer::Lexed;
use crate::syntax::{SyntaxKind, TextRange, Token};

pub(super) struct Sig<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    /// Para cada apertura, el índice de su cierre; `None` si no cierra o no es una apertura.
    close: Vec<Option<usize>>,
}

impl<'a> Sig<'a> {
    pub(super) fn new(source: &'a str, lexed: &Lexed) -> Self {
        let tokens: Vec<Token> = lexed
            .tokens
            .iter()
            .copied()
            .filter(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::EndOfFile)
            .collect();
        let close = match_brackets(&tokens);
        Self {
            source,
            tokens,
            close,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.tokens.len()
    }

    pub(super) fn kind(&self, i: usize) -> Option<SyntaxKind> {
        self.tokens.get(i).map(|t| t.kind)
    }

    pub(super) fn is(&self, i: usize, kind: SyntaxKind) -> bool {
        self.kind(i) == Some(kind)
    }

    pub(super) fn text(&self, i: usize) -> &'a str {
        self.tokens.get(i).map_or("", |t| t.text(self.source))
    }

    pub(super) fn is_word(&self, i: usize, word: &str) -> bool {
        self.is(i, SyntaxKind::Ident) && self.text(i) == word
    }

    /// Tipo del token anterior a `i`, si existe.
    pub(super) fn prev(&self, i: usize) -> Option<SyntaxKind> {
        i.checked_sub(1).and_then(|p| self.kind(p))
    }

    pub(super) fn close_of(&self, i: usize) -> Option<usize> {
        self.close.get(i).copied().flatten()
    }

    /// Rango desde el inicio del token `first` hasta el final del token `last`.
    pub(super) fn span(&self, first: usize, last: usize) -> TextRange {
        let start = self.tokens.get(first).map_or(0, |t| t.range.start);
        let end = self.tokens.get(last).map_or(start, |t| t.range.end);
        TextRange::new(start, end.max(start))
    }

    /// Texto fuente entre el final del token `a` y el inicio del token `b` (comentarios incluidos).
    pub(super) fn between(&self, a: usize, b: usize) -> &'a str {
        let (Some(a), Some(b)) = (self.tokens.get(a), self.tokens.get(b)) else {
            return "";
        };
        TextRange::new(a.range.end, b.range.start.max(a.range.end)).slice(self.source)
    }

    /// Texto que sigue al token `i` hasta el final de su línea.
    pub(super) fn rest_of_line(&self, i: usize) -> &'a str {
        let Some(token) = self.tokens.get(i) else {
            return "";
        };
        let rest = self
            .source
            .get(token.range.end as usize..)
            .unwrap_or_default();
        rest.split(['\n', '\r']).next().unwrap_or_default()
    }

    pub(super) fn break_before(&self, i: usize) -> bool {
        i.checked_sub(1)
            .is_some_and(|p| self.between(p, i).contains(['\n', '\r']))
    }

    /// Segmentos separados por comas de primer nivel entre la apertura `open` y su cierre `close`,
    /// como pares de índices inclusivos. Con `angles`, las comas dentro de `<...>` no separan.
    pub(super) fn segments(&self, open: usize, close: usize, angles: bool) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let (mut start, mut j, mut angle) = (open + 1, open + 1, 0u32);
        while j < close {
            match self.kind(j) {
                Some(SyntaxKind::LParen | SyntaxKind::LBracket | SyntaxKind::LBrace) => {
                    j = self.close_of(j).map_or(j, |c| c.min(close));
                }
                Some(SyntaxKind::Lt) if angles => angle += 1,
                Some(SyntaxKind::Gt) if angles => angle = angle.saturating_sub(1),
                Some(SyntaxKind::Comma) if angle == 0 => {
                    if j > start {
                        out.push((start, j - 1));
                    }
                    start = j + 1;
                }
                _ => {}
            }
            j += 1;
        }
        if close > start {
            out.push((start, close - 1));
        }
        out
    }

    /// Primer token de primer nivel en `first..=last` que cumple `pred`, saltando los grupos anidados.
    pub(super) fn find_top_level(
        &self,
        first: usize,
        last: usize,
        pred: impl Fn(SyntaxKind) -> bool,
    ) -> Option<usize> {
        let mut j = first;
        while j <= last {
            let kind = self.kind(j)?;
            if pred(kind) {
                return Some(j);
            }
            if matches!(
                kind,
                SyntaxKind::LParen | SyntaxKind::LBracket | SyntaxKind::LBrace
            ) {
                j = self.close_of(j).unwrap_or(j);
            }
            j += 1;
        }
        None
    }
}

fn match_brackets(tokens: &[Token]) -> Vec<Option<usize>> {
    let mut close = vec![None; tokens.len()];
    let mut open: Vec<(SyntaxKind, usize)> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let expected = match token.kind {
            SyntaxKind::LParen | SyntaxKind::LBracket | SyntaxKind::LBrace => {
                open.push((token.kind, i));
                continue;
            }
            SyntaxKind::RParen => SyntaxKind::LParen,
            SyntaxKind::RBracket => SyntaxKind::LBracket,
            SyntaxKind::RBrace => SyntaxKind::LBrace,
            _ => continue,
        };
        if open.last().is_some_and(|&(kind, _)| kind == expected) {
            if let Some(slot) = open.pop().and_then(|(_, at)| close.get_mut(at)) {
                *slot = Some(i);
            }
        }
    }
    close
}
