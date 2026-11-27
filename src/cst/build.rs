//! Construcción del [`Cst`] a partir de los eventos del parser.
//!
//! El parser solo ve los tokens significativos y emite `Start`/`Token`/`Finish`. El builder recorre
//! todos los tokens y coloca la trivia: la que precede a un nodo queda en su padre y la que queda al
//! final, en la raíz. La raíz la crea el builder, no el parser, y todo token que el parser no
//! consumió se cuelga de ella: aunque el parser se equivoque, el árbol no pierde ni un byte.

use super::{Child, Cst, NodeData};
use crate::syntax::{SyntaxKind, TextRange, Token};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// Abre un nodo. `forward_parent` apunta a un `Start` posterior que debe abrirse antes que este y
    /// envolverlo: así un nodo ya completado (el `a` de `a + b`) pasa a ser hijo de uno nuevo.
    Start {
        kind: SyntaxKind,
        forward_parent: Option<u32>,
    },
    /// Consume el siguiente token significativo.
    Token,
    Finish,
    /// Un `Start` abandonado o ya procesado.
    Tombstone,
}

struct Frame {
    kind: SyntaxKind,
    children: Vec<Child>,
}

struct Builder {
    cst: Cst,
    next: usize,
    stack: Vec<Frame>,
}

/// Construye el árbol con raíz `root` (`SourceFile`) sobre todos los `tokens` del lexer.
pub fn build(root: SyntaxKind, tokens: Vec<Token>, mut events: Vec<Event>) -> Cst {
    let mut builder = Builder {
        cst: Cst {
            tokens,
            ..Cst::default()
        },
        next: 0,
        stack: vec![Frame {
            kind: root,
            children: Vec::new(),
        }],
    };
    let mut kinds = Vec::new();
    for i in 0..events.len() {
        let Some(event) = events.get_mut(i).map(|e| std::mem::replace(e, Event::Tombstone)) else {
            continue;
        };
        match event {
            Event::Start {
                kind,
                forward_parent,
            } => {
                kinds.push(kind);
                let mut parent = forward_parent;
                while let Some(at) = parent {
                    let slot = events.get_mut(at as usize);
                    parent = match slot.map(|e| std::mem::replace(e, Event::Tombstone)) {
                        Some(Event::Start {
                            kind,
                            forward_parent,
                        }) => {
                            kinds.push(kind);
                            forward_parent
                        }
                        _ => None,
                    };
                }
                for kind in kinds.drain(..).rev() {
                    builder.start(kind);
                }
            }
            Event::Token => builder.token(),
            Event::Finish => builder.finish(),
            Event::Tombstone => {}
        }
    }
    builder.close()
}

impl Builder {
    fn push(&mut self, child: Child) {
        if let Some(frame) = self.stack.last_mut() {
            frame.children.push(child);
        }
    }

    fn push_token(&mut self) {
        let index = u32::try_from(self.next).unwrap_or(u32::MAX);
        self.push(Child::Token(index));
        self.next += 1;
    }

    fn flush_trivia(&mut self) {
        while self
            .cst
            .tokens
            .get(self.next)
            .is_some_and(|t| t.kind.is_trivia())
        {
            self.push_token();
        }
    }

    fn start(&mut self, kind: SyntaxKind) {
        self.flush_trivia();
        self.stack.push(Frame {
            kind,
            children: Vec::new(),
        });
    }

    fn token(&mut self) {
        self.flush_trivia();
        if self.next < self.cst.tokens.len() {
            self.push_token();
        }
    }

    /// Cierra el nodo abierto; la raíz solo se cierra en [`Builder::close`].
    fn finish(&mut self) {
        if self.stack.len() <= 1 {
            return;
        }
        if let Some(frame) = self.stack.pop() {
            let id = self.store(frame);
            self.push(Child::Node(id));
        }
    }

    fn close(mut self) -> Cst {
        while self.stack.len() > 1 {
            self.finish();
        }
        while self.next < self.cst.tokens.len() {
            self.push_token();
        }
        if let Some(root) = self.stack.pop() {
            self.cst.root = self.store(root);
        }
        self.cst
    }

    fn store(&mut self, frame: Frame) -> u32 {
        let range = match (frame.children.first(), frame.children.last()) {
            (Some(first), Some(last)) => {
                TextRange::new(self.child_range(*first).start, self.child_range(*last).end)
            }
            _ => {
                let at = self.cst.tokens.get(self.next).map_or_else(
                    || self.cst.tokens.last().map_or(0, |t| t.range.end),
                    |t| t.range.start,
                );
                TextRange::new(at, at)
            }
        };
        let first_child = u32::try_from(self.cst.children.len()).unwrap_or(u32::MAX);
        let child_count = u32::try_from(frame.children.len()).unwrap_or(u32::MAX);
        self.cst.children.extend(frame.children);
        self.cst.nodes.push(NodeData {
            kind: frame.kind,
            range,
            first_child,
            child_count,
        });
        u32::try_from(self.cst.nodes.len() - 1).unwrap_or(u32::MAX)
    }

    fn child_range(&self, child: Child) -> TextRange {
        let found = match child {
            Child::Token(t) => self.cst.tokens.get(t as usize).map(|t| t.range),
            Child::Node(n) => self.cst.nodes.get(n as usize).map(|n| n.range),
        };
        found.unwrap_or(TextRange::new(0, 0))
    }
}
