/// Rango semiabierto `[start, end)` de bytes dentro de un archivo fuente.
///
/// Los desplazamientos son `u32`: los archivos de más de 4 GiB se rechazan antes de lexear.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct TextRange {
    pub start: u32,
    pub end: u32,
}

impl TextRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn empty(at: u32) -> Self {
        Self { start: at, end: at }
    }

    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// Texto del rango; cadena vacía si el rango no cae en límites de carácter de `text`.
    pub fn slice(self, text: &str) -> &str {
        text.get(self.start as usize..self.end as usize)
            .unwrap_or_default()
    }
}
