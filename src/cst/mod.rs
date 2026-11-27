//! Árbol sintáctico concreto sin pérdida, inspirado en el red-green tree de rust-analyzer (D17).
//!
//! Invariantes:
//! - Todos los tokens del lexer, también la trivia, aparecen exactamente una vez y en orden como hojas
//!   del árbol: concatenar las hojas reproduce el archivo byte a byte.
//! - La memoria es plana: los nodos viven en un `Vec` y los hijos de cada nodo ocupan un tramo
//!   contiguo de otro `Vec`. Los cursores ([`Node`]) toman prestado el árbol; no hay `Rc` ni ciclos.
//! - Los tokens guardan su posición absoluta, así que la capa "roja" (posiciones) es gratuita.

mod build;

pub use build::{build, Event};

use crate::syntax::{SyntaxKind, TextRange, Token};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Child {
    Node(u32),
    Token(u32),
}

#[derive(Copy, Clone, Debug)]
struct NodeData {
    kind: SyntaxKind,
    range: TextRange,
    first_child: u32,
    child_count: u32,
}

#[derive(Debug, Default)]
pub struct Cst {
    nodes: Vec<NodeData>,
    children: Vec<Child>,
    tokens: Vec<Token>,
    root: u32,
}

impl Cst {
    pub fn root(&self) -> Node<'_> {
        Node {
            cst: self,
            id: self.root,
        }
    }

    /// Todos los tokens, trivia incluida, en orden.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Un nodo del árbol: un índice y una referencia prestada al [`Cst`].
#[derive(Copy, Clone, Debug)]
pub struct Node<'a> {
    cst: &'a Cst,
    id: u32,
}

#[derive(Copy, Clone, Debug)]
pub enum Element<'a> {
    Node(Node<'a>),
    Token(Token),
}

impl<'a> Node<'a> {
    fn data(&self) -> Option<&'a NodeData> {
        self.cst.nodes.get(self.id as usize)
    }

    pub fn kind(&self) -> SyntaxKind {
        self.data().map_or(SyntaxKind::Error, |d| d.kind)
    }

    /// Desde el inicio de su primer token hasta el final del último (vacío si no tiene tokens).
    pub fn range(&self) -> TextRange {
        self.data().map_or(TextRange::new(0, 0), |d| d.range)
    }

    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.range().slice(source)
    }

    pub fn children(&self) -> impl Iterator<Item = Element<'a>> + 'a {
        let cst = self.cst;
        let (first, count) = self
            .data()
            .map_or((0, 0), |d| (d.first_child as usize, d.child_count as usize));
        cst.children
            .get(first..first + count)
            .unwrap_or_default()
            .iter()
            .filter_map(move |child| match *child {
                Child::Node(id) => Some(Element::Node(Node { cst, id })),
                Child::Token(t) => cst.tokens.get(t as usize).map(|&t| Element::Token(t)),
            })
    }

    pub fn child_nodes(&self) -> impl Iterator<Item = Node<'a>> + 'a {
        self.children().filter_map(|e| match e {
            Element::Node(n) => Some(n),
            Element::Token(_) => None,
        })
    }

    /// Primer token hijo directo (no trivia) del tipo pedido.
    pub fn token(&self, kind: SyntaxKind) -> Option<Token> {
        self.children().find_map(|e| match e {
            Element::Token(t) if t.kind == kind => Some(t),
            _ => None,
        })
    }

    /// El propio nodo y todos sus descendientes, en preorden.
    pub fn descendants(&self) -> impl Iterator<Item = Node<'a>> + 'a {
        let mut stack = vec![*self];
        std::iter::from_fn(move || {
            let node = stack.pop()?;
            let mut kids: Vec<Node<'a>> = node.child_nodes().collect();
            kids.reverse();
            stack.extend(kids);
            Some(node)
        })
    }

    /// Hojas del subárbol, trivia incluida, en orden.
    pub fn leaves(&self) -> impl Iterator<Item = Token> + 'a {
        let mut stack = vec![Element::Node(*self)];
        std::iter::from_fn(move || loop {
            match stack.pop()? {
                Element::Token(t) => return Some(t),
                Element::Node(n) => {
                    let mut kids: Vec<Element<'a>> = n.children().collect();
                    kids.reverse();
                    stack.extend(kids);
                }
            }
        })
    }
}
