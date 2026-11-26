/// Cursor de bytes sobre UTF-8. `pos` siempre cae en un límite de carácter.
pub(crate) struct Cursor<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    /// El tamaño de la fuente se valida antes de crear el cursor, por eso la conversión no satura.
    pub(crate) fn offset(&self) -> u32 {
        u32::try_from(self.pos).unwrap_or(u32::MAX)
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    pub(crate) fn rest(&self) -> &'a str {
        self.src.get(self.pos..).unwrap_or_default()
    }

    pub(crate) fn slice_from(&self, start: u32) -> &'a str {
        self.src.get(start as usize..self.pos).unwrap_or_default()
    }

    pub(crate) fn first(&self) -> Option<char> {
        self.rest().chars().next()
    }

    pub(crate) fn second(&self) -> Option<char> {
        self.rest().chars().nth(1)
    }

    pub(crate) fn bump(&mut self) -> Option<char> {
        let c = self.first()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    pub(crate) fn eat(&mut self, expected: char) -> bool {
        if self.first() == Some(expected) {
            self.pos += expected.len_utf8();
            return true;
        }
        false
    }

    pub(crate) fn eat_str(&mut self, expected: &str) -> bool {
        if self.rest().starts_with(expected) {
            self.pos += expected.len();
            return true;
        }
        false
    }

    pub(crate) fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while let Some(c) = self.first() {
            if !predicate(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
    }

    /// Consume `\r\n`, `\n` o `\r` como un único salto de línea.
    pub(crate) fn eat_newline(&mut self) -> bool {
        self.eat_str("\r\n") || self.eat('\n') || self.eat('\r')
    }
}
