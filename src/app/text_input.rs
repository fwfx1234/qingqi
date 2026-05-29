use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, InspectorElementId,
    InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Style, Styled, TextRun, UTF16Selection,
    UnderlineStyle, Window, div, fill, hsla, point, px, relative, rgb, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::app::ui;

gpui::actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Newline,
    ]
);

#[derive(Clone, Copy)]
pub struct TextInputStyle {
    pub height: f32,
    pub font_size: f32,
    pub padding: f32,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            height: 38.0,
            font_size: 13.0,
            padding: 8.0,
        }
    }
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<TextLayoutSnapshot>,
    is_selecting: bool,
    style: TextInputStyle,
    multiline: bool,
    draw_chrome: bool,
    read_only: bool,
    monospace: bool,
    preferred_column: Option<usize>,
    text_color: Option<Hsla>,
    placeholder_color: Option<Hsla>,
}

#[derive(Clone)]
struct TextLayoutLine {
    range: Range<usize>,
    shaped: gpui::ShapedLine,
}

#[derive(Clone)]
struct TextLayoutSnapshot {
    lines: Vec<TextLayoutLine>,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    is_placeholder: bool,
}

#[derive(Clone, Copy)]
enum VerticalDirection {
    Up,
    Down,
}

impl TextInput {
    pub fn new(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        let value = value.into();
        let len = value.len();
        Self {
            focus_handle: cx.focus_handle(),
            content: value,
            placeholder: placeholder.into(),
            selected_range: len..len,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            is_selecting: false,
            style: TextInputStyle::default(),
            multiline: false,
            draw_chrome: true,
            read_only: false,
            monospace: false,
            preferred_column: None,
            text_color: None,
            placeholder_color: None,
        }
    }

    pub fn register_bindings(cx: &mut App) {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some("TextInput")),
            KeyBinding::new("delete", Delete, Some("TextInput")),
            KeyBinding::new("left", Left, Some("TextInput")),
            KeyBinding::new("right", Right, Some("TextInput")),
            KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
            KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
            KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
            KeyBinding::new("cmd-v", Paste, Some("TextInput")),
            KeyBinding::new("cmd-c", Copy, Some("TextInput")),
            KeyBinding::new("cmd-x", Cut, Some("TextInput")),
            KeyBinding::new("enter", Newline, Some("TextInput")),
            KeyBinding::new("home", Home, Some("TextInput")),
            KeyBinding::new("end", End, Some("TextInput")),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("TextInput")),
        ]);
    }

    pub fn set_style(&mut self, style: TextInputStyle, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    pub fn set_multiline(&mut self, multiline: bool, cx: &mut Context<Self>) {
        self.multiline = multiline;
        cx.notify();
    }

    pub fn set_chrome(&mut self, draw_chrome: bool, cx: &mut Context<Self>) {
        self.draw_chrome = draw_chrome;
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        if read_only {
            self.is_selecting = false;
            self.marked_range = None;
        }
        cx.notify();
    }

    pub fn set_monospace(&mut self, monospace: bool, cx: &mut Context<Self>) {
        self.monospace = monospace;
        cx.notify();
    }

    pub fn set_text_colors(
        &mut self,
        text_color: impl Into<Hsla>,
        placeholder_color: impl Into<Hsla>,
        cx: &mut Context<Self>,
    ) {
        let text_color = text_color.into();
        let placeholder_color = placeholder_color.into();
        if self.text_color == Some(text_color) && self.placeholder_color == Some(placeholder_color)
        {
            return;
        }
        self.text_color = Some(text_color);
        self.placeholder_color = Some(placeholder_color);
        cx.notify();
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        let len = text.len();
        self.content = text.into();
        self.selected_range = len..len;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text(String::new(), cx);
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    pub fn text(&self) -> String {
        self.content.to_string()
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.move_to(
                line_start_for_text(self.content.as_ref(), self.cursor_offset()),
                cx,
            );
        } else {
            self.move_to(0, cx);
        }
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.move_to(
                line_end_for_text(self.content.as_ref(), self.cursor_offset()),
                cx,
            );
        } else {
            self.move_to(self.content.len(), cx);
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = normalize_pasted_text(&text, self.multiline);
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline && !self.read_only {
            self.replace_text_in_range(None, "\n", window, cx);
        } else {
            cx.propagate();
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }

        match event.keystroke.key.as_str() {
            "up" => {
                self.move_vertical(VerticalDirection::Up, event.keystroke.modifiers.shift, cx);
                cx.stop_propagation();
            }
            "down" => {
                self.move_vertical(VerticalDirection::Down, event.keystroke.modifiers.shift, cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.preferred_column = None;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.preferred_column = None;
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }

        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn move_vertical(
        &mut self,
        direction: VerticalDirection,
        selecting: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.multiline {
            return;
        }

        let cursor = self.cursor_offset();
        let (offset, goal_column) = vertical_offset_for_text(
            self.content.as_ref(),
            cursor,
            self.preferred_column,
            direction,
        );
        self.preferred_column = Some(goal_column);

        if selecting {
            if self.selection_reversed {
                self.selected_range.start = offset;
            } else {
                self.selected_range.end = offset;
            }

            if self.selected_range.end < self.selected_range.start {
                self.selection_reversed = !self.selection_reversed;
                self.selected_range = self.selected_range.end..self.selected_range.start;
            }
        } else {
            self.selected_range = offset..offset;
            self.selection_reversed = false;
        }

        cx.notify();
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let Some(layout) = self.last_layout.as_ref() else {
            return 0;
        };

        let bounds = layout.bounds;
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }

        let line_index = line_index_for_y(position.y - bounds.top(), layout.line_height)
            .min(layout.lines.len().saturating_sub(1));
        let Some(line) = layout.lines.get(line_index) else {
            return 0;
        };
        let local_x = position.x - bounds.left();
        line.range.start + line.shaped.closest_index_for_x(local_x)
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn utf8_len_for_text(text: &str) -> usize {
        text.len()
    }

    fn offset_from_utf16_in_text(text: &str, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in text.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let range = self.clamp_range_to_content(range.clone());
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        let start = self.offset_from_utf16(range_utf16.start);
        let end = self.offset_from_utf16(range_utf16.end);
        self.clamp_range_to_content(start..end)
    }

    fn clamp_range_to_content(&self, range: Range<usize>) -> Range<usize> {
        let start = self.clamp_to_char_boundary(range.start);
        let end = self.clamp_to_char_boundary(range.end).max(start);
        start..end
    }

    fn clamp_to_char_boundary(&self, offset: usize) -> usize {
        clamp_to_char_boundary(self.content.as_ref(), offset)
    }

    fn clamp_range_to_text(text: &str, range: Range<usize>) -> Range<usize> {
        let start = clamp_to_char_boundary(text, range.start);
        let end = clamp_to_char_boundary(text, range.end).max(start);
        start..end
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

fn normalize_pasted_text(text: &str, multiline: bool) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if multiline {
        normalized
    } else {
        normalized.replace('\n', " ")
    }
}

fn line_start_for_text(text: &str, offset: usize) -> usize {
    let clamped = offset.min(text.len());
    text[..clamped].rfind('\n').map(|idx| idx + 1).unwrap_or(0)
}

fn line_end_for_text(text: &str, offset: usize) -> usize {
    let clamped = offset.min(text.len());
    text[clamped..]
        .find('\n')
        .map(|idx| clamped + idx)
        .unwrap_or(text.len())
}

fn column_for_text(text: &str, offset: usize) -> usize {
    let start = line_start_for_text(text, offset);
    text[start..offset.min(text.len())].graphemes(true).count()
}

fn offset_for_column(text: &str, line_start: usize, line_end: usize, column: usize) -> usize {
    if column == 0 {
        return line_start;
    }

    let mut current = line_start;
    for (seen, grapheme) in text[line_start..line_end].graphemes(true).enumerate() {
        if seen >= column {
            break;
        }
        current += grapheme.len();
    }
    current
}

fn vertical_offset_for_text(
    text: &str,
    offset: usize,
    preferred_column: Option<usize>,
    direction: VerticalDirection,
) -> (usize, usize) {
    let clamped = offset.min(text.len());
    let line_start = line_start_for_text(text, clamped);
    let line_end = line_end_for_text(text, clamped);
    let goal_column = preferred_column.unwrap_or_else(|| column_for_text(text, clamped));

    match direction {
        VerticalDirection::Up => {
            if line_start == 0 {
                return (
                    offset_for_column(text, 0, line_end, goal_column),
                    goal_column,
                );
            }
            let previous_end = line_start.saturating_sub(1);
            let previous_start = line_start_for_text(text, previous_end);
            (
                offset_for_column(text, previous_start, previous_end, goal_column),
                goal_column,
            )
        }
        VerticalDirection::Down => {
            if line_end >= text.len() {
                return (
                    offset_for_column(text, line_start, line_end, goal_column),
                    goal_column,
                );
            }
            let next_start = line_end + 1;
            let next_end = line_end_for_text(text, next_start);
            (
                offset_for_column(text, next_start, next_end, goal_column),
                goal_column,
            )
        }
    }
}

fn line_ranges_for_display(text: &str, multiline: bool) -> Vec<Range<usize>> {
    if !multiline {
        return vec![0..text.len()];
    }

    let mut ranges = Vec::new();
    let mut line_start = 0;
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            ranges.push(line_start..index);
            line_start = index + ch.len_utf8();
        }
    }
    ranges.push(line_start..text.len());
    ranges
}

fn line_index_for_y(y: Pixels, line_height: Pixels) -> usize {
    if y <= Pixels::ZERO || line_height <= Pixels::ZERO {
        0
    } else {
        (y / line_height).floor().max(0.0) as usize
    }
}

fn line_origin(bounds: Bounds<Pixels>, line_height: Pixels, line_index: usize) -> Point<Pixels> {
    point(
        bounds.left(),
        bounds.top() + line_height * line_index as f32,
    )
}

fn centered_line_bounds(bounds: Bounds<Pixels>, line_height: Pixels) -> Bounds<Pixels> {
    if bounds.size.height <= line_height {
        return bounds;
    }

    let inset = (bounds.size.height - line_height) / 2.0;
    Bounds::new(
        point(bounds.origin.x, bounds.origin.y + inset),
        size(bounds.size.width, line_height),
    )
}

fn line_index_for_offset(ranges: &[Range<usize>], offset: usize) -> usize {
    ranges
        .iter()
        .position(|range| offset <= range.end)
        .unwrap_or_else(|| ranges.len().saturating_sub(1))
}

fn runs_for_range(runs: &[TextRun], target_range: Range<usize>) -> Vec<TextRun> {
    let mut mapped = Vec::new();
    let mut run_start = 0;

    for run in runs {
        let run_end = run_start + run.len;
        let overlap_start = target_range.start.max(run_start);
        let overlap_end = target_range.end.min(run_end);
        if overlap_start < overlap_end {
            let mut next = run.clone();
            next.len = overlap_end - overlap_start;
            mapped.push(next);
        }
        run_start = run_end;
        if run_start >= target_range.end {
            break;
        }
    }

    mapped
}

fn resolved_text_font(mut base: gpui::Font, monospace: bool) -> gpui::Font {
    if monospace {
        base.family = "SF Mono".into();
    }
    base
}

fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let selected_range = self.clamp_range_to_content(self.selected_range.clone());
        Some(UTF16Selection {
            range: self.range_to_utf16(&selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(&self.clamp_range_to_content(range.clone())))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.clamp_range_to_content(range);

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        let cursor = self.clamp_to_char_boundary(range.start + Self::utf8_len_for_text(new_text));
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.preferred_column = None;
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.clamp_range_to_content(range);

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        let marked_end =
            self.clamp_to_char_boundary(range.start + Self::utf8_len_for_text(new_text));
        self.marked_range = (!new_text.is_empty()).then_some(range.start..marked_end);
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| {
                let start = Self::offset_from_utf16_in_text(new_text, range_utf16.start);
                let end = Self::offset_from_utf16_in_text(new_text, range_utf16.end);
                Self::clamp_range_to_text(new_text, start..end)
            })
            .map(|new_range| range.start + new_range.start..range.start + new_range.end)
            .unwrap_or(marked_end..marked_end);
        self.selected_range = self.clamp_range_to_content(self.selected_range.clone());
        self.selection_reversed = false;
        self.preferred_column = None;
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.clamp_range_to_content(self.range_from_utf16(&range_utf16));
        let mut matching_lines = last_layout.lines.iter().filter_map(|line| {
            let overlap_start = range.start.max(line.range.start);
            let overlap_end = range.end.min(line.range.end);
            (overlap_start <= overlap_end).then_some((line, overlap_start, overlap_end))
        });

        let (first_line, first_start, _) = matching_lines.next()?;
        let mut last_line = first_line;
        let mut last_end = range.end.min(first_line.range.end);
        for (line, _, overlap_end) in matching_lines {
            last_line = line;
            last_end = overlap_end;
        }

        let first_line_index = last_layout
            .lines
            .iter()
            .position(|line| line.range == first_line.range)?;
        let last_line_index = last_layout
            .lines
            .iter()
            .position(|line| line.range == last_line.range)?;
        let top = bounds.top() + last_layout.line_height * first_line_index as f32;
        let bottom = bounds.top() + last_layout.line_height * (last_line_index + 1) as f32;
        let left = bounds.left()
            + first_line
                .shaped
                .x_for_index(first_start.saturating_sub(first_line.range.start));
        let right = bounds.left()
            + last_line
                .shaped
                .x_for_index(last_end.saturating_sub(last_line.range.start));

        Some(Bounds::from_corners(point(left, top), point(right, bottom)))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.last_layout.as_ref()?;
        let local_point = layout.bounds.localize(&point)?;
        let line_index =
            line_index_for_y(local_point.y, layout.line_height).min(layout.lines.len() - 1);
        let line = layout.lines.get(line_index)?;
        let utf8_index = line.shaped.index_for_x(local_point.x)?;
        Some(self.offset_to_utf16(line.range.start + utf8_index))
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style;
        let line_height = px(style.font_size + 6.0);
        let mut root = div()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::newline))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .h(px(style.height))
            .w_full()
            .rounded(px(8.0));
        if self.draw_chrome {
            root = root
                .bg(ui::bg_surface())
                .border_1()
                .border_color(ui::border_light());
        }

        let content = div()
            .h(px(style.height))
            .w_full()
            .text_size(px(style.font_size))
            .line_height(line_height)
            .child(TextElement { input: cx.entity() });

        let content = if self.multiline {
            content
                .id("text-input-multiline-shell")
                .p(px(style.padding))
                .flex()
                .items_start()
                .overflow_y_scroll()
                .into_any_element()
        } else {
            content
                .px(px(style.padding))
                .flex()
                .items_center()
                .into_any_element()
        };

        root.child(content)
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    layout: Option<TextLayoutSnapshot>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

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
        let input = self.input.read(cx);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let line_height = px(input.style.font_size + 6.0);
        let line_count = if input.multiline {
            line_ranges_for_display(input.content.as_ref(), true)
                .len()
                .max(1)
        } else {
            1
        };
        style.size.height = (line_height * line_count as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();
        let line_height = px(input.style.font_size + 6.0);
        let font = resolved_text_font(style.font(), input.monospace);

        let is_placeholder = content.is_empty();
        let (display_text, text_color) = if is_placeholder {
            (
                input.placeholder.clone(),
                input
                    .placeholder_color
                    .unwrap_or_else(|| hsla(0., 0., 0., 0.32)),
            )
        } else {
            (
                content,
                input
                    .text_color
                    .unwrap_or_else(|| ui::text_primary().into()),
            )
        };

        let run = TextRun {
            len: display_text.len(),
            font: font.clone(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let marked_range = input
            .marked_range
            .as_ref()
            .map(|range| TextInput::clamp_range_to_text(display_text.as_ref(), range.clone()))
            .filter(|range| !range.is_empty());

        let runs = if let Some(marked_range) = marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = px(input.style.font_size);
        let display_len = display_text.len();
        let line_ranges = line_ranges_for_display(display_text.as_ref(), input.multiline);
        let mut lines = Vec::with_capacity(line_ranges.len());
        for range in &line_ranges {
            let line_text: SharedString = display_text[range.clone()].to_string().into();
            let line_runs = runs_for_range(&runs, range.clone());
            let shaped = window
                .text_system()
                .shape_line(line_text, font_size, &line_runs, None);
            lines.push(TextLayoutLine {
                range: range.clone(),
                shaped,
            });
        }

        let cursor = cursor.min(display_len);
        let selected_range =
            selected_range.start.min(display_len)..selected_range.end.min(display_len);
        let text_bounds = if input.multiline {
            bounds
        } else {
            centered_line_bounds(bounds, line_height)
        };
        let layout = TextLayoutSnapshot {
            lines,
            bounds: text_bounds,
            line_height,
            is_placeholder,
        };

        let mut selections = Vec::new();
        if !selected_range.is_empty() && !layout.is_placeholder {
            for (line_index, line) in layout.lines.iter().enumerate() {
                let overlap_start = selected_range.start.max(line.range.start);
                let overlap_end = selected_range.end.min(line.range.end);
                if overlap_start >= overlap_end {
                    continue;
                }
                let origin = line_origin(layout.bounds, line_height, line_index);
                let start_x = line
                    .shaped
                    .x_for_index(overlap_start.saturating_sub(line.range.start));
                let end_x = line
                    .shaped
                    .x_for_index(overlap_end.saturating_sub(line.range.start));
                selections.push(fill(
                    Bounds::from_corners(
                        point(origin.x + start_x, origin.y),
                        point(origin.x + end_x, origin.y + line_height),
                    ),
                    rgba(0x3311ff30),
                ));
            }
        }

        let cursor = if selected_range.is_empty() {
            let cursor_line_index = line_index_for_offset(&line_ranges, cursor);
            let cursor_line = layout.lines.get(cursor_line_index);
            cursor_line.map(|line| {
                let origin = line_origin(layout.bounds, line_height, cursor_line_index);
                let local_offset = cursor.saturating_sub(line.range.start);
                let cursor_pos = line.shaped.x_for_index(local_offset);
                fill(
                    Bounds::new(
                        point(origin.x + cursor_pos, origin.y),
                        size(px(2.), line_height),
                    ),
                    rgb(0x2563eb),
                )
            })
        } else {
            None
        };

        PrepaintState {
            layout: Some(layout),
            cursor,
            selections,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }

        let Some(layout) = prepaint.layout.take() else {
            return;
        };
        for (line_index, line) in layout.lines.iter().enumerate() {
            let origin = line_origin(bounds, layout.line_height, line_index);
            if let Err(error) = line.shaped.paint(origin, layout.line_height, window, cx) {
                tracing::warn!(error = ?error, "text input paint failed");
            }
        }

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(layout);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        VerticalDirection, column_for_text, line_end_for_text, line_index_for_offset,
        line_ranges_for_display, line_start_for_text, normalize_pasted_text, resolved_text_font,
        runs_for_range, vertical_offset_for_text,
    };
    use gpui::{FontFeatures, FontStyle, TextRun, font, hsla};

    #[test]
    fn single_line_paste_replaces_newlines_with_spaces() {
        assert_eq!(
            normalize_pasted_text("alpha\r\nbeta\rgamma\ndelta", false),
            "alpha beta gamma delta"
        );
    }

    #[test]
    fn multiline_paste_normalizes_crlf_to_lf() {
        assert_eq!(
            normalize_pasted_text("alpha\r\nbeta\rgamma", true),
            "alpha\nbeta\ngamma"
        );
    }

    #[test]
    fn line_helpers_track_start_end_and_column() {
        let text = "abc\ndefg\nhi";
        assert_eq!(line_start_for_text(text, 5), 4);
        assert_eq!(line_end_for_text(text, 5), 8);
        assert_eq!(column_for_text(text, 5), 1);
    }

    #[test]
    fn vertical_navigation_preserves_column_between_lines() {
        let text = "abcd\nef\nghijk";
        let start = 2;
        let (down, column) = vertical_offset_for_text(text, start, None, VerticalDirection::Down);
        assert_eq!(down, 7);
        assert_eq!(column, 2);

        let (down_again, column_again) =
            vertical_offset_for_text(text, down, Some(column), VerticalDirection::Down);
        assert_eq!(down_again, 10);
        assert_eq!(column_again, 2);
    }

    #[test]
    fn vertical_navigation_clamps_to_shorter_lines_and_restores_column() {
        let text = "abcd\nef\nghijk";
        let start = 3;
        let (down, column) = vertical_offset_for_text(text, start, None, VerticalDirection::Down);
        assert_eq!(down, 7);
        assert_eq!(column, 3);

        let (down_again, _) =
            vertical_offset_for_text(text, down, Some(column), VerticalDirection::Down);
        assert_eq!(down_again, 11);

        let (up_again, _) =
            vertical_offset_for_text(text, down_again, Some(column), VerticalDirection::Up);
        assert_eq!(up_again, 7);
    }

    #[test]
    fn vertical_navigation_stays_on_boundary_lines() {
        let text = "ab\ncd";
        let (up, up_column) = vertical_offset_for_text(text, 1, None, VerticalDirection::Up);
        assert_eq!(up, 1);
        assert_eq!(up_column, 1);

        let (down, down_column) =
            vertical_offset_for_text(text, text.len(), None, VerticalDirection::Down);
        assert_eq!(down, text.len());
        assert_eq!(down_column, 2);
    }

    #[test]
    fn multiline_ranges_preserve_empty_lines() {
        assert_eq!(
            line_ranges_for_display("alpha\n\nbeta", true),
            vec![0..5, 6..6, 7..11]
        );
        assert_eq!(line_ranges_for_display("alpha", true), vec![0..5]);
        assert_eq!(line_ranges_for_display("alpha\n", true), vec![0..5, 6..6]);
    }

    #[test]
    fn single_line_range_keeps_whole_text() {
        assert_eq!(line_ranges_for_display("alpha\nbeta", false), vec![0..10]);
    }

    #[test]
    fn line_index_tracks_offsets_across_lines() {
        let ranges = vec![0..5, 6..6, 7..11];
        assert_eq!(line_index_for_offset(&ranges, 0), 0);
        assert_eq!(line_index_for_offset(&ranges, 5), 0);
        assert_eq!(line_index_for_offset(&ranges, 6), 1);
        assert_eq!(line_index_for_offset(&ranges, 7), 2);
        assert_eq!(line_index_for_offset(&ranges, 11), 2);
    }

    #[test]
    fn run_slices_follow_target_line_range() {
        let runs = vec![
            TextRun {
                len: 5,
                font: gpui::Font {
                    family: "Helvetica".into(),
                    features: FontFeatures::default(),
                    fallbacks: None,
                    weight: gpui::FontWeight::default(),
                    style: FontStyle::default(),
                },
                color: hsla(0.0, 0.0, 0.0, 1.0),
                background_color: None,
                underline: None,
                strikethrough: None,
            },
            TextRun {
                len: 4,
                font: gpui::Font {
                    family: "Helvetica".into(),
                    features: FontFeatures::default(),
                    fallbacks: None,
                    weight: gpui::FontWeight::default(),
                    style: FontStyle::default(),
                },
                color: hsla(0.0, 0.0, 0.0, 1.0),
                background_color: None,
                underline: None,
                strikethrough: None,
            },
        ];

        let sliced = runs_for_range(&runs, 3..8);
        assert_eq!(sliced.len(), 2);
        assert_eq!(sliced[0].len, 2);
        assert_eq!(sliced[1].len, 3);
    }

    #[test]
    fn resolved_font_keeps_ui_font_for_regular_inputs() {
        let resolved = resolved_text_font(font("Helvetica"), false);
        assert_eq!(resolved.family.as_ref(), "Helvetica");
    }

    #[test]
    fn resolved_font_switches_to_mono_for_editor_like_inputs() {
        let resolved = resolved_text_font(font("Helvetica"), true);
        assert_eq!(resolved.family.as_ref(), "SF Mono");
    }
}
