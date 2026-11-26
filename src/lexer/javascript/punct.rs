use crate::syntax::SyntaxKind::{self, *};

/// Ordenados de mayor a menor longitud. `>` va siempre suelto (ver el módulo), por eso no aparecen
/// `>>`, `>=`, `>>>` ni sus asignaciones. Las llaves las gestiona el lexer por su pila de contextos.
const OPERATORS: &[(&str, SyntaxKind)] = &[
    ("**=", DoubleStarEq),
    ("&&=", AmpAmpEq),
    ("||=", PipePipeEq),
    ("??=", QuestionQuestionEq),
    ("===", EqEqEq),
    ("!==", NotEqEq),
    ("...", Ellipsis),
    ("<<=", LShiftEq),
    ("=>", FatArrow),
    ("==", EqEq),
    ("!=", NotEq),
    ("<=", LtEq),
    ("<<", LShift),
    ("**", DoubleStar),
    ("++", PlusPlus),
    ("--", MinusMinus),
    ("+=", PlusEq),
    ("-=", MinusEq),
    ("*=", StarEq),
    ("/=", SlashEq),
    ("%=", PercentEq),
    ("&=", AmpEq),
    ("|=", PipeEq),
    ("^=", CaretEq),
    ("&&", AmpAmp),
    ("||", PipePipe),
    ("??", QuestionQuestion),
    ("?.", QuestionDot),
    ("(", LParen),
    (")", RParen),
    ("[", LBracket),
    ("]", RBracket),
    (";", Semicolon),
    (",", Comma),
    ("<", Lt),
    (">", Gt),
    ("+", Plus),
    ("-", Minus),
    ("*", Star),
    ("/", Slash),
    ("%", Percent),
    ("&", Amp),
    ("|", Pipe),
    ("^", Caret),
    ("!", Bang),
    ("~", Tilde),
    ("?", Question),
    (":", Colon),
    ("=", Eq),
    (".", Dot),
    ("@", At),
];

pub(super) fn operator(rest: &str) -> Option<(SyntaxKind, &'static str)> {
    let optional_chain = rest.starts_with("?.");
    if optional_chain && rest.as_bytes().get(2).is_some_and(u8::is_ascii_digit) {
        return Some((Question, "?"));
    }
    OPERATORS
        .iter()
        .find(|(op, _)| rest.starts_with(op))
        .map(|&(op, kind)| (kind, op))
}

pub(super) fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

pub(super) fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{b}' | '\u{c}' | '\u{a0}' | '\u{feff}')
        || (!c.is_ascii() && c.is_whitespace() && !is_line_terminator(c))
}

/// Superconjunto sin tablas Unicode, como en Python (D10): todo carácter no ASCII que no sea
/// espacio ni fin de línea forma parte de un identificador.
pub(super) fn is_ident_start(c: char) -> bool {
    c == '_'
        || c == '$'
        || c.is_ascii_alphabetic()
        || (!c.is_ascii() && !is_whitespace(c) && !is_line_terminator(c))
}

pub(super) fn is_ident_continue(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

/// Palabras reservadas. Las contextuales (`let`, `static`, `async`, `await`, `yield`, `of`, `get`,
/// `set` y todas las de TypeScript: `type`, `interface`, `as`, ...) son `Ident` y las decide el parser.
pub(super) fn keyword(text: &str) -> Option<SyntaxKind> {
    Some(match text {
        "await" => KwAwait,
        "break" => KwBreak,
        "case" => KwCase,
        "catch" => KwCatch,
        "class" => KwClass,
        "const" => KwConst,
        "continue" => KwContinue,
        "debugger" => KwDebugger,
        "default" => KwDefault,
        "delete" => KwDelete,
        "do" => KwDo,
        "else" => KwElse,
        "enum" => KwEnum,
        "export" => KwExport,
        "extends" => KwExtends,
        "false" => KwFalse,
        "finally" => KwFinally,
        "for" => KwFor,
        "function" => KwFunction,
        "if" => KwIf,
        "import" => KwImport,
        "in" => KwIn,
        "instanceof" => KwInstanceof,
        "new" => KwNew,
        "null" => KwNull,
        "return" => KwReturn,
        "super" => KwSuper,
        "switch" => KwSwitch,
        "this" => KwThis,
        "throw" => KwThrow,
        "true" => KwTrue,
        "try" => KwTry,
        "typeof" => KwTypeof,
        "var" => KwVar,
        "void" => KwVoid,
        "while" => KwWhile,
        "with" => KwWith,
        "yield" => KwYield,
        _ => return None,
    })
}

/// Palabras cuya `(...)` es una condición seguida de una sentencia.
pub(super) fn opens_condition(kind: SyntaxKind) -> bool {
    matches!(kind, KwIf | KwWhile | KwFor | KwWith)
}

/// Tokens tras los cuales `/` divide (y `<` compara) en lugar de abrir una regex (o JSX).
pub(super) fn ends_expression(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        Ident
            | Int
            | Float
            | String
            | Regex
            | TemplateNoSub
            | TemplateTail
            | PrivateName
            | RParen
            | RBracket
            | RBrace
            | PlusPlus
            | MinusMinus
            | KwThis
            | KwSuper
            | KwTrue
            | KwFalse
            | KwNull
    )
}
