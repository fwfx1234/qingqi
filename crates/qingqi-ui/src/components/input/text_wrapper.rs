//! Text wrapping engine for multi-line input fields.

use std::ops::Range;

use gpui::{App, Font, LineFragment, Pixels, ShapedLine, px};
use ropey::Rope;
use smallvec::SmallVec;

use super::RopeExt;

#[derive(Debug, Clone)]
pub(super) struct LineItem {
    pub(super) line: Rope,
    pub(super) wrapped_lines: Vec<Range<usize>>,
}

impl LineItem {
    pub(super) fn lines_len(&self) -> usize {
        self.wrapped_lines.len()
    }
}

#[derive(Debug, Default)]
pub(super) struct LongestRow {
    pub len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPoint {
    pub row: usize,
    pub local_row: usize,
    pub column: usize,
}

impl DisplayPoint {
    pub fn new(row: usize, local_row: usize, column: usize) -> Self {
        Self {
            row,
            local_row,
            column,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LineLayout {
    pub wrapped_lines: SmallVec<[ShapedLine; 1]>,
    pub len: usize,
}

impl LineLayout {
    pub fn new() -> Self {
        Self {
            wrapped_lines: SmallVec::new(),
            len: 0,
        }
    }

    pub fn set_wrapped_lines(&mut self, lines: SmallVec<[ShapedLine; 1]>) {
        self.len = lines.iter().map(|l| l.len).sum();
        self.wrapped_lines = lines;
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.wrapped_lines.is_empty()
    }
}

pub struct TextWrapper {
    text: Rope,
    pub(super) soft_lines: usize,
    font: Font,
    font_size: Pixels,
    wrap_width: Option<Pixels>,
    pub(super) longest_row: LongestRow,
    pub(super) lines: Vec<LineItem>,
    _initialized: bool,
}

impl TextWrapper {
    pub fn new(font: Font, font_size: Pixels, wrap_width: Option<Pixels>) -> Self {
        Self {
            text: Rope::new(),
            font,
            font_size,
            wrap_width,
            soft_lines: 0,
            longest_row: LongestRow::default(),
            lines: Vec::new(),
            _initialized: false,
        }
    }

    pub fn set_default_text(&mut self, text: &Rope) {
        self.text = text.clone();
    }
    pub fn len(&self) -> usize {
        self.soft_lines
    }

    pub(super) fn line(&self, row: usize) -> Option<&LineItem> {
        self.lines.iter().skip(row).next()
    }

    pub fn set_wrap_width(&mut self, wrap_width: Option<Pixels>, cx: &mut App) {
        if wrap_width == self.wrap_width {
            return;
        }
        self.wrap_width = wrap_width;
        self.update_all(&self.text.clone(), cx);
    }

    pub fn set_font(&mut self, font: Font, font_size: Pixels, cx: &mut App) {
        if self.font.eq(&font) && self.font_size == font_size {
            return;
        }
        self.font = font;
        self.font_size = font_size;
        self.update_all(&self.text.clone(), cx);
    }

    pub fn prepare_if_need(&mut self, text: &Rope, cx: &mut App) {
        if self._initialized {
            return;
        }
        self._initialized = true;
        self.update_all(text, cx);
    }

    /// Rebuild all wrapping metadata after a programmatic or masked edit.
    pub fn reset(&mut self, text: &Rope, cx: &mut App) {
        self._initialized = true;
        self.update_all(text, cx);
    }

    pub fn update(
        &mut self,
        changed_text: &Rope,
        range: &Range<usize>,
        new_text: &Rope,
        cx: &mut App,
    ) {
        let mut line_wrapper = cx
            .text_system()
            .line_wrapper(self.font.clone(), self.font_size);
        self._update(
            changed_text,
            range,
            new_text,
            &mut |line_str, wrap_width| {
                line_wrapper
                    .wrap_line(&[LineFragment::text(line_str)], wrap_width)
                    .collect::<Vec<_>>()
            },
        );
    }

    fn _update<F>(
        &mut self,
        changed_text: &Rope,
        range: &Range<usize>,
        new_text: &Rope,
        wrap_line: &mut F,
    ) where
        F: FnMut(&str, Pixels) -> Vec<gpui::Boundary>,
    {
        let start_row = changed_text.offset_to_point(range.start).row;
        let start_row = start_row.min(self.lines.len().saturating_sub(1));
        let end_row = changed_text.offset_to_point(range.end).row;
        let end_row = end_row.min(self.lines.len());

        let old_lines_count: usize = self.lines[start_row..end_row]
            .iter()
            .map(|l| l.lines_len())
            .sum();
        self.soft_lines = self.soft_lines.saturating_sub(old_lines_count);
        self.lines.drain(start_row..end_row);

        let wrap_width = self.wrap_width.unwrap_or(px(100000.0));
        let start_row_offset = changed_text.offset_to_point(range.start).row;

        for row in start_row_offset
            ..=changed_text
                .offset_to_point(range.start + new_text.len())
                .row
        {
            let line = changed_text.slice_line(row);
            let line_str = line.to_string();
            let wrapped = wrap_line(&line_str, wrap_width);
            let wrapped_lines = boundaries_to_ranges(&wrapped, line_str.len());
            self.soft_lines += wrapped_lines.len().max(1);

            if line.len() > self.longest_row.len {
                self.longest_row = LongestRow { len: line.len() };
            }

            self.lines.insert(
                start_row + (row - start_row_offset),
                LineItem {
                    line: Rope::from(line.to_string()),
                    wrapped_lines,
                },
            );
        }
    }

    fn update_all(&mut self, text: &Rope, cx: &mut App) {
        self.text = text.clone();
        self.lines.clear();
        self.soft_lines = 0;

        let wrap_width = self.wrap_width.unwrap_or(px(100000.0));
        let mut line_wrapper = cx
            .text_system()
            .line_wrapper(self.font.clone(), self.font_size);

        for row in 0..text.lines_len() {
            let line = text.slice_line(row);
            let line_str = line.to_string();
            let fragment = LineFragment::text(&line_str);
            let fragments = [fragment];
            let wrapped = line_wrapper.wrap_line(&fragments, wrap_width);
            let wrapped_lines = boundaries_to_ranges(&wrapped.collect::<Vec<_>>(), line_str.len());

            self.soft_lines += wrapped_lines.len().max(1);

            if line.len() > self.longest_row.len {
                self.longest_row = LongestRow { len: line.len() };
            }

            self.lines.push(LineItem {
                line: Rope::from(line.to_string()),
                wrapped_lines,
            });
        }

        if self.lines.is_empty() {
            self.lines.push(LineItem {
                line: Rope::new(),
                wrapped_lines: vec![0..0],
            });
            self.soft_lines = 1;
        }
    }

    pub fn offset_to_display_point(&self, offset: usize) -> DisplayPoint {
        let mut remaining = offset;
        for (row, line_item) in self.lines.iter().enumerate() {
            let line_end = line_item.line.len();
            if remaining <= line_end {
                let mut col = remaining;
                for (local_idx, wrap_range) in line_item.wrapped_lines.iter().enumerate() {
                    if col < wrap_range.len() {
                        return DisplayPoint::new(row, local_idx, col);
                    }
                    col -= wrap_range.len();
                }
                let last_local = line_item.wrapped_lines.len().saturating_sub(1);
                return DisplayPoint::new(row, last_local, col);
            }
            remaining -= line_end + 1;
        }
        let last_row = self.lines.len().saturating_sub(1);
        if let Some(last_line) = self.lines.last() {
            DisplayPoint::new(
                last_row,
                last_line.wrapped_lines.len().saturating_sub(1),
                last_line.line.len(),
            )
        } else {
            DisplayPoint::new(0, 0, 0)
        }
    }

    pub fn display_point_to_offset(&self, point: DisplayPoint) -> usize {
        let mut offset = 0;
        for (row, line_item) in self.lines.iter().enumerate() {
            if row == point.row {
                let mut col = 0;
                for (local_idx, wrap_range) in line_item.wrapped_lines.iter().enumerate() {
                    if local_idx == point.local_row {
                        return offset + col + point.column.min(wrap_range.len());
                    }
                    col += wrap_range.len();
                }
                return offset + col;
            }
            offset += line_item.line.len() + 1;
        }
        self.text.len()
    }

    pub fn display_point_to_point(&self, point: DisplayPoint) -> super::rope_ext::Point {
        let offset = self.display_point_to_offset(point);
        self.text.offset_to_point(offset)
    }

    pub fn display_row_for_offset(&self, offset: usize) -> usize {
        let point = self.offset_to_display_point(offset);
        self.lines
            .iter()
            .take(point.row)
            .map(LineItem::lines_len)
            .sum::<usize>()
            + point.local_row
    }
}

fn boundaries_to_ranges(boundaries: &[gpui::Boundary], line_len: usize) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for boundary in boundaries {
        let end = boundary.ix;
        if end > start {
            ranges.push(start..end);
        }
        start = end;
    }
    if start < line_len {
        ranges.push(start..line_len);
    }
    if ranges.is_empty() {
        ranges.push(0..0);
    }
    ranges
}
