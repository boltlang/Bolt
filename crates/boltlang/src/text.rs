use crate::{File, parser::lexer::LineColumn};

const UNICODE_NEWLINE: u32 = 0x000A;
const UNICODE_INVALID: u32 = 0xFFFD;

#[salsa::tracked(debug)]
pub struct DbLineIndex<'db> {
    #[tracked]
    #[returns(ref)]
    pub lines: LineIndex,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LineIndex {
    lines: Vec<usize>
}

impl LineIndex {

    pub fn from_str(text: &str) -> Self {

        let mut lines = Vec::new();
        let mut char_offset = 0;
        let mut iter = text.bytes();

        loop {

            macro_rules! unwrap_byte {
                () => {
                    match iter.next() {
                        None => break,
                        Some(byte) => byte,
                    }
                }
            }

            let code = match unwrap_byte!() {
                b0 if b0 < 0x80 => b0 as u32,
                b0 if (b0 & 0xe0) == 0xc0 => {
                    let b1 = unwrap_byte!();
                      (((b0 & 0x1f) as u32) <<  6)
                    | (((b1 & 0x3f) as u32) <<  0)
                },
                b0 if (b0 & 0xf0) == 0xe0 => {
                    let b1 = unwrap_byte!();
                    let b2 = unwrap_byte!();
                      (((b0 & 0x0f) as u32) << 12)
                    | (((b1 & 0x3f) as u32) <<  6)
                    | (((b2 & 0x3f) as u32) <<  0)
                },
                b0 if (b0 & 0xf8) == 0xf0 && (b0 <= 0xf4) => {
                    let b1 = unwrap_byte!();
                    let b2 = unwrap_byte!();
                    let b3 = unwrap_byte!();
                      (((b0 & 0x07) as u32) << 18)
                    | (((b1 & 0x3f) as u32) << 12)
                    | (((b2 & 0x3f) as u32) <<  6)
                    | (((b3 & 0x3f) as u32) <<  0)
                },
                _ => {
                    // Invalid UTF-8 byte sequence
                    UNICODE_INVALID
                }
            };

            char_offset += 1;
            if code == UNICODE_NEWLINE {
                lines.push(char_offset);
            }

        }

        lines.push(char_offset);

        LineIndex { lines }
    }

    pub fn start_offset_of_line(&self, line: usize) -> usize {
        match line.checked_sub(1) {
            None => 0,
            Some(idx) => self.lines.get(idx).copied().unwrap(),
        }
    }

    /// Return the 0-based line and column that belongs to a given offset.
    ///
    // FIXME should either be 1-based or lexer should be 0-based
    pub fn line_column_of_offset(&self, offset: usize) -> LineColumn {
        let line = self.lines.partition_point(|&it| it <= offset);
        let start = self.start_offset_of_line(line);
        let col = offset - start;
        LineColumn::new(line, col.into())
    }

    pub fn end_offset_of_line(index: LineIndex, line: usize) -> usize {
        index.lines.get(line).copied().unwrap()-1
    }

    /// Return the offset from a 0-based line and column number.
    pub fn offset_from_line_column(&self, lc: &LineColumn) -> usize {
        if lc.line == 0 {
            return lc.column;
        }
        let line_start = self.lines[lc.line-1];
        line_start + lc.column
    }

}

#[salsa::tracked]
pub fn index_lines(db: &dyn salsa::Database, source: File) -> DbLineIndex<'_> {
    let text = source.contents(db);
    DbLineIndex::new(db, LineIndex::from_str(text))
}

#[cfg(test)]
mod test {
    use crate::{parser::lexer::LineColumn, text::LineIndex};

    #[test]
    fn test_get_offset_from_line_column() {
        let index = LineIndex::from_str("one\ntwo\nthree\n");
        assert_eq!(index.offset_from_line_column(&LineColumn::new(0, 0)), 0);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(0, 1)), 1);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(0, 2)), 2);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(0, 3)), 3);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(1, 0)), 4);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(1, 1)), 5);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(1, 2)), 6);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(1, 3)), 7);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(2, 0)), 8);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(2, 1)), 9);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(2, 2)), 10);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(2, 3)), 11);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(2, 4)), 12);
        assert_eq!(index.offset_from_line_column(&LineColumn::new(2, 5)), 13);
    }

}
