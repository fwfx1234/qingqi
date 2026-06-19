//! 终端 grid 渲染（行级缓存 + 选区 overlay）

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use gpui::*;
use qingqi_ui::ui;

use super::TerminalViewModel;
use crate::terminal::TerminalCell;

use super::terminal_pane::{CHAR_WIDTH_RATIO, LINE_HEIGHT_RATIO, TERM_FONT};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridPoint {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalSelection {
    pub anchor: GridPoint,
    pub head: GridPoint,
}

impl TerminalSelection {
    pub fn normalized(&self) -> (GridPoint, GridPoint) {
        if self.anchor.row < self.head.row
            || (self.anchor.row == self.head.row && self.anchor.col <= self.head.col)
        {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

pub fn copy_selection_text(term: &TerminalViewModel, selection: &TerminalSelection) -> String {
    let (start, end) = selection.normalized();
    let mut out = String::new();
    for (row_idx, row) in term.grid.iter().enumerate() {
        if row_idx < start.row || row_idx > end.row {
            continue;
        }
        let col_start = if row_idx == start.row { start.col } else { 0 };
        let col_end = if row_idx == end.row {
            end.col
        } else {
            row_display_width(row).saturating_sub(1)
        };

        let mut col = 0usize;
        for cell in row {
            let width = cell_display_width(cell.ch);
            let cell_end = col.saturating_add(width.saturating_sub(1));
            if cell_end >= col_start && col <= col_end {
                out.push(cell.ch);
            }
            col += width;
        }
        if row_idx < end.row {
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

pub fn cell_at(
    position: Point<Pixels>,
    origin: Point<Pixels>,
    char_width: Pixels,
    line_height: Pixels,
    rows: usize,
    cols: usize,
) -> Option<GridPoint> {
    let rel = position - origin;
    if rel.x < px(0.0) || rel.y < px(0.0) {
        return None;
    }
    let row = (rel.y / line_height).floor() as usize;
    let col = (rel.x / char_width).floor() as usize;
    if row >= rows || col >= cols {
        return None;
    }
    Some(GridPoint { row, col })
}

fn hsla_from_cell(color: Option<[f32; 4]>, fallback: Hsla) -> Hsla {
    color
        .map(|[h, s, l, a]| hsla(h, s, l, a))
        .unwrap_or(fallback)
}

fn default_fg() -> Hsla {
    hsla(0.0, 0.0, 0.12, 1.0)
}

fn default_bg() -> Hsla {
    hsla(0.0, 0.0, 0.99, 1.0)
}

fn selection_bg() -> Hsla {
    hsla(0.58, 0.55, 0.75, 1.0)
}

#[derive(Clone, PartialEq)]
struct StyleKey {
    fg: Option<[f32; 4]>,
    bg: Option<[f32; 4]>,
    bold: bool,
}

fn cell_display_width(ch: char) -> usize {
    if ch.is_ascii() { 1 } else { 2 }
}

fn row_display_width(cells: &[TerminalCell]) -> usize {
    cells.iter().map(|c| cell_display_width(c.ch)).sum()
}

/// 将 alacritty 网格列号映射为渲染文本中的 UTF-8 字节偏移。
fn grid_col_to_byte_offset(cells: &[TerminalCell], target_col: usize) -> usize {
    let mut col = 0usize;
    let mut bytes = 0usize;
    for cell in cells {
        if target_col <= col {
            return bytes;
        }
        let width = cell_display_width(cell.ch);
        if target_col < col + width {
            return bytes + cell.ch.len_utf8();
        }
        bytes += cell.ch.len_utf8();
        col += width;
    }
    bytes
}

fn grid_col_range_to_byte_range(
    cells: &[TerminalCell],
    col_start: usize,
    col_end: usize,
) -> Range<usize> {
    let start = grid_col_to_byte_offset(cells, col_start);
    let end = grid_col_to_byte_offset(cells, col_end.saturating_add(1));
    start..end
}

fn clamp_char_range(text: &str, range: Range<usize>) -> Range<usize> {
    let len = text.len();
    let mut start = range.start.min(len);
    let mut end = range.end.min(len);

    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    while end < len && !text.is_char_boundary(end) {
        end += 1;
    }
    end = end.min(len);
    if !text.is_char_boundary(end) {
        end = start;
    }
    start..end.max(start)
}

fn row_fingerprint(cells: &[TerminalCell]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for cell in cells {
        cell.ch.hash(&mut hasher);
        hash_option_f32_array(cell.fg, &mut hasher);
        hash_option_f32_array(cell.bg, &mut hasher);
        cell.bold.hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_option_f32_array(value: Option<[f32; 4]>, hasher: &mut DefaultHasher) {
    value.is_some().hash(hasher);
    if let Some([a, b, c, d]) = value {
        a.to_bits().hash(hasher);
        b.to_bits().hash(hasher);
        c.to_bits().hash(hasher);
        d.to_bits().hash(hasher);
    }
}

#[derive(Clone, Default)]
struct CachedRow {
    fingerprint: u64,
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
}

/// 行级渲染缓存：cell 未变时复用 `(text, highlights)`。
#[derive(Default)]
pub struct TerminalRowCache {
    rows: Vec<CachedRow>,
}

impl TerminalRowCache {
    pub fn prepare(&mut self, row_count: usize) {
        self.rows.truncate(row_count);
        self.rows.resize_with(row_count, CachedRow::default);
    }

    fn row_content(
        &mut self,
        row_idx: usize,
        cells: &[TerminalCell],
    ) -> (String, Vec<(Range<usize>, HighlightStyle)>) {
        let fp = row_fingerprint(cells);
        if self
            .rows
            .get(row_idx)
            .is_some_and(|e| e.fingerprint == fp && !e.text.is_empty())
        {
            let entry = &self.rows[row_idx];
            return (entry.text.clone(), entry.highlights.clone());
        }
        let (text, highlights) = row_highlights(row_idx, cells, None);
        let highlights = sanitize_highlights(&text, highlights);
        if row_idx >= self.rows.len() {
            self.rows.resize(row_idx + 1, CachedRow::default());
        }
        self.rows[row_idx] = CachedRow {
            fingerprint: fp,
            text: text.clone(),
            highlights: highlights.clone(),
        };
        (text, highlights)
    }
}

fn sanitize_highlights(
    text: &str,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
) -> Vec<(Range<usize>, HighlightStyle)> {
    highlights
        .into_iter()
        .map(|(range, style)| (clamp_char_range(text, range), style))
        .filter(|(range, _)| !range.is_empty())
        .collect()
}

fn merge_runs(cells: &[TerminalCell]) -> Vec<(String, StyleKey)> {
    let mut runs: Vec<(String, StyleKey)> = Vec::new();
    for cell in cells {
        let style = StyleKey {
            fg: cell.fg,
            bg: cell.bg,
            bold: cell.bold,
        };
        if let Some((text, last_style)) = runs.last_mut() {
            if *last_style == style {
                text.push(cell.ch);
                continue;
            }
        }
        runs.push((cell.ch.to_string(), style));
    }
    runs
}

fn row_highlights(
    row_idx: usize,
    cells: &[TerminalCell],
    selection: Option<&TerminalSelection>,
) -> (String, Vec<(Range<usize>, HighlightStyle)>) {
    let runs = merge_runs(cells);
    let text: String = runs.iter().map(|(t, _)| t.as_str()).collect();
    let mut highlights = Vec::new();
    let mut offset = 0usize;

    for (run_text, style) in &runs {
        let len = run_text.len();
        let range = offset..offset + len;
        let mut highlight = HighlightStyle::default();
        highlight.color = Some(hsla_from_cell(style.fg, default_fg()));
        if let Some(bg) = style.bg {
            highlight.background_color = Some(hsla_from_cell(Some(bg), default_bg()));
        }
        if style.bold {
            highlight.font_weight = Some(FontWeight::BOLD);
        }
        highlights.push((range.clone(), highlight));
        offset += len;
    }

    if let Some(sel) = selection {
        if !sel.is_empty() {
            let (start, end) = sel.normalized();
            if row_idx >= start.row && row_idx <= end.row {
                let col_start = if row_idx == start.row { start.col } else { 0 };
                let col_end = if row_idx == end.row {
                    end.col
                } else {
                    row_display_width(cells).saturating_sub(1)
                };
                let byte_range = grid_col_range_to_byte_range(cells, col_start, col_end);
                if !byte_range.is_empty() && byte_range.end <= text.len() {
                    highlights.push((
                        byte_range,
                        HighlightStyle {
                            background_color: Some(selection_bg()),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
    }

    (text, highlights)
}

fn render_grid_row_cached(
    cells: &[TerminalCell],
    row_idx: usize,
    cache: &mut TerminalRowCache,
    font_size: f32,
    line_height: f32,
) -> impl IntoElement {
    let (text, highlights) = cache.row_content(row_idx, cells);

    div()
        .h(px(line_height))
        .w_full()
        .flex_shrink_0()
        .font_family(TERM_FONT)
        .text_size(px(font_size))
        .line_height(px(line_height))
        .text_color(default_fg())
        .child(StyledText::new(text).with_highlights(highlights))
        .into_any_element()
}

/// 选区 overlay：拖动选区时不重建各行 StyledText。
pub struct TerminalSelectionOverlay {
    selection: Option<TerminalSelection>,
    font_size: f32,
    line_height: f32,
    rows: usize,
    cols: usize,
}

impl TerminalSelectionOverlay {
    pub fn new(
        selection: Option<TerminalSelection>,
        font_size: f32,
        line_height: f32,
        rows: usize,
        cols: usize,
    ) -> Self {
        Self {
            selection,
            font_size,
            line_height,
            rows,
            cols,
        }
    }
}

impl IntoElement for TerminalSelectionOverlay {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalSelectionOverlay {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.position = Position::Absolute;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(sel) = self.selection.filter(|s| !s.is_empty()) else {
            return;
        };
        let char_width = (self.font_size * CHAR_WIDTH_RATIO).max(7.0);
        let line_h = self.line_height;
        let (start, end) = sel.normalized();
        let bg = selection_bg();

        for row_idx in start.row..=end.row.min(self.rows.saturating_sub(1)) {
            let col_start = if row_idx == start.row { start.col } else { 0 };
            let col_end = if row_idx == end.row {
                end.col
            } else {
                self.cols.saturating_sub(1)
            };
            let x = bounds.origin.x + px(col_start as f32 * char_width);
            let y = bounds.origin.y + px(row_idx as f32 * line_h);
            let w = px(((col_end + 1).saturating_sub(col_start)) as f32 * char_width);
            let h = px(line_h);
            window.paint_quad(fill(Bounds::new(point(x, y), size(w, h)), bg));
        }
        let _ = cx;
    }
}

pub fn estimate_grid_size(
    bounds: Size<Pixels>,
    font_size: f32,
    padding: f32,
) -> (usize, usize, Pixels, Pixels) {
    let line_height = px((font_size * LINE_HEIGHT_RATIO).max(15.0));
    let char_width = px((font_size * CHAR_WIDTH_RATIO).max(7.0));
    let inner_w = (bounds.width - px(padding * 2.0)).max(px(0.0));
    let inner_h = (bounds.height - px(padding * 2.0)).max(px(0.0));
    let cols = ((inner_w / char_width).floor() as usize).clamp(20, 300);
    let rows = ((inner_h / line_height).floor() as usize).clamp(8, 200);
    (cols, rows, char_width, line_height)
}

fn empty_row(cols: usize) -> Vec<TerminalCell> {
    (0..cols)
        .map(|_| TerminalCell {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
            inverse: false,
        })
        .collect()
}

/// Shell 终端视口：行数与 PTY 一致。
pub fn render_shell_grid(
    term: &TerminalViewModel,
    font_size: f32,
    cache: &mut TerminalRowCache,
    selection: Option<TerminalSelection>,
    cx: &App,
) -> AnyElement {
    let line_height = (font_size * LINE_HEIGHT_RATIO).max(15.0);
    let cols = term
        .cols
        .max(term.grid.first().map(|r| r.len()).unwrap_or(80));
    let viewport_rows = term.rows.max(term.grid.len()).max(1);
    let blank = empty_row(cols);
    cache.prepare(viewport_rows);

    if term.grid.is_empty() {
        return div()
            .flex_1()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .font_family(TERM_FONT)
            .text_size(px(font_size))
            .line_height(px(line_height))
            .text_color(ui::text_secondary(cx))
            .child("已连接，点击此处开始输入…")
            .into_any_element();
    }

    let mut row_elements = Vec::with_capacity(viewport_rows);
    for row_idx in 0..viewport_rows {
        let cells = term
            .grid
            .get(row_idx)
            .map(|r| r.as_slice())
            .unwrap_or(blank.as_slice());
        row_elements.push(
            render_grid_row_cached(cells, row_idx, cache, font_size, line_height)
                .into_any_element(),
        );
    }

    div()
        .flex_1()
        .size_full()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .w_full()
        .child(
            div()
                .relative()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .flex_col()
                        .children(row_elements),
                )
                .child(TerminalSelectionOverlay::new(
                    selection,
                    font_size,
                    line_height,
                    viewport_rows,
                    cols,
                )),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{grid_col_range_to_byte_range, grid_col_to_byte_offset, sanitize_highlights};
    use crate::terminal::TerminalCell;

    fn cell(ch: char) -> TerminalCell {
        TerminalCell {
            ch,
            fg: None,
            bg: None,
            bold: false,
            inverse: false,
        }
    }

    #[test]
    fn grid_col_maps_wide_char_columns() {
        let row = vec![cell('中'), cell('a')];
        assert_eq!(grid_col_to_byte_offset(&row, 0), 0);
        assert_eq!(grid_col_to_byte_offset(&row, 1), '中'.len_utf8());
        assert_eq!(grid_col_to_byte_offset(&row, 2), '中'.len_utf8());
        assert_eq!(
            grid_col_to_byte_offset(&row, 3),
            ('中'.to_string() + "a").len()
        );
    }

    #[test]
    fn grid_col_range_respects_char_boundaries() {
        let row = vec![cell('中'), cell('a')];
        let text = "中a".to_string();
        let range = grid_col_range_to_byte_range(&row, 0, 1);
        assert!(text.is_char_boundary(range.start));
        assert!(text.is_char_boundary(range.end));
    }

    #[test]
    fn sanitize_highlights_clamps_invalid_ranges() {
        let text = "中文ab";
        let highlights = sanitize_highlights(text, vec![(1..2, gpui::HighlightStyle::default())]);
        for (range, _) in highlights {
            assert!(text.is_char_boundary(range.start));
            assert!(text.is_char_boundary(range.end));
        }
    }
}
