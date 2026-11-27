//! Parsers de descenso recursivo hacia el [`Cst`], con recuperación de errores.
//!
//! El parser trabaja solo con los tokens significativos y emite eventos; el árbol lo construye
//! [`crate::cst::build`], que coloca la trivia y cuelga de la raíz lo que el parser no consumió.
//! Ante una construcción que no reconoce, abre un nodo `Error`, registra un [`ParseError`] y avanza:
//! nunca entra en pánico. Un presupuesto de pasos proporcional al número de tokens garantiza que
//! termina aunque una regla de la gramática deje de avanzar.

mod javascript;
mod python;

use std::path::Path;

use crate::arena::Language;
use crate::cst::{self, Cst, Event};
use crate::lexer::javascript::Dialect;
use crate::lexer::Lexed;
use crate::syntax::{SyntaxKind, TextRange, Token};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub message: &'static str,
    pub range: TextRange,
}

#[derive(Debug)]
pub struct Parsed {
    pub cst: Cst,
    pub errors: Vec<ParseError>,
}

/// Parsea un archivo ya lexeado; `path` decide el dialecto (TypeScript, JSX) como en el lexer.
pub fn parse(source: &str, lexed: &Lexed, language: Language, path: &Path) -> Parsed {
    let mut parser = Parser::new(source, &lexed.tokens);
    match language {
        Language::Python => python::file(&mut parser),
        Language::JavaScript | Language::TypeScript => {
            parser.typescript = language == Language::TypeScript;
            parser.jsx = Dialect::from_path(path).jsx;
            javascript::file(&mut parser);
        }
    }
    Parsed {
        cst: cst::build(SyntaxKind::SourceFile, lexed.tokens.clone(), parser.events),
        errors: parser.errors,
    }
}

/// Pasos de lectura permitidos por token: de sobra para cualquier gramática que avance.
const STEPS_PER_TOKEN: usize = 64;

pub(crate) struct Parser<'t> {
    source: &'t str,
    tokens: Vec<Token>,
    /// Para cada `(`, `[` o `{`, el índice de su cierre.
    close: Vec<Option<usize>>,
    pos: usize,
    events: Vec<Event>,
    errors: Vec<ParseError>,
    steps: usize,
    max_steps: usize,
    /// Dentro de los patrones de un `case` de Python, donde `x as nombre` vale en cualquier nivel.
    pub(crate) in_pattern: bool,
    pub(crate) typescript: bool,
    pub(crate) jsx: bool,
    /// En la inicialización de un `for`, donde `in` separa y no es un operador.
    pub(crate) no_in: bool,
}

/// Estado al que [`Parser::rewind`] devuelve el parser tras un intento especulativo fallido.
#[derive(Copy, Clone)]
pub(crate) struct Checkpoint {
    pos: usize,
    events: usize,
    errors: usize,
}

impl<'t> Parser<'t> {
    fn new(source: &'t str, all: &[Token]) -> Self {
        let tokens: Vec<Token> = all
            .iter()
            .copied()
            .filter(|t| !t.kind.is_trivia())
            .collect();
        let close = match_brackets(&tokens);
        let max_steps = tokens.len().saturating_mul(STEPS_PER_TOKEN) + 1024;
        Self {
            source,
            tokens,
            close,
            pos: 0,
            events: Vec::new(),
            errors: Vec::new(),
            steps: 0,
            max_steps,
            in_pattern: false,
            typescript: false,
            jsx: false,
            no_in: false,
        }
    }

    /// Tipo del token `n` posiciones por delante; `EndOfFile` al final o si se agotó el presupuesto.
    pub(crate) fn nth(&mut self, n: usize) -> SyntaxKind {
        self.steps += 1;
        if self.steps > self.max_steps {
            return SyntaxKind::EndOfFile;
        }
        self.tokens
            .get(self.pos + n)
            .map_or(SyntaxKind::EndOfFile, |t| t.kind)
    }

    pub(crate) fn current(&mut self) -> SyntaxKind {
        self.nth(0)
    }

    pub(crate) fn at(&mut self, kind: SyntaxKind) -> bool {
        self.current() == kind
    }

    pub(crate) fn nth_text(&self, n: usize) -> &'t str {
        self.tokens
            .get(self.pos + n)
            .map_or("", |t| t.text(self.source))
    }

    /// Identificador con un texto dado: las palabras clave contextuales (`match`, `type`, `of`).
    pub(crate) fn at_word(&mut self, word: &str) -> bool {
        self.at(SyntaxKind::Ident) && self.nth_text(0) == word
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Índice absoluto del cierre del grupo que se abre en `at`.
    pub(crate) fn close_of(&self, at: usize) -> Option<usize> {
        self.close.get(at).copied().flatten()
    }

    pub(crate) fn text_at(&self, at: usize) -> &'t str {
        self.tokens.get(at).map_or("", |t| t.text(self.source))
    }

    pub(crate) fn kind_at(&self, at: usize) -> SyntaxKind {
        self.tokens
            .get(at)
            .map_or(SyntaxKind::EndOfFile, |t| t.kind)
    }

    /// Hay un salto de línea entre el token anterior y el actual (inserción automática de `;`).
    pub(crate) fn newline_before(&self) -> bool {
        self.newline_before_nth(0)
    }

    /// Hay un salto de línea justo antes del token `n` posiciones por delante.
    pub(crate) fn newline_before_nth(&self, n: usize) -> bool {
        let at = self.pos + n;
        let (Some(prev), Some(cur)) = (
            at.checked_sub(1).and_then(|p| self.tokens.get(p)),
            self.tokens.get(at),
        ) else {
            return false;
        };
        TextRange::new(prev.range.end, cur.range.start.max(prev.range.end))
            .slice(self.source)
            .contains(['\n', '\r'])
    }

    /// El token `n` empieza justo donde acaba el `n - 1`, sin trivia entre ambos.
    pub(crate) fn adjacent(&self, n: usize) -> bool {
        let at = self.pos + n;
        match (at.checked_sub(1).and_then(|p| self.tokens.get(p)), self.tokens.get(at)) {
            (Some(a), Some(b)) => a.range.end == b.range.start,
            _ => false,
        }
    }

    /// Texto fuente que sigue al token `n` posiciones por delante.
    pub(crate) fn source_after(&self, n: usize) -> &'t str {
        self.tokens
            .get(self.pos + n)
            .and_then(|t| self.source.get(t.range.end as usize..))
            .unwrap_or_default()
    }

    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            events: self.events.len(),
            errors: self.errors.len(),
        }
    }

    pub(crate) fn rewind(&mut self, to: Checkpoint) {
        self.pos = to.pos;
        self.events.truncate(to.events);
        self.errors.truncate(to.errors);
    }

    /// Errores registrados desde `since`: un intento especulativo con errores se descarta.
    pub(crate) fn errors_since(&self, since: Checkpoint) -> bool {
        self.errors.len() > since.errors
    }

    pub(crate) fn bump(&mut self) {
        if self.pos < self.tokens.len() {
            self.events.push(Event::Token);
            self.pos += 1;
        }
    }

    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        let found = self.at(kind);
        if found {
            self.bump();
        }
        found
    }

    pub(crate) fn expect(&mut self, kind: SyntaxKind, message: &'static str) -> bool {
        let found = self.eat(kind);
        if !found {
            self.error(message);
        }
        found
    }

    pub(crate) fn error(&mut self, message: &'static str) {
        let range = self
            .tokens
            .get(self.pos)
            .or(self.tokens.last())
            .map_or(TextRange::new(0, 0), |t| t.range);
        self.errors.push(ParseError { message, range });
    }

    /// Registra el error y consume el token actual dentro de un nodo `Error`.
    pub(crate) fn error_and_bump(&mut self, message: &'static str) {
        self.error(message);
        let m = self.start();
        self.bump();
        m.complete(self, SyntaxKind::Error);
    }

    /// Consume como `Error` todo lo que quede antes del índice `until`.
    pub(crate) fn skip_to(&mut self, until: usize, message: &'static str) {
        if self.pos >= until {
            return;
        }
        self.error(message);
        let m = self.start();
        while self.pos < until && self.current() != SyntaxKind::EndOfFile {
            self.bump();
        }
        m.complete(self, SyntaxKind::Error);
    }

    /// Cierra con `close` el grupo abierto en el índice `open`, descartando lo que quede dentro.
    pub(crate) fn close_group(&mut self, open: usize, close: SyntaxKind) {
        if let Some(end) = self.close_of(open) {
            self.skip_to(end, "contenido inesperado antes del cierre");
        }
        self.expect(close, "falta el cierre del grupo");
    }

    pub(crate) fn start(&mut self) -> Marker {
        let at = u32::try_from(self.events.len()).unwrap_or(u32::MAX);
        self.events.push(Event::Tombstone);
        Marker { at }
    }
}

/// Un nodo abierto. Se cierra con [`Marker::complete`] o se descarta con [`Marker::abandon`].
#[must_use]
pub(crate) struct Marker {
    at: u32,
}

impl Marker {
    pub(crate) fn complete(self, p: &mut Parser, kind: SyntaxKind) -> Completed {
        if let Some(slot) = p.events.get_mut(self.at as usize) {
            *slot = Event::Start {
                kind,
                forward_parent: None,
            };
        }
        p.events.push(Event::Finish);
        Completed { at: self.at }
    }

    /// Descarta el nodo: sus hijos pasan al padre.
    pub(crate) fn abandon(self, _p: &mut Parser) {}
}

#[derive(Copy, Clone)]
pub(crate) struct Completed {
    at: u32,
}

impl Completed {
    /// Abre un nodo nuevo que contendrá a este: `a` pasa a ser el primer hijo de `a + b`.
    pub(crate) fn precede(self, p: &mut Parser) -> Marker {
        let parent = p.start();
        if let Some(Event::Start { forward_parent, .. }) = p.events.get_mut(self.at as usize) {
            *forward_parent = Some(parent.at);
        }
        parent
    }
}

fn match_brackets(tokens: &[Token]) -> Vec<Option<usize>> {
    use SyntaxKind::{LBrace, LBracket, LParen, RBrace, RBracket, RParen};
    let mut close = vec![None; tokens.len()];
    let mut open: Vec<(SyntaxKind, usize)> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let expected = match token.kind {
            LParen | LBracket | LBrace => {
                open.push((token.kind, i));
                continue;
            }
            RParen => LParen,
            RBracket => LBracket,
            RBrace => LBrace,
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
