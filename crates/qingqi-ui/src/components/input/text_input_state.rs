use gpui::*;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use super::TextInputCore;

pub type TextChangeCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync>;

pub struct TextInputState {
    pub value: String,
    pub placeholder: SharedString,
    pub max_length: Option<usize>,
    pub on_change: Option<TextChangeCallback>,
    pub on_submit: Option<TextChangeCallback>,
    pub paste_newlines: bool,
    pub disabled: bool,
    pub masked: bool,
    pub core: TextInputCore,
}

impl Deref for TextInputState {
    type Target = TextInputCore;
    fn deref(&self) -> &TextInputCore { &self.core }
}
impl DerefMut for TextInputState {
    fn deref_mut(&mut self) -> &mut TextInputCore { &mut self.core }
}

impl TextInputState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            value: String::new(),
            placeholder: SharedString::new_static(""),
            max_length: None,
            on_change: None,
            on_submit: None,
            paste_newlines: false,
            disabled: false,
            masked: false,
            core: TextInputCore::new(cx),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle { self.core.focus_handle() }
    pub fn selected_range(&self) -> std::ops::Range<usize> { self.core.selected_range() }
    pub fn has_selection(&self) -> bool { self.core.has_selection() }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        let len = self.value.len();
        self.core.move_to(&self.value, len);
        self.core.scroll_x = px(0.0);
    }

    pub fn content(&self) -> String { self.value.clone() }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() { return; }
        self.core.replace_text_in_range_bytes(&mut self.value, None, None, text);
    }

    pub fn replace_text_in_range_bytes(&mut self, range: Option<std::ops::Range<usize>>, new_text: &str) -> bool {
        self.core.replace_text_in_range_bytes(&mut self.value, self.max_length, range, new_text)
    }

    pub fn replace_and_mark_text_in_range_bytes(
        &mut self, range: Option<std::ops::Range<usize>>, new_text: &str,
        new_selected_range: Option<std::ops::Range<usize>>,
    ) {
        self.core.replace_and_mark_text_in_range_bytes(&mut self.value, range, new_text, new_selected_range);
    }
}

impl Focusable for TextInputState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle { self.core.focus_handle() }
}

impl EntityInputHandler for TextInputState {
    fn text_for_range(&mut self, range_utf16: std::ops::Range<usize>, adjusted_range: &mut Option<std::ops::Range<usize>>, _window: &mut Window, _cx: &mut Context<Self>) -> Option<String> {
        let (text, adjusted) = self.core.text_for_range_inner(&self.value, range_utf16);
        *adjusted_range = Some(adjusted);
        Some(text)
    }
    fn selected_text_range(&mut self, _ignore_disabled_input: bool, _window: &mut Window, _cx: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(self.core.selected_text_range_inner(&self.value))
    }
    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<std::ops::Range<usize>> {
        self.core.marked_range.as_ref().map(|r| TextInputCore::range_to_utf16(&self.value, r))
    }
    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) { self.core.marked_range = None; }
    fn replace_text_in_range(&mut self, range_utf16: Option<std::ops::Range<usize>>, new_text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.value.clone();
        let range = range_utf16.map(|r| TextInputCore::range_from_utf16(&self.value, &r))
            .or_else(|| self.core.marked_range.clone())
            .or_else(|| if !self.selected_range().is_empty() { Some(self.selected_range()) } else { None });
        self.core.replace_text_in_range_bytes(&mut self.value, self.max_length, range, new_text);
        self.core.marked_range = None;
        if self.value != before { if let Some(cb) = self.on_change.as_ref() { cb(&self.value, window, cx); } }
        cx.notify();
    }
    fn replace_and_mark_text_in_range(&mut self, range_utf16: Option<std::ops::Range<usize>>, new_text: &str, new_selected_range_utf16: Option<std::ops::Range<usize>>, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.value.clone();
        let range = range_utf16.map(|r| TextInputCore::range_from_utf16(&self.value, &r));
        let new_sel = new_selected_range_utf16.map(|r| TextInputCore::range_from_utf16(&self.value, &r));
        self.core.replace_and_mark_text_in_range_bytes(&mut self.value, range, new_text, new_sel);
        if self.value != before { if let Some(cb) = self.on_change.as_ref() { cb(&self.value, window, cx); } }
        cx.notify();
    }
    fn bounds_for_range(&mut self, range_utf16: std::ops::Range<usize>, element_bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        self.core.bounds_for_range_inner(&self.value, range_utf16, element_bounds)
    }
    fn character_index_for_point(&mut self, point: Point<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) -> Option<usize> {
        self.core.character_index_for_point_inner(&self.value, point)
    }
}
