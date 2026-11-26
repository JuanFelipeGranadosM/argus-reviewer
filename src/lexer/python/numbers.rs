use crate::lexer::cursor::Cursor;
use crate::syntax::SyntaxKind;

fn is_digit_part(c: char) -> bool {
    c.is_ascii_digit() || c == '_'
}

/// Precondición: el cursor está en un dígito, o en `.` seguido de dígito.
pub(super) fn lex_number(cur: &mut Cursor) -> SyntaxKind {
    let radix_prefix = cur.first() == Some('0')
        && cur
            .second()
            .is_some_and(|c| matches!(c.to_ascii_lowercase(), 'x' | 'o' | 'b'));
    if radix_prefix {
        cur.bump();
        cur.bump();
        cur.eat_while(|c| c.is_ascii_hexdigit() || c == '_');
        return SyntaxKind::Int;
    }
    let mut kind = SyntaxKind::Int;
    cur.eat_while(is_digit_part);
    if cur.eat('.') {
        cur.eat_while(is_digit_part);
        kind = SyntaxKind::Float;
    }
    if exponent_ahead(cur) {
        cur.bump();
        if !cur.eat('+') {
            cur.eat('-');
        }
        cur.eat_while(is_digit_part);
        kind = SyntaxKind::Float;
    }
    if cur.eat('j') || cur.eat('J') {
        kind = SyntaxKind::Imaginary;
    }
    kind
}

fn exponent_ahead(cur: &Cursor) -> bool {
    let mut chars = cur.rest().chars();
    if !matches!(chars.next(), Some('e' | 'E')) {
        return false;
    }
    match chars.next() {
        Some('+' | '-') => chars.next().is_some_and(|c| c.is_ascii_digit()),
        Some(c) => c.is_ascii_digit(),
        None => false,
    }
}
