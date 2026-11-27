//! Esqueleto estructural de un archivo: funciones (parámetros, anotaciones, complejidad), bloques que
//! capturan errores y construcciones peligrosas, reconocidos sobre los tokens sin el árbol completo.
//!
//! Es la primera capa del parser (D13). Las reglas por función no esperan a la gramática entera, y el
//! reconocimiento es conservador: ante una forma que no entiende, no inventa una función.

mod accumulators;
mod idioms;
mod javascript;
mod js_facts;
mod python;
mod sig;

use std::cmp::Reverse;

use self::sig::Sig;
use crate::arena::Language;
use crate::lexer::Lexed;
use crate::syntax::TextRange;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
    pub name: String,
    /// Tiene anotación de tipo (`x: int`, `x: string`).
    pub annotated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    /// `None` para funciones anónimas sin nombre deducible (callbacks).
    pub name: Option<String>,
    /// Desde la cabecera hasta el final del cuerpo.
    pub span: TextRange,
    /// Parámetros que cuentan para el presupuesto: sin `self`/`cls` ni separadores `*` y `/`.
    pub params: Vec<Param>,
    pub has_return_type: bool,
    /// Complejidad ciclomática: 1 más cada punto de decisión propio (no los de funciones anidadas).
    pub complexity: u32,
    /// Definida dentro de otra función: no es una frontera pública del módulo.
    pub nested: bool,
    first: usize,
    last: usize,
}

impl Function {
    fn new(sig: &Sig, name: Option<String>, (first, last): (usize, usize)) -> Self {
        Self {
            name,
            span: sig.span(first, last),
            params: Vec::new(),
            has_return_type: false,
            complexity: 1,
            nested: false,
            first,
            last,
        }
    }

    fn with_signature(mut self, params: Vec<Param>, has_return_type: bool) -> Self {
        self.params = params;
        self.has_return_type = has_return_type;
        self
    }
}

/// Un bloque que captura errores (`catch`, `except`, `.catch(...)`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Catch {
    pub span: TextRange,
    /// Descarta el error sin hacer nada ni explicar por qué.
    pub silent: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdiomKind {
    /// Python: `== None` o `!= None` en lugar de `is None`.
    NoneEquality,
    /// Python: `nombre = lambda ...` en lugar de `def`.
    LambdaAssignment,
    /// JS/TS: `==` o `!=` que no compara con `null`/`undefined`.
    LooseEquality,
    /// JS/TS: `var` (fuera de `declare var`).
    VarDeclaration,
    /// JS/TS: sentencia `debugger`.
    Debugger,
}

/// Una construcción desaconsejada por las guías del lenguaje, con el rango de su token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Idiom {
    pub kind: IdiomKind,
    pub span: TextRange,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Collection {
    List,
    Set,
    Dict,
}

/// Una colección que se crea vacía y se llena con un bucle que solo inserta, con la expresión
/// funcional equivalente (comprensión, `map`/`filter`) lista para sustituirlo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Accumulator {
    pub name: String,
    /// Desde la declaración de la colección hasta el final del bucle.
    pub span: TextRange,
    pub replacement: String,
    pub collection: Collection,
    /// Condición que el agente debe comprobar antes de aplicar la sustitución.
    pub caveat: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Outline {
    /// Ordenadas por inicio, y a igual inicio la más externa primero: las funciones anidadas en una
    /// ocupan el tramo contiguo que la sigue.
    pub functions: Vec<Function>,
    pub catches: Vec<Catch>,
    /// Llamadas a `eval`, `exec` o `new Function`.
    pub dynamic_evals: Vec<TextRange>,
    /// Usos de `any` en posición de tipo (solo TypeScript).
    pub explicit_any: Vec<TextRange>,
    pub idioms: Vec<Idiom>,
    pub accumulators: Vec<Accumulator>,
}

pub fn outline(source: &str, lexed: &Lexed, language: Language) -> Outline {
    let sig = Sig::new(source, lexed);
    let (mut outline, decision): (Outline, fn(&Sig, usize) -> bool) = match language {
        Language::Python => (python::outline(&sig), python::is_decision),
        Language::JavaScript | Language::TypeScript => (
            javascript::outline(&sig, language == Language::TypeScript),
            js_facts::is_decision,
        ),
    };
    assign_complexity(&mut outline.functions, &sig, decision);
    outline
}

/// Atribuye cada punto de decisión a la función más interna que lo contiene y marca las anidadas.
fn assign_complexity(functions: &mut [Function], sig: &Sig, decision: fn(&Sig, usize) -> bool) {
    functions.sort_by_key(|f| (f.first, Reverse(f.last)));
    let mut stack: Vec<usize> = Vec::new();
    let mut next = 0;
    for i in 0..sig.len() {
        while stack
            .last()
            .and_then(|&f| functions.get(f))
            .is_some_and(|f| f.last < i)
        {
            stack.pop();
        }
        while let Some(function) = functions.get_mut(next) {
            if function.first > i {
                break;
            }
            function.nested = !stack.is_empty();
            stack.push(next);
            next += 1;
        }
        if decision(sig, i) {
            if let Some(function) = stack.last().and_then(|&f| functions.get_mut(f)) {
                function.complexity += 1;
            }
        }
    }
}
