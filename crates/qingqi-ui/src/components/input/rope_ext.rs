//! Extension trait for [`ropey::Rope`] — implements all line/char/byte conversions
//! from scratch since ropey 2.0.0-beta.1 only provides basic slice/insert/remove.

use std::ops::Range;

use ropey::{LineType, Rope, RopeSlice};
use sum_tree::Bias;

/// Simple Point type for row/column positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}

impl Point {
    pub fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

/// An iterator over the lines of a `Rope`.
pub struct RopeLines<'a> {
    rope: &'a Rope,
    row: usize,
    end_row: usize,
}

impl<'a> RopeLines<'a> {
    pub fn new(rope: &'a Rope) -> Self {
        let end_row = rope.lines_len();
        Self {
            row: 0,
            end_row,
            rope,
        }
    }
}

impl<'a> Iterator for RopeLines<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.end_row {
            return None;
        }
        let line = self.rope.slice_line(self.row);
        self.row += 1;
        Some(line)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.row = self.row.saturating_add(n);
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end_row - self.row;
        (len, Some(len))
    }
}

impl std::iter::ExactSizeIterator for RopeLines<'_> {}
impl std::iter::FusedIterator for RopeLines<'_> {}

/// Extension trait providing line/char/byte conversions for Rope.
pub trait RopeExt {
    fn line_start_offset(&self, row: usize) -> usize;
    fn line_end_offset(&self, row: usize) -> usize;
    fn slice_line(&self, row: usize) -> RopeSlice<'_>;
    fn slice_lines(&self, rows_range: Range<usize>) -> RopeSlice<'_>;
    fn iter_lines(&self) -> RopeLines<'_>;
    fn lines_len(&self) -> usize;
    fn line_len(&self, row: usize) -> usize;
    fn replace(&mut self, range: Range<usize>, new_text: &str);
    fn clip_offset(&self, offset: usize, bias: Bias) -> usize;
    fn char_at(&self, offset: usize) -> Option<char>;
    fn word_at(&self, offset: usize) -> String;
    fn word_range(&self, offset: usize) -> Option<Range<usize>>;
    fn offset_to_offset_utf16(&self, offset: usize) -> usize;
    fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize;
    fn offset_to_char_index(&self, offset: usize) -> usize;
    fn char_index_to_offset(&self, char_index: usize) -> usize;
    fn offset_to_point(&self, offset: usize) -> Point;
    fn point_to_offset(&self, point: &Point) -> usize;
    fn byte_to_line(&self, byte_idx: usize) -> usize;
    fn line_to_byte(&self, line_idx: usize) -> usize;
    fn byte_to_char(&self, byte_idx: usize) -> usize;
    fn char_to_byte(&self, char_idx: usize) -> usize;
    fn len_chars(&self) -> usize;
}

impl RopeExt for Rope {
    fn line_start_offset(&self, row: usize) -> usize {
        self.line_to_byte_idx(row, LineType::LF_CR)
    }

    fn line_end_offset(&self, row: usize) -> usize {
        let next_row = (row + 1).min(self.len_lines(LineType::LF_CR));
        self.line_to_byte_idx(next_row, LineType::LF_CR)
    }

    fn slice_line(&self, row: usize) -> RopeSlice<'_> {
        let start = self.line_start_offset(row);
        let end = self.line_end_offset(row);
        // Strip trailing newline if present (P2: guard against end == 0)
        let end = if end > start && end > 0 {
            let byte_at_end_minus_1 = self.byte(end - 1);
            if byte_at_end_minus_1 == b'\n' {
                if end >= 2 && self.byte(end - 2) == b'\r' {
                    end - 2
                } else {
                    end - 1
                }
            } else if byte_at_end_minus_1 == b'\r' {
                end - 1
            } else {
                end
            }
        } else {
            end
        };
        self.slice(start..end)
    }

    fn slice_lines(&self, rows_range: Range<usize>) -> RopeSlice<'_> {
        let start = self.line_start_offset(rows_range.start);
        let end_row = rows_range.end.min(self.lines_len());
        let end = if end_row < self.lines_len() {
            self.line_start_offset(end_row)
        } else {
            self.len()
        };
        self.slice(start..end)
    }

    fn iter_lines(&self) -> RopeLines<'_> {
        RopeLines::new(self)
    }

    fn lines_len(&self) -> usize {
        let len = self.len_lines(LineType::LF_CR);
        if self.len() == 0 { 1 } else { len }
    }

    fn line_len(&self, row: usize) -> usize {
        self.slice_line(row).len_chars()
    }

    fn replace(&mut self, range: Range<usize>, new_text: &str) {
        let start = range.start.min(self.len());
        let end = range.end.min(self.len());
        if start >= end {
            if start <= self.len() {
                self.insert(start, new_text);
            }
            return;
        }
        self.remove(start..end);
        self.insert(start, new_text);
    }

    fn clip_offset(&self, offset: usize, bias: Bias) -> usize {
        let offset = offset.min(self.len());
        if offset >= self.len() {
            return self.len();
        }
        // Check if offset is at a char boundary using ropey's native method
        if self.is_char_boundary(offset) {
            return offset;
        }
        match bias {
            Bias::Left => {
                let mut best = 0;
                for (byte_idx, _) in self.char_indices() {
                    if byte_idx <= offset {
                        best = byte_idx;
                    } else {
                        break;
                    }
                }
                best
            }
            Bias::Right => {
                for (byte_idx, _) in self.char_indices() {
                    if byte_idx >= offset {
                        return byte_idx;
                    }
                }
                self.len()
            }
        }
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        let offset = offset.min(self.len());
        if !self.is_char_boundary(offset) {
            return None;
        }
        self.chars_at(offset).next()
    }

    fn word_at(&self, offset: usize) -> String {
        match self.word_range(offset) {
            Some(r) => self.slice(r).to_string(),
            None => String::new(),
        }
    }

    fn word_range(&self, offset: usize) -> Option<Range<usize>> {
        use unicode_segmentation::UnicodeSegmentation;
        let text = self.to_string();
        let offset = self.clip_offset(offset, Bias::Left);
        let mut current_offset = 0;
        for g in text.graphemes(true) {
            let g_end = current_offset + g.len();
            if current_offset <= offset && offset < g_end {
                return Some(current_offset..g_end);
            }
            current_offset = g_end;
        }
        None
    }

    fn offset_to_offset_utf16(&self, offset: usize) -> usize {
        let offset = offset.min(self.len());
        let mut utf16_offset = 0;
        for (byte_idx, ch) in self.char_indices() {
            if byte_idx >= offset {
                break;
            }
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize {
        let mut current_utf16 = 0;
        for (byte_idx, ch) in self.char_indices() {
            if current_utf16 >= offset_utf16 {
                return byte_idx;
            }
            current_utf16 += ch.len_utf16();
        }
        self.len()
    }

    fn offset_to_char_index(&self, offset: usize) -> usize {
        let offset = offset.min(self.len());
        let mut char_idx = 0;
        for (byte_idx, _) in self.char_indices() {
            if byte_idx >= offset {
                break;
            }
            char_idx += 1;
        }
        char_idx
    }

    fn char_index_to_offset(&self, char_index: usize) -> usize {
        let mut current_char = 0;
        for (byte_idx, _) in self.char_indices() {
            if current_char >= char_index {
                return byte_idx;
            }
            current_char += 1;
        }
        self.len()
    }

    fn offset_to_point(&self, offset: usize) -> Point {
        let offset = offset.min(self.len());
        let row = self.byte_to_line_idx(offset, LineType::LF_CR);
        let line_start = self.line_to_byte_idx(row, LineType::LF_CR);
        let col = self
            .char_indices_at(line_start)
            .take_while(|(b, _)| *b < offset)
            .count();
        Point { row, column: col }
    }

    fn point_to_offset(&self, point: &Point) -> usize {
        let target_row = point.row;
        let target_col = point.column;
        let line_start = self.line_to_byte_idx(target_row, LineType::LF_CR);
        let mut col = 0;
        for (byte_idx, _) in self.char_indices_at(line_start) {
            if col == target_col {
                return byte_idx;
            }
            col += 1;
        }
        self.len()
    }

    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.byte_to_line_idx(byte_idx.min(self.len()), LineType::LF_CR)
    }

    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.line_to_byte_idx(line_idx, LineType::LF_CR)
    }

    fn byte_to_char(&self, byte_idx: usize) -> usize {
        let byte_idx = byte_idx.min(self.len());
        let mut char_idx = 0;
        for (b, _) in self.char_indices() {
            if b >= byte_idx {
                break;
            }
            char_idx += 1;
        }
        char_idx
    }

    fn char_to_byte(&self, char_idx: usize) -> usize {
        let mut current_char = 0;
        for (byte_idx, _) in self.char_indices() {
            if current_char >= char_idx {
                return byte_idx;
            }
            current_char += 1;
        }
        self.len()
    }

    fn len_chars(&self) -> usize {
        let mut count = 0;
        for _ in self.chars() {
            count += 1;
        }
        count
    }
}

/// Extension trait for RopeSlice to provide len_chars
pub trait RopeSliceExt {
    fn len_chars(&self) -> usize;
}

impl RopeSliceExt for RopeSlice<'_> {
    fn len_chars(&self) -> usize {
        self.chars().count()
    }
}
