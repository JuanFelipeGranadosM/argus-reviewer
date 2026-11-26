//! Lexer de Python 3.10+ sin pérdida.
//!
//! Emite `Indent`/`Dedent` de ancho cero según la pila de columnas (tabulador = múltiplo de 8) y
//! distingue el fin de sentencia (`LogicalNewline`) de los saltos de línea que son trivia: líneas en
//! blanco, líneas solo con comentario y saltos dentro de paréntesis, corchetes o llaves.

mod numbers;
mod punct;
mod strings;

use super::cursor::Cursor;
use super::{check_size, LexError, LexErrorKind, Lexed, SourceError};
use crate::syntax::{SyntaxKind, TextRange, Token};

const TAB_SIZE: u32 = 8;
const BOM: char = '\u{feff}';

pub fn lex(source: &str) -> Result<Lexed, SourceError> {
    check_size(source)?;
    let mut lexer = PyLexer::new(source);
    lexer.run();
    Ok(lexer.finish())
}

struct PyLexer<'a> {
    cur: Cursor<'a>,
    out: Lexed,
    indents: Vec<u32>,
    bracket_depth: u32,
    at_line_start: bool,
    line_has_code: bool,
}

impl<'a> PyLexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            cur: Cursor::new(source),
            out: Lexed::default(),
            indents: vec![0],
            bracket_depth: 0,
            at_line_start: true,
            line_has_code: false,
        }
    }

    fn run(&mut self) {
        if self.cur.eat(BOM) {
            self.push_from(SyntaxKind::Whitespace, 0);
        }
        while !self.cur.is_eof() {
            let before = self.cur.offset();
            if self.at_line_start {
                self.at_line_start = false;
                if self.bracket_depth == 0 {
                    self.line_start();
                    continue;
                }
            }
            self.token();
            if self.cur.offset() == before {
                self.cur.bump();
                self.error(LexErrorKind::UnexpectedChar, before);
                self.push_from(SyntaxKind::Error, before);
            }
        }
    }

    fn finish(mut self) -> Lexed {
        let at = TextRange::empty(self.cur.offset());
        if self.line_has_code {
            self.push(SyntaxKind::LogicalNewline, at);
        }
        while self.indents.len() > 1 {
            self.indents.pop();
            self.push(SyntaxKind::Dedent, at);
        }
        self.push(SyntaxKind::EndOfFile, at);
        self.out
    }

    fn line_start(&mut self) {
        let start = self.cur.offset();
        let mut column = 0u32;
        while let Some(c) = self.cur.first() {
            match c {
                ' ' => column = column.saturating_add(1),
                '\t' => {
                    column = (column / TAB_SIZE)
                        .saturating_add(1)
                        .saturating_mul(TAB_SIZE)
                }
                '\x0c' => column = 0,
                _ => break,
            }
            self.cur.bump();
        }
        self.push_from(SyntaxKind::Whitespace, start);
        if !matches!(self.cur.first(), None | Some('#' | '\n' | '\r')) {
            self.indent_to(column);
        }
    }

    fn indent_to(&mut self, column: u32) {
        let at = TextRange::empty(self.cur.offset());
        if column > self.current_indent() {
            self.indents.push(column);
            self.push(SyntaxKind::Indent, at);
            return;
        }
        while self.current_indent() > column {
            self.indents.pop();
            self.push(SyntaxKind::Dedent, at);
        }
        if self.current_indent() != column {
            self.out.errors.push(LexError {
                kind: LexErrorKind::InconsistentDedent,
                range: at,
            });
            self.indents.push(column);
        }
    }

    fn current_indent(&self) -> u32 {
        self.indents.last().copied().unwrap_or(0)
    }

    fn token(&mut self) {
        let start = self.cur.offset();
        let Some(c) = self.cur.first() else {
            return;
        };
        let kind = match c {
            '\n' | '\r' => self.newline(),
            ' ' | '\t' | '\x0c' => {
                self.cur.eat_while(|c| matches!(c, ' ' | '\t' | '\x0c'));
                SyntaxKind::Whitespace
            }
            '#' => {
                self.cur.eat_while(|c| c != '\n' && c != '\r');
                SyntaxKind::Comment
            }
            '\\' => self.backslash(start),
            _ if strings::prefix_len(self.cur.rest()).is_some() => {
                strings::lex_string(&mut self.cur, &mut self.out.errors)
            }
            '0'..='9' => numbers::lex_number(&mut self.cur),
            '.' if self.cur.second().is_some_and(|d| d.is_ascii_digit()) => {
                numbers::lex_number(&mut self.cur)
            }
            _ if punct::is_ident_start(c) => self.ident(start),
            _ => self.operator(start),
        };
        if !kind.is_trivia() && kind != SyntaxKind::LogicalNewline {
            self.line_has_code = true;
        }
        self.push_from(kind, start);
    }

    fn newline(&mut self) -> SyntaxKind {
        self.cur.eat_newline();
        self.at_line_start = true;
        if self.bracket_depth == 0 && self.line_has_code {
            self.line_has_code = false;
            return SyntaxKind::LogicalNewline;
        }
        SyntaxKind::Newline
    }

    fn backslash(&mut self, start: u32) -> SyntaxKind {
        self.cur.bump();
        if self.cur.eat_newline() {
            return SyntaxKind::LineContinuation;
        }
        self.error(LexErrorKind::StrayBackslash, start);
        SyntaxKind::Error
    }

    fn ident(&mut self, start: u32) -> SyntaxKind {
        self.cur.eat_while(punct::is_ident_continue);
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
            SyntaxKind::LParen | SyntaxKind::LBracket | SyntaxKind::LBrace => {
                self.bracket_depth = self.bracket_depth.saturating_add(1);
            }
            SyntaxKind::RParen | SyntaxKind::RBracket | SyntaxKind::RBrace => {
                self.bracket_depth = self.bracket_depth.saturating_sub(1);
            }
            _ => {}
        }
        kind
    }

    fn error(&mut self, kind: LexErrorKind, start: u32) {
        let range = TextRange::new(start, self.cur.offset());
        self.out.errors.push(LexError { kind, range });
    }

    fn push(&mut self, kind: SyntaxKind, range: TextRange) {
        self.out.tokens.push(Token::new(kind, range));
    }

    fn push_from(&mut self, kind: SyntaxKind, start: u32) {
        let range = TextRange::new(start, self.cur.offset());
        if !range.is_empty() {
            self.push(kind, range);
        }
    }
}
