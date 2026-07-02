use gpui::*;
use std::ops::Range;

/// 共享的文本输入状态机 — 被 TextInputState 和 ComboBoxState 复用
/// 借鉴 Yororen UI 的设计：value 作为调用者参数，而非 self 字段
#[derive(Clone)]
pub struct TextInputCore {
    pub caret: usize,
    pub selection_start: usize,
    pub selection_end: usize,
    pub scroll_x: Pixels,
    pub last_layout: Option<ShapedLine>,
    pub last_bounds: Option<Bounds<Pixels>>,
    pub last_line_layouts: Vec<ShapedLine>,
    pub last_line_byte_ranges: Vec<Range<usize>>,
    pub last_line_height: Option<Pixels>,
    pub is_selecting: bool,
    pub cursor_visible: bool,
    pub cursor_blink_epoch: usize,
    pub marked_range: Option<Range<usize>>,
    focus_handle: FocusHandle,
}

impl TextInputCore {
    pub fn new(cx: &mut App) -> Self {
        Self {
            caret: 0,
            selection_start: 0,
            selection_end: 0,
            scroll_x: px(0.0),
            last_layout: None,
            last_bounds: None,
            last_line_layouts: Vec::new(),
            last_line_byte_ranges: Vec::new(),
            last_line_height: None,
            is_selecting: false,
            cursor_visible: true,
            cursor_blink_epoch: 0,
            marked_range: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn selected_range(&self) -> Range<usize> {
        self.selection_start.min(self.selection_end)
            ..self.selection_start.max(self.selection_end)
    }

    pub fn has_selection(&self) -> bool {
        self.selection_start != self.selection_end
    }

    pub fn focus_in(&mut self) {
        self.cursor_blink_epoch = self.cursor_blink_epoch.wrapping_add(1);
        self.cursor_visible = true;
    }

    pub fn on_mouse_up(&mut self) {
        self.is_selecting = false;
    }

    pub fn show_character_palette(&self, window: &mut Window) {
        window.show_character_palette();
    }

    // UTF-8 ↔ UTF-16 转换（IME 管线使用 UTF-16）
    pub fn offset_to_utf16(value: &str, byte_offset: usize) -> usize {
        let mut count = 0usize;
        for (i, c) in value.char_indices() {
            if i >= byte_offset { return count; }
            count += c.len_utf16();
        }
        count
    }

    pub fn utf16_to_offset(value: &str, utf16_offset: usize) -> usize {
        let mut count = 0usize;
        for (i, c) in value.char_indices() {
            if count >= utf16_offset { return i; }
            count += c.len_utf16();
        }
        value.len()
    }

    pub fn range_to_utf16(value: &str, byte_range: &Range<usize>) -> Range<usize> {
        Self::offset_to_utf16(value, byte_range.start)
            ..Self::offset_to_utf16(value, byte_range.end)
    }

    pub fn range_from_utf16(value: &str, utf16_range: &Range<usize>) -> Range<usize> {
        Self::utf16_to_offset(value, utf16_range.start)
            ..Self::utf16_to_offset(value, utf16_range.end)
    }

    pub fn text_for_range_utf16(value: &str, range_utf16: Range<usize>) -> (String, Range<usize>) {
        let start = Self::utf16_to_offset(value, range_utf16.start);
        let end = Self::utf16_to_offset(value, range_utf16.end);
        let text = value.get(start..end).unwrap_or("").to_string();
        (text, start..end)
    }

    // UTF-8 安全边界检测
    pub fn prev_boundary(value: &str, byte_offset: usize) -> usize {
        if byte_offset == 0 { return 0; }
        let bytes = value.as_bytes();
        let mut i = byte_offset - 1;
        while i > 0 && (bytes[i] & 0b1100_0000) == 0b1000_0000 { i -= 1; }
        i
    }

    pub fn next_boundary(value: &str, byte_offset: usize) -> usize {
        let len = value.len();
        if byte_offset >= len { return len; }
        let bytes = value.as_bytes();
        let mut i = byte_offset + 1;
        while i < len && (bytes[i] & 0b1100_0000) == 0b1000_0000 { i += 1; }
        i
    }

    // 光标移动
    pub fn move_to(&mut self, value: &str, offset: usize) {
        let clamped = offset.min(value.len());
        self.caret = clamped;
        self.selection_start = clamped;
        self.selection_end = clamped;
    }

    pub fn select_to(&mut self, value: &str, offset: usize) {
        let clamped = offset.min(value.len());
        self.caret = clamped;
        self.selection_end = clamped;
    }

    // 文本替换
    pub fn replace_text(&mut self, value: &mut String, start: usize, end: usize, new_text: &str) {
        let start = start.min(value.len());
        let end = end.max(start).min(value.len());
        value.replace_range(start..end, new_text);
        let new_caret = start + new_text.len();
        self.caret = new_caret;
        self.selection_start = new_caret;
        self.selection_end = new_caret;
    }

    pub fn replace_text_in_range_bytes(
        &mut self,
        value: &mut String,
        max_length: Option<usize>,
        range: Option<Range<usize>>,
        new_text: &str,
    ) -> bool {
        let before = value.clone();
        let resolved = range.or_else(|| self.marked_range.clone()).or_else(|| {
            if !self.selected_range().is_empty() { Some(self.selected_range()) } else { None }
        });
        let effective = if let Some(cap) = max_length {
            let existing_len = match &resolved {
                Some(r) => value.len() - (r.end - r.start),
                None => value.len(),
            };
            let room = cap.saturating_sub(existing_len);
            &new_text[..new_text.len().min(room)]
        } else {
            new_text
        };
        match &resolved {
            Some(r) => self.replace_text(value, r.start, r.end, effective),
            None => self.replace_text(value, self.caret, self.caret, effective),
        }
        self.marked_range = None;
        *value != before
    }

    pub fn replace_and_mark_text_in_range_bytes(
        &mut self,
        value: &mut String,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
    ) -> bool {
        let before = value.clone();
        let range = range.or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range());
        let range_start = range.start.min(value.len());
        let range_end = range.end.max(range_start).min(value.len());
        value.replace_range(range_start..range_end, new_text);
        let marked_start = range_start;
        let marked_end = range_start + new_text.len();
        self.caret = marked_end;
        if !new_text.is_empty() {
            self.marked_range = Some(marked_start..marked_end);
        } else {
            self.marked_range = None;
        }
        if let Some(sel_utf16) = new_selected_range {
            let start_in_marked = Self::utf16_to_offset(value, sel_utf16.start).saturating_sub(marked_start);
            let end_in_marked = Self::utf16_to_offset(value, sel_utf16.end).saturating_sub(marked_start);
            let sel_start = (marked_start + start_in_marked).min(marked_end);
            let sel_end = (marked_start + end_in_marked).min(marked_end);
            self.selection_start = sel_start;
            self.selection_end = sel_end;
        } else {
            self.selection_start = marked_end;
            self.selection_end = marked_end;
        }
        *value != before
    }

    // EntityInputHandler body methods
    pub fn text_for_range_inner(&self, value: &str, range_utf16: Range<usize>) -> (String, Range<usize>) {
        Self::text_for_range_utf16(value, range_utf16)
    }

    pub fn selected_text_range_inner(&self, value: &str) -> UTF16Selection {
        let byte_range = self.selected_range();
        let start = Self::offset_to_utf16(value, byte_range.start);
        let end = Self::offset_to_utf16(value, byte_range.end);
        UTF16Selection { range: start..end, reversed: false }
    }

    pub fn bounds_for_range_inner(
        &self,
        value: &str,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range_bytes = Self::range_from_utf16(value, &range_utf16);
        let start_x = line.x_for_index(range_bytes.start);
        let end_x = line.x_for_index(range_bytes.end);
        Some(Bounds::from_corners(
            gpui::point(element_bounds.left() + start_x - self.scroll_x, element_bounds.top()),
            gpui::point(element_bounds.left() + end_x - self.scroll_x, element_bounds.bottom()),
        ))
    }

    pub fn character_index_for_point_inner(&self, value: &str, point: Point<Pixels>) -> Option<usize> {
        if value.is_empty() { return Some(0); }
        let bounds = self.last_bounds.as_ref()?;
        let local = bounds.localize(&point)?;
        let line = self.last_layout.as_ref()?;
        let utf8_index = line.index_for_x(local.x + self.scroll_x).unwrap_or(line.len());
        Some(Self::offset_to_utf16(value, utf8_index))
    }

    // 光标移动 action
    pub fn left(&mut self, value: &str) {
        if self.has_selection() {
            self.move_to(value, self.selected_range().start);
        } else {
            self.move_to(value, Self::prev_boundary(value, self.caret));
        }
    }

    pub fn right(&mut self, value: &str) {
        if self.has_selection() {
            self.move_to(value, self.selected_range().end);
        } else {
            self.move_to(value, Self::next_boundary(value, self.caret));
        }
    }

    pub fn select_left(&mut self, value: &str) {
        self.select_to(value, Self::prev_boundary(value, self.caret));
    }

    pub fn select_right(&mut self, value: &str) {
        self.select_to(value, Self::next_boundary(value, self.caret));
    }

    pub fn select_all(&mut self, value: &str) {
        self.move_to(value, 0);
        self.select_to(value, value.len());
    }

    pub fn home(&mut self) {
        self.caret = 0;
        self.selection_start = 0;
        self.selection_end = 0;
    }

    pub fn end(&mut self, value: &str) {
        self.move_to(value, value.len());
    }

    // 编辑 action
    pub fn backspace(&mut self, value: &mut String) -> bool {
        let before = value.clone();
        if self.has_selection() {
            let r = self.selected_range();
            self.replace_text(value, r.start, r.end, "");
        } else if self.caret > 0 {
            let prev = Self::prev_boundary(value, self.caret);
            self.replace_text(value, prev, self.caret, "");
        }
        self.marked_range = None;
        *value != before
    }

    pub fn delete(&mut self, value: &mut String) -> bool {
        let before = value.clone();
        if self.has_selection() {
            let r = self.selected_range();
            self.replace_text(value, r.start, r.end, "");
        } else if self.caret < value.len() {
            let next = Self::next_boundary(value, self.caret);
            self.replace_text(value, self.caret, next, "");
        }
        self.marked_range = None;
        *value != before
    }

    pub fn paste(&mut self, value: &mut String, paste_newlines: bool, cx: &mut App) -> bool {
        let Some(item) = cx.read_from_clipboard() else { return false; };
        let Some(text) = item.text() else { return false; };
        let text = if paste_newlines { text.to_string() } else { text.replace('\n', " ") };
        let before = value.clone();
        let changed = self.replace_text_in_range_bytes(value, None, None, &text);
        self.marked_range = None;
        let _ = before;
        changed
    }

    pub fn copy(&self, value: &str, cx: &mut App) {
        if self.has_selection() {
            let r = self.selected_range();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(value[r.clone()].to_string()));
        }
    }

    pub fn cut(&mut self, value: &mut String, cx: &mut App) -> bool {
        if self.has_selection() {
            let r = self.selected_range();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(value[r.clone()].to_string()));
            let before = value.clone();
            self.replace_text(value, r.start, r.end, "");
            return *value != before;
        }
        false
    }

    // 鼠标选择
    pub fn on_mouse_down(&mut self, value: &str, position: Point<Pixels>, window: &mut Window) {
        self.is_selecting = true;
        if let Some(utf16) = self.character_index_for_point_inner(value, position) {
            self.move_to(value, Self::utf16_to_offset(value, utf16));
        }
        window.focus(&self.focus_handle);
    }

    pub fn on_mouse_move(&mut self, value: &str, event: &MouseMoveEvent) {
        if self.is_selecting {
            if let Some(utf16) = self.character_index_for_point_inner(value, event.position) {
                self.select_to(value, Self::utf16_to_offset(value, utf16));
            }
        }
    }
}
