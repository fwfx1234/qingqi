//! InputState — the core state entity for the text input field.

use std::ops::Range;
use std::sync::Arc;
use std::rc::Rc;

use anyhow::Result;
use gpui::{
    Action, App, AppContext, Bounds, ClipboardItem, Context, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, Pixels, Point, Render, ScrollHandle, ScrollWheelEvent, SharedString,
    Styled as _, Subscription, Task, TextRun, UTF16Selection, Window, actions, div,
    point, size,
    prelude::FluentBuilder as _, px,
};
use lsp_types::Position as LspPosition;
use ropey::Rope;
use serde::Deserialize;
use unicode_segmentation::*;

use super::{
    BlinkCursor, Change, InputMode, MaskPattern, RopeExt, Selection,
    TextWrapper,
};
use crate::token::tokens;

// Re-export Position from lsp_types for compatibility
pub use lsp_types::Position;

/// The direction of cursor movement.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveDirection {
    Up,
    Down,
}

// ── Action definitions ────────────────────────────────────────────────────

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = qingqi_ui, no_json)]
pub struct Enter {
    pub secondary: bool,
}

actions!(
    qingqi_ui,
    [
        Backspace,
        Delete,
        DeleteToBeginningOfLine,
        DeleteToEndOfLine,
        DeleteToPreviousWordStart,
        DeleteToNextWordEnd,
        Indent,
        Outdent,
        IndentInline,
        OutdentInline,
        MoveUp,
        MoveDown,
        MoveLeft,
        MoveRight,
        MoveHome,
        MoveEnd,
        MovePageUp,
        MovePageDown,
        SelectAll,
        SelectToStartOfLine,
        SelectToEndOfLine,
        SelectToStart,
        SelectToEnd,
        SelectToPreviousWordStart,
        SelectToNextWordEnd,
        ShowCharacterPalette,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        MoveToStartOfLine,
        MoveToEndOfLine,
        MoveToStart,
        MoveToEnd,
        MoveToPreviousWord,
        MoveToNextWord,
        Escape,
        ToggleCodeActions,
        Search,
        GoToDefinition,
    ]
);

#[derive(Clone)]
pub enum InputEvent {
    Change,
    PressEnter { secondary: bool },
    Focus,
    Blur,
}

impl EventEmitter<InputEvent> for InputState {}

pub const CONTEXT: &str = "QingqiInput";

/// Initialize key bindings for the input field.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-backspace", DeleteToBeginningOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-delete", DeleteToEndOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-delete", DeleteToNextWordEnd, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-delete", DeleteToNextWordEnd, Some(CONTEXT)),
        KeyBinding::new("enter", Enter { secondary: false }, Some(CONTEXT)),
        KeyBinding::new("secondary-enter", Enter { secondary: true }, Some(CONTEXT)),
        KeyBinding::new("escape", Escape, Some(CONTEXT)),
        KeyBinding::new("up", MoveUp, Some(CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(CONTEXT)),
        KeyBinding::new("pageup", MovePageUp, Some(CONTEXT)),
        KeyBinding::new("pagedown", MovePageDown, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        KeyBinding::new("shift-up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("shift-down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("home", MoveHome, Some(CONTEXT)),
        KeyBinding::new("end", MoveEnd, Some(CONTEXT)),
        KeyBinding::new("tab", IndentInline, Some(CONTEXT)),
        KeyBinding::new("shift-tab", OutdentInline, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-z", Redo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-y", Redo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-y", Redo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-f", Search, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-f", Search, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some(CONTEXT)),
    ]);
}

// ── Additional select actions ─────────────────────────────────────────────

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = qingqi_ui)]
pub struct SelectLeft;
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = qingqi_ui)]
pub struct SelectRight;
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = qingqi_ui)]
pub struct SelectUp;
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = qingqi_ui)]
pub struct SelectDown;

// ── History (simplified) ─────────────────────────────────────────────────

#[derive(Debug, Default)]
struct History {
    undo_stack: Vec<Change>,
    redo_stack: Vec<Change>,
}

impl History {
    fn push(&mut self, change: Change) {
        self.undo_stack.push(change);
        self.redo_stack.clear();
    }

    fn undo(&mut self) -> Option<Change> {
        self.undo_stack.pop()
    }

    fn redo(&mut self) -> Option<Change> {
        self.redo_stack.pop()
    }

    fn start_grouping(&mut self) {}
    fn end_grouping(&mut self) {}
}

// ── LSP stub ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct Lsp {
    pub completion_provider: Option<Rc<dyn CompletionProvider>>,
}

pub trait CompletionProvider {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        cx: &mut Context<InputState>,
    ) -> Task<Result<Vec<String>>>;
}

// ── Search stub ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SearchPanel {
    pub open: bool,
}

// ── Inline completion stub ───────────────────────────────────────────────

#[derive(Default)]
pub struct InlineCompletion {
    pub item: Option<String>,
}

// ── Context menu stub ────────────────────────────────────────────────────

#[derive(Clone)]
pub enum ContextMenu {
    Completion,
    CodeAction,
    MouseContext { position: Point<Pixels> },
}

// ── Hover popover stub ───────────────────────────────────────────────────

#[derive(Clone)]
pub struct HoverPopover;

#[derive(Clone)]
pub struct DiagnosticPopover;

#[derive(Default)]
pub struct HoverDefinition {
    pub offset: Option<usize>,
}

// ── InputState ────────────────────────────────────────────────────────────

pub struct InputState {
    pub(crate) text: Rope,
    pub(crate) selected_range: Selection,
    pub(crate) selected_word_range: Option<Selection>,
    pub(crate) placeholder: SharedString,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) scroll_handle: ScrollHandle,
    pub(crate) masked: bool,
    pub(crate) disabled: bool,
    pub(crate) read_only: bool,
    pub(crate) mode: InputMode,
    pub(crate) blink_cursor: Entity<BlinkCursor>,
    pub(crate) text_wrapper: TextWrapper,
    pub(crate) history: History,
    pub(crate) input_bounds: Bounds<Pixels>,
    pub(crate) last_layout: Option<LastLayout>,
    pub(crate) preferred_column: Option<(Pixels, usize)>,
    pub(crate) ime_marked_range: Option<Selection>,
    pub(crate) on_change: Option<Arc<dyn Fn(&str, &mut Window, &mut Context<Self>)>>,
    pub(crate) on_submit: Option<Arc<dyn Fn(&str, &mut Window, &mut Context<Self>)>>,

    // LSP stubs
    pub(crate) lsp: Lsp,
    pub(crate) search_panel: Option<SearchPanel>,
    pub(crate) context_menu: Option<ContextMenu>,
    pub(crate) hover_popover: Option<HoverPopover>,
    pub(crate) diagnostic_popover: Option<DiagnosticPopover>,
    pub(crate) hover_definition: HoverDefinition,
    pub(crate) inline_completion: InlineCompletion,

    // Internal state
    pub(crate) loading: bool,
    pub(crate) mask_pattern: MaskPattern,
    pub(crate) search_matcher: Option<super::search::SearchMatcher>,

    pub(crate) silent_replace_text: bool,
    pub(crate) _pending_update: bool,
    pub(crate) soft_wrap_enabled: bool,

    _subscriptions: Vec<Subscription>,
}


/// Convert byte index to char index for a string
fn byte_to_char_idx_str(s: &str, byte_idx: usize) -> usize {
    let byte_idx = byte_idx.min(s.len());
    let mut char_idx = 0;
    for (i, _) in s.char_indices() {
        if i >= byte_idx { break; }
        char_idx += 1;
    }
    char_idx
}

impl InputState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let blink_cursor = cx.new(|_| BlinkCursor::new());
        let scroll_handle = ScrollHandle::new();

        let _subscriptions = vec![
            cx.observe(&blink_cursor, |_, _, cx| cx.notify()),
            cx.on_focus(&focus_handle, window, Self::on_focus),
            cx.on_blur(&focus_handle, window, Self::on_blur),
        ];

        Self {
            text: Rope::new(),
            selected_range: Selection::default(),
            selected_word_range: None,
            placeholder: SharedString::default(),
            focus_handle,
            scroll_handle,
            masked: false,
            disabled: false,
            read_only: false,
            mode: InputMode::plain_text(),
            blink_cursor,
            text_wrapper: TextWrapper::new(
                gpui::Font {
                    family: ".SystemUIFont".into(),
                    weight: gpui::FontWeight::default(),
                    style: gpui::FontStyle::Normal,
                    features: gpui::FontFeatures::default(),
                    fallbacks: None,
                },
                px(13.0),
                None,
            ),
            history: History::default(),
            input_bounds: Bounds::default(),
            last_layout: None,
            preferred_column: None,
            ime_marked_range: None,
            on_change: None,
            on_submit: None,
            lsp: Lsp::default(),
            search_panel: None,
            context_menu: None,
            hover_popover: None,
            diagnostic_popover: None,
            hover_definition: HoverDefinition::default(),
            inline_completion: InlineCompletion::default(),
            loading: false,
            mask_pattern: MaskPattern::None,
            search_matcher: None,
            silent_replace_text: false,
            _pending_update: false,
            soft_wrap_enabled: false,
            _subscriptions,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
    pub fn multi_line(mut self, multi_line: bool) -> Self {
        self.mode = self.mode.clone().multi_line(multi_line);
        if multi_line {
            if let InputMode::PlainText { rows, .. } = &mut self.mode {
                *rows = 3;
            }
        }
        self
    }
    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap_enabled = soft_wrap;
        self
    }

    pub fn searchable(self, _searchable: bool) -> Self {
        self
    }

    pub fn default_value(mut self, value: SharedString) -> Self {
        self.text = Rope::from(value.to_string());
        self.selected_range = (self.text.len()..self.text.len()).into();
        self
    }

    pub fn value(&self) -> SharedString {
        self.text.to_string().into()
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>, _: &mut Window, _: &mut Context<Self>) {
        self.placeholder = placeholder.into();
    }
    pub fn set_value(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_and_expand_common_prefix(&Selection::default(), &(0..self.text.len()), value, window, cx);
    }

    pub fn reset_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        let s: SharedString = value.into();
        self.text = Rope::from(s.to_string());
        self.selected_range = (self.text.len()..self.text.len()).into();
        cx.notify();
    }

    pub fn cursor(&self) -> usize {
        // During IME composition, cursor is at the end of the marked range
        if let Some(ime_marked_range) = &self.ime_marked_range {
            return ime_marked_range.end;
        }
        self.selected_range.end
    }

    pub fn is_empty(&self) -> bool {
        self.text.len() == 0
    }

    pub fn focus(&self, window: &mut Window, _: &mut Context<Self>) {
        window.focus(&self.focus_handle);
    }

    pub fn clean(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_value("", window, cx);
    }

    pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.masked = masked;
        cx.notify();
    }

    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.loading = loading;
        cx.notify();
    }
    pub fn set_soft_wrap(&mut self, soft_wrap: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.soft_wrap_enabled = soft_wrap;
        let wrap_width = if soft_wrap { Some(px(800.0)) } else { None };
        self.text_wrapper.set_wrap_width(wrap_width, cx);
    }

    pub fn set_mask_pattern(&mut self, pattern: MaskPattern) {
        self.mask_pattern = pattern;
    }

    pub fn set_searchable(&mut self, searchable: bool, cx: &mut Context<Self>) {
        if searchable && self.search_matcher.is_none() {
            self.search_matcher = Some(super::search::SearchMatcher::new());
        } else if !searchable {
            self.search_matcher = None;
        }
        cx.notify();
    }

    fn on_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| {
            cursor.start(cx);
        });
        cx.emit(InputEvent::Focus);
    }

    fn on_blur(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| cursor.stop(cx));
        cx.emit(InputEvent::Blur);
        cx.notify();
    }

    /// Returns whether the cursor should be visible (for painting).
    pub(crate) fn show_cursor(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle.is_focused(window)
            && self.blink_cursor.read(cx).visible()
            && window.is_window_active()
    }

    pub(super) fn pause_blink_cursor(&mut self, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| {
            cursor.pause(cx);
        });
    }

    pub(crate) fn hide_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
        cx.notify();
    }

    pub(crate) fn clear_inline_completion(&mut self, _cx: &mut Context<Self>) {
        self.inline_completion = InlineCompletion::default();
    }

    pub(crate) fn handle_action_for_context_menu(
        &mut self,
        _action: Box<dyn gpui::Action>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }

    pub(crate) fn replace_text_in_range_silent(
        &mut self,
        range_utf16: Option<UTF16Selection>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = match range_utf16 {
            Some(sel) => self.range_from_utf16(&sel.range),
            None => self.selected_range.into(),
        };
        self.replace_and_expand_common_prefix(&range.clone().into(), &range, new_text, window, cx);
    }

    fn replace_and_expand_common_prefix(
        &mut self,
        old_range: &Selection,
        range: &Range<usize>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let old_text: String = self.text.slice(range.clone()).to_string();

        let clamped_start = range.start.min(self.text.len());
        let clamped_end = range.end.min(self.text.len());
        self.text.replace(clamped_start..clamped_end, new_text);

        let new_end = clamped_start + new_text.len();
        self.selected_range = (new_end..new_end).into();

        // Clear IME marked range to avoid stale state when text is modified by actions
        self.ime_marked_range = None;

        self.text_wrapper.update(
            &Rope::from(self.text.to_string()),
            &(clamped_start..clamped_end),
            &Rope::from(new_text),
            cx,
        );

        self.mode.update_auto_grow(&self.text_wrapper);

        self.history.push(Change::new(
            *old_range,
            &old_text,
            self.selected_range,
            new_text,
        ));

        if let Some(on_change) = &self.on_change {
            let text = self.text.to_string();
            on_change(&text, window, cx);
        }

        cx.emit(InputEvent::Change);
        cx.notify();
    }

    pub(crate) fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.clamp(0, self.text.len());
        self.selected_range = (self.selected_range.start..offset).into();
        cx.notify();
    }

    pub(crate) fn select_to_start(&mut self, _: &SelectToStart, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = (0..self.selected_range.end).into();
        cx.notify();
    }

    pub(crate) fn select_to_end(&mut self, _: &SelectToEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = (self.selected_range.start..self.text.len()).into();
        cx.notify();
    }

    pub(crate) fn select_to_start_of_line(&mut self, _: &SelectToStartOfLine, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.start_of_line();
        self.selected_range = (offset..self.selected_range.end).into();
        cx.notify();
    }

    pub(crate) fn select_to_end_of_line(&mut self, _: &SelectToEndOfLine, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.end_of_line();
        self.selected_range = (self.selected_range.start..offset).into();
        cx.notify();
    }

    pub(crate) fn select_to_previous_word(&mut self, _: &SelectToPreviousWordStart, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.previous_start_of_word();
        self.selected_range = (offset..self.selected_range.end).into();
        cx.notify();
    }

    pub(crate) fn select_to_next_word(&mut self, _: &SelectToNextWordEnd, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.next_end_of_word();
        self.selected_range = (self.selected_range.start..offset).into();
        cx.notify();
    }

    pub(crate) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = (0..self.text.len()).into();
        cx.notify();
    }

    pub(crate) fn start_of_line(&self) -> usize {
        let cursor = self.cursor();
        self.text.line_start_offset(self.text.byte_to_line(cursor))
    }

    pub(crate) fn end_of_line(&self) -> usize {
        let cursor = self.cursor();
        let line = self.text.byte_to_line(cursor);
        let line_start = self.text.line_start_offset(line);
        let line_len = self.text.slice_line(line).len();
        line_start + line_len
    }

    pub(crate) fn start_of_line_of_selection(&self, _: &mut Window, _: &mut Context<Self>) -> usize {
        let start = self.selected_range.start;
        self.text.line_start_offset(self.text.byte_to_line(start))
    }

    pub(crate) fn previous_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        if offset == 0 { return 0; }
        let text = self.text.to_string();
        let mut prev = 0;
        for (idx, _) in text.char_indices() {
            if idx >= offset {
                return prev;
            }
            prev = idx;
        }
        prev
    }

    pub(crate) fn next_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        let text = self.text.to_string();
        for (idx, _) in text.char_indices() {
            if idx > offset {
                return idx;
            }
        }
        self.text.len()
    }

    pub(crate) fn previous_start_of_word(&self) -> usize {
        let cursor = self.cursor();
        if cursor == 0 { return 0; }
        let text = self.text.to_string();
        let mut pos = cursor;
        while pos > 0 {
            let prev = self.previous_boundary(pos);
            let ch = text.chars().nth(byte_to_char_idx_str(&text, prev)).unwrap_or(' ');
            if !ch.is_whitespace() { break; }
            pos = prev;
        }
        while pos > 0 {
            let prev = self.previous_boundary(pos);
            let ch = text.chars().nth(byte_to_char_idx_str(&text, prev)).unwrap_or(' ');
            if ch.is_whitespace() || !ch.is_alphanumeric() { break; }
            pos = prev;
        }
        pos
    }

    pub(crate) fn next_end_of_word(&self) -> usize {
        let cursor = self.cursor();
        if cursor >= self.text.len() { return self.text.len(); }
        let text = self.text.to_string();
        let mut pos = cursor;
        while pos < self.text.len() {
            let ch = text.chars().nth(byte_to_char_idx_str(&text, pos)).unwrap_or(' ');
            if ch.is_whitespace() || !ch.is_alphanumeric() { break; }
            pos = self.next_boundary(pos);
        }
        while pos < self.text.len() {
            let ch = text.chars().nth(byte_to_char_idx_str(&text, pos)).unwrap_or(' ');
            if !ch.is_whitespace() { break; }
            pos = self.next_boundary(pos);
        }
        pos
    }

    pub(crate) fn range_to_utf16(&self, range: &Range<usize>) -> UTF16Selection {
        UTF16Selection {
            range: self.text.offset_to_offset_utf16(range.start)..self.text.offset_to_offset_utf16(range.end),
            reversed: false,
        }
    }

    pub(crate) fn range_from_utf16(&self, range: &std::ops::Range<usize>) -> Range<usize> {
        self.text.offset_utf16_to_offset(range.start)..self.text.offset_utf16_to_offset(range.end)
    }

    pub(crate) fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text.offset_to_offset_utf16(offset)
    }

    pub(crate) fn offset_from_utf16(&self, offset_utf16: usize) -> usize {
        self.text.offset_utf16_to_offset(offset_utf16)
    }

    pub(crate) fn text_for_utf16_range(
        &self,
        range_utf16: UTF16Selection,
        _selected_range: &mut Option<UTF16Selection>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<SharedString> {
        let range = self.range_from_utf16(&range_utf16.range);
        Some(self.text.slice(range).to_string().into())
    }

    pub(crate) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text: String = self.text.slice::<std::ops::Range<usize>>(self.selected_range.into()).to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub(crate) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text: String = self.text.slice::<std::ops::Range<usize>>(self.selected_range.into()).to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            let selected_range = self.selected_range;
            self.replace_and_expand_common_prefix(
                &selected_range,
                &selected_range.into(),
                "",
                window,
                cx,
            );
        }
    }

    pub(crate) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(clipboard) = cx.read_from_clipboard() {
            if let Some(text) = clipboard.text() {
                let selected_range = self.selected_range;
                self.replace_and_expand_common_prefix(
                    &selected_range,
                    &selected_range.into(),
                    &text,
                    window,
                    cx,
                );
            }
        }
    }

    pub(crate) fn undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(change) = self.history.undo() {
            let range = change.old_range;
            self.text.replace(range.start..range.end.min(self.text.len()), &change.old_text);
            self.selected_range = (range.start..range.start + change.old_text.len()).into();
            cx.emit(InputEvent::Change);
            cx.notify();
        }
    }

    pub(crate) fn redo(&mut self, _: &Redo, _window: &mut Window, _cx: &mut Context<Self>) {}

    pub(crate) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        // If IME is composing, skip our deletion (IME already handled it by updating composition text)
        if self.ime_marked_range.is_some() {
            self.pause_blink_cursor(cx);
            return;
        }

        let selected_range = self.selected_range;
        if !selected_range.is_empty() {
            self.replace_and_expand_common_prefix(
                &selected_range,
                &selected_range.into(),
                "",
                window,
                cx,
            );
        } else if selected_range.start > 0 {
            let end = selected_range.start;
            let start = self.previous_boundary(end);
            self.replace_and_expand_common_prefix(
                &selected_range,
                &(start..end),
                "",
                window,
                cx,
            );
        }
        self.pause_blink_cursor(cx);
    }

    pub(crate) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        // If IME is composing, skip our deletion
        if self.ime_marked_range.is_some() {
            self.pause_blink_cursor(cx);
            return;
        }

        let selected_range = self.selected_range;
        if !selected_range.is_empty() {
            self.replace_and_expand_common_prefix(
                &selected_range,
                &selected_range.into(),
                "",
                window,
                cx,
            );
        } else if selected_range.end < self.text.len() {
            let start = selected_range.end;
            let end = self.next_boundary(start);
            self.replace_and_expand_common_prefix(
                &selected_range,
                &(start..end),
                "",
                window,
                cx,
            );
        }
        self.pause_blink_cursor(cx);
    }

    pub(crate) fn delete_to_beginning_of_line(&mut self, _: &DeleteToBeginningOfLine, window: &mut Window, cx: &mut Context<Self>) {
        let offset = self.start_of_line();
        let cursor = self.cursor();
        if offset < cursor {
            let selected_range = self.selected_range;
            self.replace_and_expand_common_prefix(
                &selected_range,
                &(offset..cursor),
                "",
                window,
                cx,
            );
        }
    }

    pub(crate) fn delete_to_end_of_line(&mut self, _: &DeleteToEndOfLine, window: &mut Window, cx: &mut Context<Self>) {
        let offset = self.end_of_line();
        let cursor = self.cursor();
        if cursor < offset {
            let selected_range = self.selected_range;
            self.replace_and_expand_common_prefix(
                &selected_range,
                &(cursor..offset),
                "",
                window,
                cx,
            );
        }
    }

    pub(crate) fn delete_to_previous_word_start(&mut self, _: &DeleteToPreviousWordStart, window: &mut Window, cx: &mut Context<Self>) {
        let offset = self.previous_start_of_word();
        let cursor = self.cursor();
        if offset < cursor {
            let selected_range = self.selected_range;
            self.replace_and_expand_common_prefix(
                &selected_range,
                &(offset..cursor),
                "",
                window,
                cx,
            );
        }
    }

    pub(crate) fn delete_to_next_word_end(&mut self, _: &DeleteToNextWordEnd, window: &mut Window, cx: &mut Context<Self>) {
        let offset = self.next_end_of_word();
        let cursor = self.cursor();
        if cursor < offset {
            let selected_range = self.selected_range;
            self.replace_and_expand_common_prefix(
                &selected_range,
                &(cursor..offset),
                "",
                window,
                cx,
            );
        }
    }

    pub(crate) fn on_action_enter(&mut self, action: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_multi_line() {
            let selected_range = self.selected_range;
            self.replace_and_expand_common_prefix(
                &selected_range,
                &selected_range.into(),
                "\n",
                window,
                cx,
            );
        } else {
            if let Some(on_submit) = &self.on_submit {
                let text = self.text.to_string();
                on_submit(&text, window, cx);
            }
            cx.emit(InputEvent::PressEnter { secondary: action.secondary });
        }
    }

    pub(crate) fn on_action_escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        // If there is still an IME marked range (system didn't clear it), clear the composition text
        if let Some(ime_range) = self.ime_marked_range.take() {
            if !ime_range.is_empty() && ime_range.end <= self.text.len() {
                self.text.replace(ime_range.start..ime_range.end, "");
                self.selected_range = (ime_range.start..ime_range.start).into();
                self.text_wrapper.update(
                    &self.text.clone(),
                    &(ime_range.start..ime_range.end),
                    &ropey::Rope::from(""),
                    cx,
                );
                cx.emit(InputEvent::Change);
            }
        }

        if let Some(ref mut panel) = self.search_panel {
            panel.open = false;
        }
        self.hide_context_menu(cx);
        cx.notify();
    }

    pub(crate) fn on_action_search(&mut self, _: &Search, _: &mut Window, cx: &mut Context<Self>) {
        if self.search_panel.is_none() {
            self.search_panel = Some(SearchPanel::default());
        }
        if let Some(ref mut panel) = self.search_panel {
            panel.open = !panel.open;
        }
        cx.notify();
    }

    pub(crate) fn show_character_palette(&mut self, _: &ShowCharacterPalette, _: &mut Window, _: &mut Context<Self>) {}

    pub(crate) fn go_to_definition(&mut self, _: &GoToDefinition, _: &mut Window, _: &mut Context<Self>) {}

    pub(crate) fn toggle_code_actions(&mut self, _: &ToggleCodeActions, _: &mut Window, _: &mut Context<Self>) {}

    pub(crate) fn on_key_down(&mut self, _: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // Pause blink cursor on any key press, the blink will resume after PAUSE_DELAY
        self.pause_blink_cursor(cx);
    }

    pub(crate) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled { return; }
        self.focus(window, cx);
        if let Some(offset) = self.character_index_for_point(event.position, window, cx) {
            if event.modifiers.shift {
                self.select_to(offset, cx);
            } else {
                self.move_to(offset, None, cx);
            }
        }
    }

    pub(crate) fn on_mouse_up(&mut self, _event: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {}

    pub(crate) fn on_mouse_move(&mut self, _event: &MouseMoveEvent, _: &mut Window, _: &mut Context<Self>) {}

    pub(crate) fn on_drag_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.character_index_for_point(event.position, window, cx) {
            self.select_to(offset, cx);
        }
    }

    pub(crate) fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        // self.scroll_handle.handle_scroll_event(event, cx);
    }

    pub(crate) fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        // Calculate the click position relative to the text origin
        let text_padding = px(8.0);
        let line_height = window.line_height();
        let font_size = window.rem_size() * 0.875;
        
        // Text origin (same as in element.rs paint)
        let text_origin_x = self.input_bounds.origin.x + text_padding;
        let text_origin_y = self.input_bounds.origin.y + text_padding;
        
        // Check if click is within the text vertical bounds
        if point.y < text_origin_y || point.y > text_origin_y + line_height {
            return None;
        }
        
        // Get relative X position from text start
        let rel_x = point.x - text_origin_x;
        if rel_x < px(0.0) {
            return Some(0);
        }
        
        // Measure each character width to find which character the click is on
        let text = self.text.to_string();
        if text.is_empty() {
            return Some(0);
        }
        
        let mut current_width = px(0.0);
        for (i, ch) in text.char_indices() {
            let ch_str = &text[i..i + ch.len_utf8()];
            let run = TextRun {
                len: ch_str.len(),
                font: gpui::Font {
                    family: ".SystemUIFont".into(),
                    weight: gpui::FontWeight::default(),
                    style: gpui::FontStyle::Normal,
                    features: gpui::FontFeatures::default(),
                    fallbacks: None,
                },
                color: crate::token::tokens(cx).foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = window.text_system().shape_line(
                ch_str.to_string().into(),
                font_size,
                &[run],
                None,
            );
            let half_width = shaped.width / 2.;
            
            if rel_x < current_width + half_width {
                return Some(i);
            }
            current_width += shaped.width;
        }
        
        // Click is past the end of the text
        Some(text.len())
    }

    pub(crate) fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }
}


impl EntityInputHandler for InputState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        adjusted_range.replace(self.range_to_utf16(&range).range.clone());
        Some(self.text.slice(range).to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range.into()).range,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.ime_marked_range
            .map(|range| self.range_to_utf16(&range.into()).range)
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.ime_marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }


        // Resolve range: use provided UTF16 range, or fall back to IME marked range, or current selection
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.ime_marked_range.map(|range| {
                let range = self.range_to_utf16(&(range.start..range.end)).range;
                self.range_from_utf16(&range)
            }))
            .unwrap_or(self.selected_range.into());

        // Directly replace text
        let clamped_start = range.start.min(self.text.len());
        let clamped_end = range.end.min(self.text.len());
        self.text.replace(clamped_start..clamped_end, new_text);

        // Update text wrapper
        self.text_wrapper.update(
            &self.text.clone(),
            &(clamped_start..clamped_end),
            &ropey::Rope::from(new_text),
            cx,
        );

        let new_offset = clamped_start + new_text.len();
        self.selected_range = (new_offset..new_offset).into();
        
        // Clear IME marked range after text is committed
        self.ime_marked_range.take();
        
        cx.emit(InputEvent::Change);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        // Resolve range: use provided UTF16 range, or fall back to IME marked range, or current selection
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.ime_marked_range.map(|range| {
                let range = self.range_to_utf16(&(range.start..range.end)).range;
                self.range_from_utf16(&range)
            }))
            .unwrap_or(self.selected_range.into());

        if new_text.is_empty() {
            // IME composition was cancelled — clear the composition text from self.text
            let clamped_start = range.start.min(self.text.len());
            let clamped_end = range.end.min(self.text.len());
            if clamped_start < clamped_end {
                self.text.replace(clamped_start..clamped_end, "");
            }
            self.selected_range = (clamped_start..clamped_start).into();
            self.ime_marked_range = None;
            // Update text wrapper to reflect cleared text
            self.text_wrapper.update(
                &self.text.clone(),
                &(clamped_start..clamped_end),
                &ropey::Rope::from(""),
                cx,
            );
        } else {
            // Replace text in range
            let clamped_start = range.start.min(self.text.len());
            let clamped_end = range.end.min(self.text.len());
            self.text.replace(clamped_start..clamped_end, new_text);

            // Update text wrapper
            self.text_wrapper.update(
                &self.text.clone(),
                &(clamped_start..clamped_end),
                &ropey::Rope::from(new_text),
                cx,
            );

            // Mark the range for IME composition display
            self.ime_marked_range = Some((clamped_start..clamped_start + new_text.len()).into());
            
            // Update selection based on IME's proposed selection within the composition
            self.selected_range = new_selected_range_utf16
                .as_ref()
                .map(|range_utf16| self.range_from_utf16(range_utf16))
                .map(|new_range| {
                    (clamped_start + new_range.start..clamped_start + new_range.end).into()
                })
                .unwrap_or_else(|| (clamped_start + new_text.len()..clamped_start + new_text.len()).into());
        }
        
        self.mode.update_auto_grow(&self.text_wrapper);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // Return the bounds at the cursor position for IME candidate window placement
        let cursor_pos = self.cursor();
        let line_height = window.line_height();
        let font_size = window.rem_size() * 0.875;
        let text = self.text.to_string();
        let text_before_cursor = if cursor_pos > 0 {
            text[..cursor_pos.min(text.len())].to_string()
        } else {
            String::new()
        };
        
        // Shape the text before cursor to get its width
        let run = TextRun {
            len: text_before_cursor.len(),
            font: gpui::Font {
                family: ".SystemUIFont".into(),
                weight: gpui::FontWeight::default(),
                style: gpui::FontStyle::Normal,
                features: gpui::FontFeatures::default(),
                fallbacks: None,
            },
            color: crate::token::tokens(&*cx).foreground,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = window.text_system().shape_line(
            text_before_cursor.into(),
            font_size,
            &[run],
            None,
        );
        
        let cursor_x = bounds.left() + px(8.0) + shaped.width;
        let cursor_y = bounds.top() + px(8.0) + line_height;
        
        Some(Bounds::new(
            point(cursor_x, cursor_y),
            size(px(1.0), line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        // Use the same implementation as the pub(crate) method
        let text_padding = px(8.0);
        let line_height = window.line_height();
        let font_size = window.rem_size() * 0.875;
        
        let text_origin_x = self.input_bounds.origin.x + text_padding;
        let text_origin_y = self.input_bounds.origin.y + text_padding;
        
        if point.y < text_origin_y || point.y > text_origin_y + line_height {
            return None;
        }
        
        let rel_x = point.x - text_origin_x;
        if rel_x < px(0.0) {
            return Some(0);
        }
        
        let text = self.text.to_string();
        if text.is_empty() {
            return Some(0);
        }
        
        let mut current_width = px(0.0);
        for (i, ch) in text.char_indices() {
            let ch_str = &text[i..i + ch.len_utf8()];
            let run = TextRun {
                len: ch_str.len(),
                font: gpui::Font {
                    family: ".SystemUIFont".into(),
                    weight: gpui::FontWeight::default(),
                    style: gpui::FontStyle::Normal,
                    features: gpui::FontFeatures::default(),
                    fallbacks: None,
                },
                color: crate::token::tokens(cx).foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = window.text_system().shape_line(
                ch_str.to_string().into(),
                font_size,
                &[run],
                None,
            );
            let half_width = shaped.width / 2.;
            
            if rel_x < current_width + half_width {
                return Some(i);
            }
            current_width += shaped.width;
        }
        
        Some(text.len())
    }
}

impl Focusable for InputState {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InputState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_handle.is_focused(_window) {
            let running = self.blink_cursor.read(cx).is_running();
            if !running {
                self.blink_cursor.update(cx, |cursor, cx| cursor.start(cx));
            }
        }

        div()
            .id("input-state")
            .flex_1()
            .h_full()
            .flex_grow()
            .overflow_x_hidden()
            .child(super::element::TextElement::new(cx.entity().clone()).placeholder(self.placeholder.clone()))
    }
}

// ── LastLayout ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct LastLayout {
    pub line_height: Pixels,
    pub line_number_width: Pixels,
    pub visible_range: Range<usize>,
    pub visible_top: Pixels,
    pub visible_range_offset: Range<usize>,
    pub lines: Vec<super::text_wrapper::LineLayout>,
}

impl Default for LastLayout {
    fn default() -> Self {
        Self {
            line_height: px(18.0),
            line_number_width: px(0.0),
            visible_range: 0..1,
            visible_top: px(0.0),
            visible_range_offset: 0..0,
            lines: Vec::new(),
        }
    }
}

impl LastLayout {
    pub fn line(&self, row: usize) -> Option<&super::text_wrapper::LineLayout> {
        self.lines.get(row)
    }
}
