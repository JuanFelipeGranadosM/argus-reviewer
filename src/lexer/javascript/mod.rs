//! Lexer de JavaScript/TypeScript (ES2024, TypeScript, JSX/TSX) sin pérdida.
//!
//! Sin parser no se sabe si `/` divide o abre una regex, ni si `<` compara o abre JSX: se decide
//! por el token significativo anterior, como hacen los escáneres de JS. Las plantillas se emiten
//! por tramos (`TemplateHead`/`Middle`/`Tail`) y sus `${...}` se lexean como código normal. Los `>`
//! van siempre sueltos: `>>`, `>=` o `>>>=` los recompone el parser cuando no hay trivia entre
//! ellos, porque en genéricos (`Array<Array<T>>`) cada `>` cierra un nivel.

mod jsx;
mod literals;
mod punct;

use std::path::Path;

use self::punct::{is_ident_continue, is_ident_start, is_line_terminator, is_whitespace};
use super::cursor::Cursor;
use super::{check_size, LexError, LexErrorKind, Lexed, SourceError};
use crate::syntax::{SyntaxKind, TextRange, Token};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Dialect {
    pub jsx: bool,
}

impl Dialect {
    /// `.ts`, `.mts` y `.cts` no admiten JSX (`<T>x` es una aserción de tipo); el resto sí.
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        Self {
            jsx: !matches!(ext, "ts" | "mts" | "cts"),
        }
    }
}

pub fn lex(source: &str, dialect: Dialect) -> Result<Lexed, SourceError> {
    check_size(source)?;
    let mut lexer = JsLexer::new(source, dialect);
    lexer.run();
    Ok(lexer.finish())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Ctx {
    /// `{` de bloque, objeto o tipo.
    Brace,
    /// `${` de una plantilla: su `}` reanuda el texto de la plantilla.
    TemplateExpr,
    /// `{` dentro de JSX: su `}` vuelve a la etiqueta o a los hijos.
    JsxExpr,
    JsxTag {
        closing: bool,
        self_closing: bool,
    },
    JsxChildren,
}

struct JsLexer<'a> {
    cur: Cursor<'a>,
    out: Lexed,
    stack: Vec<Ctx>,
    /// Los dos últimos tokens significativos, el más reciente primero.
    prev: [Option<SyntaxKind>; 2],
    /// Por cada `(` abierto, si es la condición de `if`/`while`/`for`/`with`: tras su `)` empieza
    /// una sentencia, así que `/` abre una regex.
    parens: Vec<bool>,
    after_condition: bool,
    dialect: Dialect,
}

impl<'a> JsLexer<'a> {
    fn new(source: &'a str, dialect: Dialect) -> Self {
        Self {
            cur: Cursor::new(source),
            out: Lexed::default(),
            stack: Vec::new(),
            prev: [None, None],
            parens: Vec::new(),
            after_condition: false,
            dialect,
        }
    }

    fn run(&mut self) {
        if self.cur.rest().starts_with("#!") {
            self.cur.eat_while(|c| !is_line_terminator(c));
            self.push_from(SyntaxKind::Comment, 0);
        }
        while !self.cur.is_eof() {
            let before = self.cur.offset();
            match self.stack.last().copied() {
                Some(Ctx::JsxTag { closing, .. }) => self.jsx_tag_token(closing),
                Some(Ctx::JsxChildren) => self.jsx_child_token(),
                _ => self.token(),
            }
            if self.cur.offset() == before {
                self.cur.bump();
                self.error(LexErrorKind::UnexpectedChar, before);
                self.push_from(SyntaxKind::Error, before);
            }
        }
    }

    fn finish(mut self) -> Lexed {
        let at = TextRange::empty(self.cur.offset());
        let open_jsx = self
            .stack
            .iter()
            .any(|c| matches!(c, Ctx::JsxTag { .. } | Ctx::JsxChildren));
        if open_jsx {
            self.out.errors.push(LexError {
                kind: LexErrorKind::UnterminatedJsx,
                range: at,
            });
        }
        if self.stack.contains(&Ctx::TemplateExpr) {
            self.out.errors.push(LexError {
                kind: LexErrorKind::UnterminatedTemplate,
                range: at,
            });
        }
        self.out.tokens.push(Token::new(SyntaxKind::EndOfFile, at));
        self.out
    }

    fn token(&mut self) {
        let start = self.cur.offset();
        let Some(c) = self.cur.first() else {
            return;
        };
        let kind = match c {
            c if is_line_terminator(c) => self.newline(),
            c if is_whitespace(c) => self.whitespace(),
            '/' => self.slash(start),
            '\'' | '"' => literals::string(&mut self.cur, &mut self.out.errors),
            '`' => self.template(),
            '{' => self.open(Ctx::Brace, SyntaxKind::LBrace),
            '}' => self.close_brace(),
            '0'..='9' => literals::number(&mut self.cur),
            '.' if self.cur.second().is_some_and(|d| d.is_ascii_digit()) => {
                literals::number(&mut self.cur)
            }
            '#' => self.private_name(start),
            '<' if self.jsx_starts_here() => self.open(
                Ctx::JsxTag {
                    closing: false,
                    self_closing: false,
                },
                SyntaxKind::Lt,
            ),
            '\\' if self.cur.second() == Some('u') => self.ident(start),
            c if is_ident_start(c) => self.ident(start),
            _ => self.operator(start),
        };
        self.push_from(kind, start);
    }

    fn newline(&mut self) -> SyntaxKind {
        if !self.cur.eat_newline() {
            self.cur.bump();
        }
        SyntaxKind::Newline
    }

    fn whitespace(&mut self) -> SyntaxKind {
        self.cur.eat_while(is_whitespace);
        SyntaxKind::Whitespace
    }

    /// Consume un carácter de apertura y entra en `ctx`.
    fn open(&mut self, ctx: Ctx, kind: SyntaxKind) -> SyntaxKind {
        self.cur.bump();
        self.stack.push(ctx);
        kind
    }

    fn slash(&mut self, start: u32) -> SyntaxKind {
        match self.cur.second() {
            Some('/') => {
                self.cur.eat_while(|c| !is_line_terminator(c));
                SyntaxKind::Comment
            }
            Some('*') => self.block_comment(start),
            _ if self.regex_allowed() => literals::regex(&mut self.cur, &mut self.out.errors),
            _ => self.operator(start),
        }
    }

    fn block_comment(&mut self, start: u32) -> SyntaxKind {
        match self.cur.rest().get(2..).and_then(|body| body.find("*/")) {
            Some(end) => self.cur.advance(end + 4),
            None => {
                self.cur.advance(usize::MAX);
                self.error(LexErrorKind::UnterminatedComment, start);
            }
        }
        SyntaxKind::Comment
    }

    fn template(&mut self) -> SyntaxKind {
        let kind = literals::template_chunk(&mut self.cur, &mut self.out.errors);
        if matches!(kind, SyntaxKind::TemplateHead | SyntaxKind::TemplateMiddle) {
            self.stack.push(Ctx::TemplateExpr);
        }
        kind
    }

    fn close_brace(&mut self) -> SyntaxKind {
        if self.stack.pop() == Some(Ctx::TemplateExpr) {
            return self.template();
        }
        self.cur.bump();
        SyntaxKind::RBrace
    }

    fn private_name(&mut self, start: u32) -> SyntaxKind {
        self.cur.bump();
        if !self.cur.first().is_some_and(is_ident_start) {
            self.error(LexErrorKind::UnexpectedChar, start);
            return SyntaxKind::Error;
        }
        self.cur.eat_while(is_ident_continue);
        SyntaxKind::PrivateName
    }

    fn ident(&mut self, start: u32) -> SyntaxKind {
        loop {
            match self.cur.first() {
                Some('\\') if self.cur.second() == Some('u') => {
                    literals::unicode_escape(&mut self.cur)
                }
                Some(c) if is_ident_continue(c) => {
                    self.cur.bump();
                }
                _ => break,
            }
        }
        punct::keyword(self.cur.slice_from(start)).unwrap_or(SyntaxKind::Ident)
    }

    fn operator(&mut self, start: u32) -> SyntaxKind {
        let Some((kind, op)) = punct::operator(self.cur.rest()) else {
            self.cur.bump();
            self.error(LexErrorKind::UnexpectedChar, start);
            return SyntaxKind::Error;
        };
        self.cur.eat_str(op);
        match kind {
            SyntaxKind::LParen => {
                let [last, _] = self.prev;
                self.parens.push(last.is_some_and(punct::opens_condition));
            }
            SyntaxKind::RParen => self.after_condition = self.parens.pop().unwrap_or(false),
            _ => {}
        }
        kind
    }

    /// Una palabra reservada tras `.` o `?.` es un nombre de propiedad (`x.default / 2`), y un `!`
    /// tras una expresión es la aserción no nula de TypeScript (`alto! / 12`).
    fn regex_allowed(&self) -> bool {
        let [last, before] = self.prev;
        let Some(last) = last else {
            return true;
        };
        if last == SyntaxKind::RParen && self.after_condition {
            return true;
        }
        let property =
            last.is_keyword() && matches!(before, Some(SyntaxKind::Dot | SyntaxKind::QuestionDot));
        let non_null = last == SyntaxKind::Bang && before.is_some_and(punct::ends_expression);
        !property && !non_null && !punct::ends_expression(last)
    }

    fn error(&mut self, kind: LexErrorKind, start: u32) {
        let range = TextRange::new(start, self.cur.offset());
        self.out.errors.push(LexError { kind, range });
    }

    fn push_from(&mut self, kind: SyntaxKind, start: u32) {
        let range = TextRange::new(start, self.cur.offset());
        if range.is_empty() {
            return;
        }
        self.out.tokens.push(Token::new(kind, range));
        if !kind.is_trivia() {
            let [last, _] = self.prev;
            self.prev = [Some(kind), last];
        }
    }
}
