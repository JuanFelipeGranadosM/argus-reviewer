//! JSX/TSX. Un `<` en posición de expresión abre una etiqueta. Dentro de la etiqueta los nombres
//! admiten `-` (`aria-label`) y las cadenas no tienen escapes; entre etiquetas el texto es `JsxText`,
//! así que un apóstrofo (`Don't`) no abre una cadena. Si una "etiqueta" contiene algo imposible en
//! JSX (era un genérico mal detectado), se abandona el modo JSX y se sigue lexeando como código.

use super::punct::{is_ident_continue, is_ident_start, is_line_terminator, is_whitespace};
use super::{Ctx, JsLexer};
use crate::lexer::LexErrorKind;
use crate::syntax::SyntaxKind;

/// En `.tsx`, `<T,>`, `<T extends U>`, `<T = U>` y `<T>(` son parámetros de tipo, no JSX.
pub(crate) fn looks_like_type_parameters(after_lt: &str) -> bool {
    let name_len = after_lt
        .find(|c: char| !is_ident_continue(c))
        .unwrap_or(after_lt.len());
    let name = after_lt.get(..name_len).unwrap_or_default();
    let rest = after_lt.get(name_len..).unwrap_or_default();
    let next = rest.trim_start();
    let extends = next
        .strip_prefix("extends")
        .is_some_and(|r| r.starts_with(char::is_whitespace));
    let generic_arrow =
        rest.starts_with(">(") && name.starts_with(|c: char| c.is_ascii_uppercase());
    next.starts_with(',') || next.starts_with('=') || extends || generic_arrow
}

impl JsLexer<'_> {
    pub(super) fn jsx_starts_here(&self) -> bool {
        if !self.dialect.jsx || !self.regex_allowed() {
            return false;
        }
        let after_lt = self.cur.rest().get(1..).unwrap_or_default();
        match after_lt.chars().next() {
            Some('>') => true,
            Some(c) if is_ident_start(c) => !looks_like_type_parameters(after_lt),
            _ => false,
        }
    }

    pub(super) fn jsx_tag_token(&mut self, closing: bool) {
        let start = self.cur.offset();
        let Some(c) = self.cur.first() else {
            return;
        };
        let kind = match c {
            c if is_line_terminator(c) => self.newline(),
            c if is_whitespace(c) => self.whitespace(),
            '/' if matches!(self.cur.second(), Some('/' | '*')) => self.slash(start),
            '/' => {
                self.cur.bump();
                self.mark_self_closing();
                SyntaxKind::Slash
            }
            '>' => {
                self.cur.bump();
                self.end_tag(closing);
                SyntaxKind::Gt
            }
            '{' => self.open(Ctx::JsxExpr, SyntaxKind::LBrace),
            '"' | '\'' => self.jsx_string(start),
            '=' | '.' | ':' => self.operator(start),
            c if is_ident_start(c) => {
                self.cur.eat_while(|c| is_ident_continue(c) || c == '-');
                SyntaxKind::Ident
            }
            _ => {
                self.stack.pop();
                self.token();
                return;
            }
        };
        self.push_from(kind, start);
    }

    pub(super) fn jsx_child_token(&mut self) {
        let start = self.cur.offset();
        let kind = match self.cur.first() {
            None => return,
            Some('<') => {
                let closing = self.cur.second() == Some('/');
                let tag = Ctx::JsxTag {
                    closing,
                    self_closing: false,
                };
                self.open(tag, SyntaxKind::Lt)
            }
            Some('{') => self.open(Ctx::JsxExpr, SyntaxKind::LBrace),
            Some(_) => {
                self.cur.eat_while(|c| c != '<' && c != '{');
                SyntaxKind::JsxText
            }
        };
        self.push_from(kind, start);
    }

    fn mark_self_closing(&mut self) {
        if let Some(Ctx::JsxTag { self_closing, .. }) = self.stack.last_mut() {
            *self_closing = true;
        }
    }

    /// `>` de una etiqueta: la de apertura abre sus hijos; la de cierre cierra los del elemento.
    fn end_tag(&mut self, closing: bool) {
        let self_closing = matches!(
            self.stack.pop(),
            Some(Ctx::JsxTag {
                self_closing: true,
                ..
            })
        );
        if closing {
            if self.stack.last() == Some(&Ctx::JsxChildren) {
                self.stack.pop();
            }
        } else if !self_closing {
            self.stack.push(Ctx::JsxChildren);
        }
    }

    /// Las cadenas de atributos JSX no tienen escapes y pueden ocupar varias líneas.
    fn jsx_string(&mut self, start: u32) -> SyntaxKind {
        let quote = self.cur.bump();
        self.cur.eat_while(|c| Some(c) != quote);
        if self.cur.bump().is_none() {
            self.error(LexErrorKind::UnterminatedString, start);
        }
        SyntaxKind::String
    }
}
