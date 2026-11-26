/// Traduce desplazamientos de bytes a números de línea (base 1). `\n`, `\r\n` y `\r` terminan línea.
#[derive(Debug, Clone)]
pub struct LineIndex {
    starts: Vec<u32>,
    len: u32,
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut starts = vec![0];
        for (i, &b) in bytes.iter().enumerate() {
            let ends_line = b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
            if ends_line {
                starts.push(to_u32(i + 1));
            }
        }
        Self {
            starts,
            len: to_u32(text.len()),
        }
    }

    pub fn line(&self, offset: u32) -> u32 {
        to_u32(self.starts.partition_point(|&start| start <= offset).max(1))
    }

    pub fn line_start(&self, line: u32) -> Option<u32> {
        self.starts.get((line as usize).checked_sub(1)?).copied()
    }

    /// Líneas del archivo; un salto final no abre una línea nueva.
    pub fn line_count(&self) -> u32 {
        if self.len == 0 {
            return 0;
        }
        let trailing_newline = self.starts.last() == Some(&self.len);
        to_u32(self.starts.len() - usize::from(trailing_newline))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_one_based_and_handle_all_newlines() {
        let index = LineIndex::new("a\nb\r\nc\rd");
        assert_eq!([0, 2, 5, 7].map(|o| index.line(o)), [1, 2, 3, 4]);
        assert_eq!(index.line(1), 1);
        assert_eq!(index.line_count(), 4);
    }

    #[test]
    fn trailing_newline_does_not_count_as_line() {
        assert_eq!(LineIndex::new("a\nb\n").line_count(), 2);
        assert_eq!(LineIndex::new("").line_count(), 0);
        assert_eq!(LineIndex::new("\n").line_count(), 1);
    }
}
