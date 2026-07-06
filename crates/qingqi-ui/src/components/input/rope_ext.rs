//! Extension trait for [`ropey::Rope`] — implements all line/char/byte conversions
//! from scratch since ropey 2.0.0-beta.1 only provides basic slice/insert/remove.

use std::ops::Range;

use ropey::{Rope, RopeSlice};
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
        Self { row: 0, end_row, rope }
    }
}

impl<'a> Iterator for RopeLines<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.end_row { return None; }
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

/// Count chars in a string slice
fn count_chars(s: &str) -> usize {
    s.chars().count()
}

/// Convert byte index to char index for a string
fn byte_to_char_idx(s: &str, byte_idx: usize) -> usize {
    let byte_idx = byte_idx.min(s.len());
    let mut char_idx = 0;
    for (i, _) in s.char_indices() {
        if i >= byte_idx { break; }
        char_idx += 1;
    }
    char_idx
}

/// Convert char index to byte index for a string
fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    let mut current_char = 0;
    for (byte_idx, _) in s.char_indices() {
        if current_char >= char_idx { return byte_idx; }
        current_char += 1;
    }
    s.len()
}

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
    fn is_char_boundary(&self, offset: usize) -> bool;
}

impl RopeExt for Rope {
    fn line_start_offset(&self, row: usize) -> usize {
        let text = self.to_string();
        let mut current_row = 0;
        for (byte_idx, ch) in text.char_indices() {
            if current_row == row { return byte_idx; }
            if ch == '\n' { current_row += 1; }
        }
        text.len()
    }

    fn line_end_offset(&self, row: usize) -> usize {
        let text = self.to_string();
        let mut current_row = 0;
        for (byte_idx, ch) in text.char_indices() {
            if ch == '\n' {
                if current_row == row { return byte_idx + 1; }
                current_row += 1;
            }
        }
        text.len()
    }

    fn slice_line(&self, row: usize) -> RopeSlice<'_> {
        let start = self.line_start_offset(row);
        let end = self.line_end_offset(row);
        let end = if end > start {
            let text = self.to_string();
            let char_idx = byte_to_char_idx(&text, end - 1);
            if text.chars().nth(char_idx) == Some('\n') { end - 1 } else { end }
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
        let text = self.to_string();
        if text.is_empty() { return 1; }
        text.chars().filter(|c| *c == '\n').count() + 1
    }

    fn line_len(&self, row: usize) -> usize {
        self.slice_line(row).len_chars()
    }

    fn replace(&mut self, range: Range<usize>, new_text: &str) {
        let start = range.start.min(self.len());
        let end = range.end.min(self.len());
        if start >= end {
            if start <= self.len() { self.insert(start, new_text); }
            return;
        }
        self.remove(start..end);
        self.insert(start, new_text);
    }

    fn clip_offset(&self, offset: usize, bias: Bias) -> usize {
        let offset = offset.min(self.len());
        let text = self.to_string();
        if offset >= text.len() { return text.len(); }
        if text.is_char_boundary(offset) { return offset; }
        match bias {
            Bias::Left => {
                let mut best = 0;
                for (byte_idx, _) in text.char_indices() {
                    if byte_idx <= offset { best = byte_idx; } else { break; }
                }
                best
            }
            Bias::Right => {
                for (byte_idx, _) in text.char_indices() {
                    if byte_idx >= offset { return byte_idx; }
                }
                text.len()
            }
        }
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        let text = self.to_string();
        let char_idx = byte_to_char_idx(&text, offset.min(text.len()));
        text.chars().nth(char_idx)
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
        let text = self.to_string();
        let offset = offset.min(text.len());
        let mut utf16_offset = 0;
        for (byte_idx, ch) in text.char_indices() {
            if byte_idx >= offset { break; }
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize {
        let text = self.to_string();
        let mut current_utf16 = 0;
        for (byte_idx, ch) in text.char_indices() {
            if current_utf16 >= offset_utf16 { return byte_idx; }
            current_utf16 += ch.len_utf16();
        }
        text.len()
    }

    fn offset_to_char_index(&self, offset: usize) -> usize {
        let text = self.to_string();
        byte_to_char_idx(&text, offset.min(text.len()))
    }

    fn char_index_to_offset(&self, char_index: usize) -> usize {
        let text = self.to_string();
        char_to_byte_idx(&text, char_index.min(count_chars(&text)))
    }

    fn offset_to_point(&self, offset: usize) -> Point {
        let text = self.to_string();
        let offset = offset.min(text.len());
        let mut row = 0;
        let mut col = 0;
        for (byte_idx, ch) in text.char_indices() {
            if byte_idx >= offset { return Point { row, column: col }; }
            if ch == '\n' { row += 1; col = 0; } else { col += 1; }
        }
        Point { row, column: col }
    }

    fn point_to_offset(&self, point: &Point) -> usize {
        let text = self.to_string();
        let target_row = point.row;
        let target_col = point.column;
        let mut row = 0;
        let mut col = 0;
        for (byte_idx, ch) in text.char_indices() {
            if row == target_row && col == target_col { return byte_idx; }
            if ch == '\n' { row += 1; col = 0; } else { col += 1; }
        }
        text.len()
    }

    fn byte_to_line(&self, byte_idx: usize) -> usize {
        let text = self.to_string();
        let byte_idx = byte_idx.min(text.len());
        let mut row = 0;
        for (i, ch) in text.char_indices() {
            if i >= byte_idx { break; }
            if ch == '\n' { row += 1; }
        }
        row
    }

    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.line_start_offset(line_idx)
    }

    fn byte_to_char(&self, byte_idx: usize) -> usize {
        let text = self.to_string();
        byte_to_char_idx(&text, byte_idx.min(text.len()))
    }

    fn char_to_byte(&self, char_idx: usize) -> usize {
        let text = self.to_string();
        char_to_byte_idx(&text, char_idx.min(count_chars(&text)))
    }

    fn len_chars(&self) -> usize {
        count_chars(&self.to_string())
    }

    fn is_char_boundary(&self, offset: usize) -> bool {
        let text = self.to_string();
        offset <= text.len() && (offset == 0 || text.is_char_boundary(offset))
    }
}


/// Extension trait for RopeSlice to provide len_chars
pub trait RopeSliceExt {
    fn len_chars(&self) -> usize;
    fn len(&self) -> usize;
}

impl RopeSliceExt for RopeSlice<'_> {
    fn len_chars(&self) -> usize {
        self.to_string().chars().count()
    }
    fn len(&self) -> usize {
        self.to_string().len()
    }
}
