use crate::syntax::SyntaxKind::{self, *};

/// Ordenados de mayor a menor longitud: la primera coincidencia es la más larga.
const OPERATORS: &[(&str, SyntaxKind)] = &[
    ("**=", DoubleStarEq),
    ("//=", DoubleSlashEq),
    (">>=", RShiftEq),
    ("<<=", LShiftEq),
    ("...", Ellipsis),
    ("->", Arrow),
    (":=", ColonEq),
    ("==", EqEq),
    ("!=", NotEq),
    ("<=", LtEq),
    (">=", GtEq),
    ("<<", LShift),
    (">>", RShift),
    ("**", DoubleStar),
    ("//", DoubleSlash),
    ("+=", PlusEq),
    ("-=", MinusEq),
    ("*=", StarEq),
    ("/=", SlashEq),
    ("%=", PercentEq),
    ("@=", AtEq),
    ("&=", AmpEq),
    ("|=", PipeEq),
    ("^=", CaretEq),
    ("(", LParen),
    (")", RParen),
    ("[", LBracket),
    ("]", RBracket),
    ("{", LBrace),
    ("}", RBrace),
    (":", Colon),
    (",", Comma),
    (";", Semicolon),
    (".", Dot),
    ("@", At),
    ("=", Eq),
    ("!", Bang),
    ("+", Plus),
    ("-", Minus),
    ("*", Star),
    ("/", Slash),
    ("%", Percent),
    ("|", Pipe),
    ("&", Amp),
    ("^", Caret),
    ("~", Tilde),
    ("<", Lt),
    (">", Gt),
];

pub(super) fn operator(rest: &str) -> Option<(SyntaxKind, &'static str)> {
    OPERATORS
        .iter()
        .find(|(op, _)| rest.starts_with(op))
        .map(|&(op, kind)| (kind, op))
}

/// Superconjunto de los identificadores de Python sin tablas Unicode: todo carácter no ASCII que no
/// sea espacio cuenta. Así el código válido (marcas combinantes, selectores de variante, `℘`, ...)
/// nunca da error; a cambio, un identificador inválido como `€` no se detecta en el lexer.
pub(super) fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic() || (!c.is_ascii() && !c.is_whitespace())
}

pub(super) fn is_ident_continue(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

/// Palabras reservadas duras. Las blandas (`match`, `case`, `type`, `_`) son `Ident` y las decide el parser.
pub(super) fn keyword(text: &str) -> Option<SyntaxKind> {
    Some(match text {
        "False" => KwFalse,
        "None" => KwNone,
        "True" => KwTrue,
        "and" => KwAnd,
        "as" => KwAs,
        "assert" => KwAssert,
        "async" => KwAsync,
        "await" => KwAwait,
        "break" => KwBreak,
        "class" => KwClass,
        "continue" => KwContinue,
        "def" => KwDef,
        "del" => KwDel,
        "elif" => KwElif,
        "else" => KwElse,
        "except" => KwExcept,
        "finally" => KwFinally,
        "for" => KwFor,
        "from" => KwFrom,
        "global" => KwGlobal,
        "if" => KwIf,
        "import" => KwImport,
        "in" => KwIn,
        "is" => KwIs,
        "lambda" => KwLambda,
        "nonlocal" => KwNonlocal,
        "not" => KwNot,
        "or" => KwOr,
        "pass" => KwPass,
        "raise" => KwRaise,
        "return" => KwReturn,
        "try" => KwTry,
        "while" => KwWhile,
        "with" => KwWith,
        "yield" => KwYield,
        _ => return None,
    })
}
