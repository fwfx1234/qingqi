//! InputState — the core state entity for the text input field.

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Action, App, AppContext, Bounds, ClipboardItem, Context, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding,
    KeyDownEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Render, ScrollHandle, ScrollWheelEvent, SharedString, Styled as _, Subscription, TextRun,
    UTF16Selection, Window, actions, div, point, px, size,
};
use ropey::Rope;
use serde::Deserialize;
use sum_tree::Bias;

use super::{
    BlinkCursor, Change, HighlightSpan, InputDiagnostic, InputMode, MaskPattern, RopeExt,
    Selection, TEXT_PADDING, TextWrapper, input_font_size, input_line_height, input_text_top,
    lsp::{HoverDefinition, InlineCompletion, Lsp},
    popovers::{ContextMenu, DiagnosticPopover, HoverPopover},
    search::SearchPanel,
};

// Re-export Position from lsp_types for compatibility
pub use lsp_types::Position;

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

struct InputBindingsInitialized;

impl gpui::Global for InputBindingsInitialized {}

/// Initialize key bindings for the input field.
pub fn init(cx: &mut App) {
    if cx.try_global::<InputBindingsInitialized>().is_some() {
        return;
    }
    cx.set_global(InputBindingsInitialized);

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
    super::search::init(cx);
    super::number_input::init(cx);
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
    last_groupable: bool,
}

impl History {
    fn push(&mut self, change: Change, groupable: bool) {
        if groupable && self.last_groupable {
            if let Some(previous) = self.undo_stack.last_mut()
                && previous.old_text.is_empty()
                && change.old_text.is_empty()
                && previous.old_range.start + previous.new_text.len() == change.old_range.start
            {
                previous.new_text.push_str(&change.new_text);
                previous.new_range = change.new_range;
                self.redo_stack.clear();
                return;
            }
            if let Some(previous) = self.undo_stack.last_mut()
                && previous.old_range.start == 0
                && change.old_range.start == 0
                && previous.new_text == change.old_text
            {
                previous.new_text = change.new_text;
                previous.new_range = change.new_range;
                self.redo_stack.clear();
                return;
            }
        }
        self.undo_stack.push(change);
        self.redo_stack.clear();
        self.last_groupable = groupable;
    }

    fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_groupable = false;
    }

    fn undo(&mut self) -> Option<Change> {
        self.last_groupable = false;
        self.undo_stack.pop()
    }

    fn redo(&mut self) -> Option<Change> {
        self.last_groupable = false;
        self.redo_stack.pop()
    }

    fn end_grouping(&mut self) {
        self.last_groupable = false;
    }
}

// ── InputState ────────────────────────────────────────────────────────────

pub struct InputState {
    pub(crate) text: Rope,
    pub(crate) selected_range: Selection,
    pub(crate) selection_reversed: bool,
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
    history: History,
    pub(crate) input_bounds: Bounds<Pixels>,
    pub(crate) last_layout: Option<LastLayout>,
    pub(crate) ime_marked_range: Option<Selection>,
    ime_initial_state: Option<(Rope, Selection)>,
    pub(crate) on_change: Option<Arc<dyn Fn(&str, &mut Window, &mut Context<Self>)>>,
    pub(crate) on_submit: Option<Arc<dyn Fn(&str, &mut Window, &mut Context<Self>)>>,
    pub(crate) on_blur: Option<Arc<dyn Fn(&mut Window, &mut Context<Self>)>>,

    // LSP stubs
    #[allow(dead_code)]
    pub(crate) lsp: Lsp,
    pub(crate) search_panel: Option<SearchPanel>,
    pub(crate) context_menu: Option<ContextMenu>,
    #[allow(dead_code)]
    pub(crate) hover_popover: Option<HoverPopover>,
    #[allow(dead_code)]
    pub(crate) diagnostic_popover: Option<DiagnosticPopover>,
    #[allow(dead_code)]
    pub(crate) hover_definition: HoverDefinition,
    pub(crate) inline_completion: InlineCompletion,

    // Internal state
    pub(crate) loading: bool,
    pub(crate) mask_pattern: MaskPattern,
    pattern: Option<regex::Regex>,
    validate: Option<Rc<dyn Fn(&str, &mut Context<Self>) -> bool>>,
    pub(crate) search_matcher: Option<super::search::SearchMatcher>,
    pub(crate) number_min: Option<f64>,
    pub(crate) number_max: Option<f64>,
    pub(crate) number_step: f64,

    pub(crate) _pending_update: bool,
    pub(crate) soft_wrap_enabled: bool,
    _subscriptions: Vec<Subscription>,
}

/// Convert byte index to char index for a string
fn byte_to_char_idx_str(s: &str, byte_idx: usize) -> usize {
    let byte_idx = byte_idx.min(s.len());
    let mut char_idx = 0;
    for (i, _) in s.char_indices() {
        if i >= byte_idx {
            break;
        }
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
            selection_reversed: false,
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
            ime_marked_range: None,
            ime_initial_state: None,
            on_change: None,
            on_submit: None,
            on_blur: None,
            lsp: Lsp::default(),
            search_panel: None,
            context_menu: None,
            hover_popover: None,
            diagnostic_popover: None,
            hover_definition: HoverDefinition::default(),
            inline_completion: InlineCompletion::default(),
            loading: false,
            mask_pattern: MaskPattern::None,
            pattern: None,
            validate: None,
            search_matcher: None,
            number_min: None,
            number_max: None,
            number_step: 1.0,
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

    pub fn auto_grow(mut self, min_rows: usize, max_rows: usize) -> Self {
        self.mode = InputMode::auto_grow(min_rows.max(1), max_rows.max(min_rows).max(1));
        self.soft_wrap_enabled = true;
        self
    }

    pub fn code_editor(mut self, language: impl Into<SharedString>) -> Self {
        self.mode = InputMode::code_editor(language);
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.mode.set_rows(rows.max(1));
        self
    }

    pub fn line_number(mut self, enabled: bool) -> Self {
        self.mode.set_line_number(enabled);
        self
    }

    pub fn folding(mut self, enabled: bool) -> Self {
        self.mode.set_folding(enabled);
        self
    }
    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap_enabled = soft_wrap;
        self
    }

    pub fn searchable(mut self, searchable: bool) -> Self {
        if searchable {
            self.search_matcher = Some(super::search::SearchMatcher::new());
        } else {
            self.search_matcher = None;
        }
        self
    }

    pub fn default_value(mut self, value: SharedString) -> Self {
        self.text = Rope::from(value.to_string());
        self.selected_range = (self.text.len()..self.text.len()).into();
        self.text_wrapper.set_default_text(&self.text);
        self
    }

    pub fn pattern(mut self, pattern: regex::Regex) -> Self {
        self.pattern = Some(pattern);
        self
    }

    pub fn validate(
        mut self,
        validate: impl Fn(&str, &mut Context<Self>) -> bool + 'static,
    ) -> Self {
        self.validate = Some(Rc::new(validate));
        self
    }

    pub fn mask_pattern(mut self, pattern: impl Into<MaskPattern>) -> Self {
        self.mask_pattern = pattern.into();
        if let Some(placeholder) = self.mask_pattern.placeholder() {
            self.placeholder = placeholder.into();
        }
        self
    }

    pub fn value(&self) -> SharedString {
        self.text.to_string().into()
    }

    pub fn selected_value(&self) -> SharedString {
        self.text
            .slice::<Range<usize>>(self.selected_range.into())
            .to_string()
            .into()
    }

    pub fn unmask_value(&self) -> SharedString {
        self.mask_pattern.unmask(&self.text.to_string()).into()
    }

    pub fn text(&self) -> &Rope {
        &self.text
    }

    pub fn selected_range(&self) -> Range<usize> {
        self.selected_range.into()
    }

    pub fn scroll_offset(&self) -> Point<Pixels> {
        self.scroll_handle.offset()
    }

    pub fn set_scroll_offset(&mut self, offset: Point<Pixels>, cx: &mut Context<Self>) {
        self.update_scroll_offset(offset, cx);
    }

    pub fn visible_row_range(&self) -> Option<Range<usize>> {
        self.last_layout
            .as_ref()
            .map(|layout| layout.visible_range.clone())
    }

    pub fn set_selected_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let start = self.text.clip_offset(range.start, Bias::Left);
        let end = self.text.clip_offset(range.end, Bias::Right);
        self.selected_range = Selection::new(start.min(end), end.max(start));
        self.selection_reversed = range.start > range.end;
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }
    pub fn set_value(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        self.history.end_grouping();
        self.apply_edit(0..self.text.len(), value, true, true, window, cx);
    }

    pub fn reset_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        let s: SharedString = value.into();
        self.text = Rope::from(s.to_string());
        self.selected_range = (self.text.len()..self.text.len()).into();
        self.selection_reversed = false;
        self.ime_marked_range = None;
        self.ime_initial_state = None;
        self.history.clear();
        self.text_wrapper.reset(&self.text, cx);
        self.mode.update_auto_grow(&self.text_wrapper);
        if let Some(matcher) = &mut self.search_matcher {
            matcher.update(&self.text);
        }
        self.scroll_cursor_into_view(cx);
        cx.notify();
    }

    pub fn cursor(&self) -> usize {
        // During IME composition, cursor is at the end of the marked range
        if let Some(ime_marked_range) = &self.ime_marked_range {
            return ime_marked_range.end;
        }
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.len() == 0
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle);
        // The platform IME handler is registered in TextElement::paint. Request
        // the next frame immediately after focus so input source changes and IME
        // composition never wait for an unrelated redraw.
        cx.notify();
    }

    pub(super) fn can_edit(&self) -> bool {
        !self.disabled && !self.read_only
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub fn clean(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        self.set_value("", window, cx);
    }

    pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.masked = masked;
        cx.notify();
    }

    pub fn is_masked(&self) -> bool {
        self.masked
    }

    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.loading = loading;
        cx.notify();
    }
    pub fn set_soft_wrap(&mut self, soft_wrap: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.soft_wrap_enabled = soft_wrap;
        let wrap_width = if soft_wrap {
            let w = self.input_bounds.size.width;
            if w > px(0.0) { Some(w) } else { None }
        } else {
            None
        };
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

    pub fn set_folding(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.mode.set_folding(enabled);
        self.text_wrapper.reset(&self.text, cx);
        cx.notify();
    }

    pub fn fold_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> bool {
        let start = self
            .text
            .clip_offset(range.start.min(self.text.len()), Bias::Left);
        let end = self
            .text
            .clip_offset(range.end.min(self.text.len()), Bias::Right);
        let folded = self.mode.fold_range(start.min(end)..start.max(end));
        if folded {
            if self.cursor() > start && self.cursor() < end {
                self.selected_range = (start..start).into();
                self.selection_reversed = false;
            }
            cx.notify();
        }
        folded
    }

    pub fn unfold_all(&mut self, cx: &mut Context<Self>) {
        self.mode.unfold_all();
        cx.notify();
    }

    pub fn set_highlights(
        &mut self,
        highlights: impl IntoIterator<Item = HighlightSpan>,
        cx: &mut Context<Self>,
    ) {
        self.mode.set_highlights(highlights);
        cx.notify();
    }

    pub fn set_diagnostics(
        &mut self,
        diagnostics: impl IntoIterator<Item = InputDiagnostic>,
        cx: &mut Context<Self>,
    ) {
        if let Some(current) = self.mode.diagnostics_mut() {
            current.set(diagnostics);
            cx.notify();
        }
    }

    /// Update the query used by the embedded search panel.
    ///
    /// `case_sensitive` follows the usual editor convention: `false` matches
    /// ASCII text without regard to case.
    pub fn set_search_query(
        &mut self,
        query: &str,
        case_sensitive: bool,
        cx: &mut Context<Self>,
    ) -> usize {
        let cursor = self.cursor();
        let matcher = self
            .search_matcher
            .get_or_insert_with(super::search::SearchMatcher::new);
        matcher.update(&self.text);
        matcher.update_query(query, !case_sensitive);
        matcher.update_cursor_by_offset(cursor);
        let count = matcher.len();
        cx.notify();
        count
    }

    pub fn search_match_count(&self) -> usize {
        self.search_matcher
            .as_ref()
            .map_or(0, |matcher| matcher.len())
    }

    pub fn next_match(&mut self, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let range = self.search_matcher.as_mut()?.next()?;
        self.selected_range = range.clone().into();
        self.selection_reversed = false;
        self.scroll_cursor_into_view(cx);
        cx.notify();
        Some(range)
    }

    pub fn previous_match(&mut self, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let range = self.search_matcher.as_mut()?.next_back()?;
        self.selected_range = range.clone().into();
        self.selection_reversed = false;
        self.scroll_cursor_into_view(cx);
        cx.notify();
        Some(range)
    }

    pub fn replace_current_match(
        &mut self,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(matcher) = self.search_matcher.as_mut() else {
            return false;
        };
        let Some(range) = matcher
            .matched_ranges
            .get(matcher.current_match_ix)
            .cloned()
        else {
            return false;
        };
        matcher.replacing = true;
        self.apply_edit(range, replacement, true, true, window, cx)
    }

    pub fn replace_all_matches(
        &mut self,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let ranges = match &self.search_matcher {
            Some(matcher) if !matcher.matched_ranges.is_empty() => matcher.matched_ranges.clone(),
            _ => return 0,
        };
        let source = self.text.to_string();
        let mut result = String::with_capacity(source.len());
        let mut cursor = 0;
        for range in ranges.iter() {
            result.push_str(&source[cursor..range.start]);
            result.push_str(replacement);
            cursor = range.end;
        }
        result.push_str(&source[cursor..]);
        let count = ranges.len();
        if self.apply_edit(0..source.len(), &result, true, true, window, cx) {
            count
        } else {
            0
        }
    }

    fn on_focus(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| {
            cursor.start(cx);
        });
        cx.emit(InputEvent::Focus);
    }

    fn on_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| cursor.stop(cx));
        cx.emit(InputEvent::Blur);
        if let Some(on_blur) = &self.on_blur {
            on_blur(window, cx);
        }
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
        if !self.can_edit() {
            return;
        }
        let range = match range_utf16 {
            Some(sel) => self.range_from_utf16(&sel.range),
            None => self.selected_range.into(),
        };
        self.history.end_grouping();
        self.apply_edit(range, new_text, true, true, window, cx);
    }

    fn is_valid_input(&self, value: &str, cx: &mut Context<Self>) -> bool {
        if let Some(validate) = &self.validate
            && !validate(value, cx)
        {
            return false;
        }
        if !self.mask_pattern.is_valid(value) {
            return false;
        }
        self.pattern
            .as_ref()
            .is_none_or(|pattern| pattern.is_match(value))
    }

    fn apply_edit(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        record_history: bool,
        emit_change: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_edit() {
            return false;
        }
        let start = self
            .text
            .clip_offset(range.start.min(self.text.len()), Bias::Left);
        let end = self
            .text
            .clip_offset(range.end.min(self.text.len()), Bias::Right);
        let range = start.min(end)..start.max(end);
        let replacement = if self.mode.is_single_line() {
            new_text.replace(['\n', '\r'], " ")
        } else {
            new_text.to_string()
        };
        let old_document = self.text.clone();
        let old_text = self.text.slice(range.clone()).to_string();
        self.text.replace(range.clone(), &replacement);

        let mut new_end = range.start + replacement.len();
        if self.mode.is_single_line() {
            let pending = self.text.to_string();
            if !self.mask_pattern.is_none() {
                let masked = self.mask_pattern.mask(&pending);
                if !self.is_valid_input(&masked, cx) {
                    self.text = old_document;
                    return false;
                }
                let inserted_formatting = masked.len().saturating_sub(pending.len());
                self.text = Rope::from(masked.as_str());
                new_end = new_end
                    .saturating_add(inserted_formatting)
                    .min(self.text.len());
            } else if !self.is_valid_input(&pending, cx) {
                self.text = old_document;
                return false;
            }
        }
        self.selected_range = (new_end..new_end).into();
        self.selection_reversed = false;

        self.ime_marked_range = None;
        self.ime_initial_state = None;
        self.text_wrapper.reset(&self.text, cx);
        self.mode.update_auto_grow(&self.text_wrapper);
        if let Some(matcher) = &mut self.search_matcher {
            matcher.update(&self.text);
        }
        self.scroll_cursor_into_view(cx);
        if record_history {
            let groupable = range.is_empty() && replacement.chars().count() == 1;
            if self.mask_pattern.is_none() {
                self.history.push(
                    Change::new(
                        Selection::from(range.clone()),
                        &old_text,
                        self.selected_range,
                        &replacement,
                    ),
                    groupable,
                );
            } else {
                self.history.push(
                    Change::new(
                        Selection::new(0, old_document.len()),
                        &old_document.to_string(),
                        self.selected_range,
                        &self.text.to_string(),
                    ),
                    groupable,
                );
            }
        }
        if emit_change {
            if let Some(on_change) = self.on_change.clone() {
                let text = self.text.to_string();
                on_change(&text, window, cx);
            }
            cx.emit(InputEvent::Change);
        }
        cx.notify();
        true
    }

    fn sync_after_history_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection_reversed = false;
        self.ime_marked_range = None;
        self.ime_initial_state = None;
        self.text_wrapper.reset(&self.text, cx);
        self.mode.update_auto_grow(&self.text_wrapper);
        if let Some(matcher) = &mut self.search_matcher {
            matcher.update(&self.text);
        }
        self.scroll_cursor_into_view(cx);
        if let Some(on_change) = self.on_change.clone() {
            let text = self.text.to_string();
            on_change(&text, window, cx);
        }
        cx.emit(InputEvent::Change);
        cx.notify();
    }

    pub(crate) fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.clamp(0, self.text.len());
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range =
                Selection::new(self.selected_range.end, self.selected_range.start);
        }
        cx.notify();
    }

    pub(crate) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor()), cx);
    }

    pub(crate) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor()), cx);
    }

    pub(crate) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_multi_line() {
            let offset = self.start_of_line().saturating_sub(1);
            self.select_to(self.previous_boundary(offset), cx);
        }
    }

    pub(crate) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_multi_line() {
            let offset = (self.end_of_line() + 1).min(self.text.len());
            self.select_to(self.next_boundary(offset), cx);
        }
    }

    pub(crate) fn select_to_start(
        &mut self,
        _: &SelectToStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = (0..self.selected_range.end).into();
        cx.notify();
    }

    pub(crate) fn select_to_end(
        &mut self,
        _: &SelectToEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = (self.selected_range.start..self.text.len()).into();
        cx.notify();
    }

    pub(crate) fn select_to_start_of_line(
        &mut self,
        _: &SelectToStartOfLine,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.start_of_line();
        self.selected_range = (offset..self.selected_range.end).into();
        cx.notify();
    }

    pub(crate) fn select_to_end_of_line(
        &mut self,
        _: &SelectToEndOfLine,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.end_of_line();
        self.selected_range = (self.selected_range.start..offset).into();
        cx.notify();
    }

    pub(crate) fn select_to_previous_word(
        &mut self,
        _: &SelectToPreviousWordStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.previous_start_of_word();
        self.selected_range = (offset..self.selected_range.end).into();
        cx.notify();
    }

    pub(crate) fn select_to_next_word(
        &mut self,
        _: &SelectToNextWordEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub(crate) fn start_of_line_of_selection(
        &self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> usize {
        let start = self.selected_range.start;
        self.text.line_start_offset(self.text.byte_to_line(start))
    }

    pub(crate) fn previous_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        if offset == 0 {
            return 0;
        }
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
        if cursor == 0 {
            return 0;
        }
        let text = self.text.to_string();
        let mut pos = cursor;
        while pos > 0 {
            let prev = self.previous_boundary(pos);
            let ch = text
                .chars()
                .nth(byte_to_char_idx_str(&text, prev))
                .unwrap_or(' ');
            if !ch.is_whitespace() {
                break;
            }
            pos = prev;
        }
        while pos > 0 {
            let prev = self.previous_boundary(pos);
            let ch = text
                .chars()
                .nth(byte_to_char_idx_str(&text, prev))
                .unwrap_or(' ');
            if ch.is_whitespace() || !ch.is_alphanumeric() {
                break;
            }
            pos = prev;
        }
        pos
    }

    pub(crate) fn next_end_of_word(&self) -> usize {
        let cursor = self.cursor();
        if cursor >= self.text.len() {
            return self.text.len();
        }
        let text = self.text.to_string();
        let mut pos = cursor;
        while pos < self.text.len() {
            let ch = text
                .chars()
                .nth(byte_to_char_idx_str(&text, pos))
                .unwrap_or(' ');
            if ch.is_whitespace() || !ch.is_alphanumeric() {
                break;
            }
            pos = self.next_boundary(pos);
        }
        while pos < self.text.len() {
            let ch = text
                .chars()
                .nth(byte_to_char_idx_str(&text, pos))
                .unwrap_or(' ');
            if !ch.is_whitespace() {
                break;
            }
            pos = self.next_boundary(pos);
        }
        pos
    }

    pub(crate) fn range_to_utf16(&self, range: &Range<usize>) -> UTF16Selection {
        UTF16Selection {
            range: self.text.offset_to_offset_utf16(range.start)
                ..self.text.offset_to_offset_utf16(range.end),
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
            let text: String = self
                .text
                .slice::<std::ops::Range<usize>>(self.selected_range.into())
                .to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub(crate) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        if !self.selected_range.is_empty() {
            let text: String = self
                .text
                .slice::<std::ops::Range<usize>>(self.selected_range.into())
                .to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.apply_edit(self.selected_range.into(), "", true, true, window, cx);
        }
    }

    pub(crate) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        if let Some(clipboard) = cx.read_from_clipboard() {
            if let Some(text) = clipboard.text() {
                self.history.end_grouping();
                self.apply_edit(self.selected_range.into(), &text, true, true, window, cx);
            }
        }
    }

    pub(crate) fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        if let Some(change) = self.history.undo() {
            let total = self.text.len();
            let replaced_len = change.new_text.len();
            let old_len = change.old_text.len();

            let range = Self::clamped_replace_range(change.old_range.start, replaced_len, total);
            self.text.replace(range, &change.old_text);

            let new_total = self.text.len();
            let sel_end = change
                .old_range
                .start
                .saturating_add(old_len)
                .min(new_total);
            self.selected_range = (change.old_range.start.min(new_total)..sel_end).into();

            self.history.redo_stack.push(change);
            self.sync_after_history_change(window, cx);
        }
    }

    pub(crate) fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        if let Some(change) = self.history.redo() {
            let total = self.text.len();
            let old_len = change.old_text.len();
            let new_len = change.new_text.len();

            let range = Self::clamped_replace_range(change.old_range.start, old_len, total);
            self.text.replace(range, &change.new_text);

            let new_total = self.text.len();
            let sel_end = change
                .old_range
                .start
                .saturating_add(new_len)
                .min(new_total);
            self.selected_range = (change.old_range.start.min(new_total)..sel_end).into();

            self.history.undo_stack.push(change);
            self.sync_after_history_change(window, cx);
        }
    }

    /// Compute a clamped replace range for undo/redo that never exceeds `total`.
    fn clamped_replace_range(start: usize, replaced_len: usize, total: usize) -> Range<usize> {
        let start = start.min(total);
        let end = start.saturating_add(replaced_len).min(total);
        start..end
    }

    pub(crate) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            self.pause_blink_cursor(cx);
            return;
        }
        // If IME is composing, skip our deletion (IME already handled it by updating composition text)
        if self.ime_marked_range.is_some() {
            self.pause_blink_cursor(cx);
            return;
        }

        let selected_range = self.selected_range;
        if !selected_range.is_empty() {
            self.apply_edit(selected_range.into(), "", true, true, window, cx);
        } else if selected_range.start > 0 {
            let end = selected_range.start;
            let start = self.previous_boundary(end);
            self.apply_edit(start..end, "", true, true, window, cx);
        }
        self.pause_blink_cursor(cx);
    }

    pub(crate) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            self.pause_blink_cursor(cx);
            return;
        }
        // If IME is composing, skip our deletion
        if self.ime_marked_range.is_some() {
            self.pause_blink_cursor(cx);
            return;
        }

        let selected_range = self.selected_range;
        if !selected_range.is_empty() {
            self.apply_edit(selected_range.into(), "", true, true, window, cx);
        } else if selected_range.end < self.text.len() {
            let start = selected_range.end;
            let end = self.next_boundary(start);
            self.apply_edit(start..end, "", true, true, window, cx);
        }
        self.pause_blink_cursor(cx);
    }

    pub(crate) fn delete_to_beginning_of_line(
        &mut self,
        _: &DeleteToBeginningOfLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        let offset = self.start_of_line();
        let cursor = self.cursor();
        if offset < cursor {
            self.apply_edit(offset..cursor, "", true, true, window, cx);
        }
    }

    pub(crate) fn delete_to_end_of_line(
        &mut self,
        _: &DeleteToEndOfLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        let offset = self.end_of_line();
        let cursor = self.cursor();
        if cursor < offset {
            self.apply_edit(cursor..offset, "", true, true, window, cx);
        }
    }

    pub(crate) fn delete_to_previous_word_start(
        &mut self,
        _: &DeleteToPreviousWordStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        let offset = self.previous_start_of_word();
        let cursor = self.cursor();
        if offset < cursor {
            self.apply_edit(offset..cursor, "", true, true, window, cx);
        }
    }

    pub(crate) fn delete_to_next_word_end(
        &mut self,
        _: &DeleteToNextWordEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        let offset = self.next_end_of_word();
        let cursor = self.cursor();
        if cursor < offset {
            self.apply_edit(cursor..offset, "", true, true, window, cx);
        }
    }

    pub(crate) fn on_action_enter(
        &mut self,
        action: &Enter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode.is_multi_line() && !self.can_edit() {
            return;
        }
        if self.mode.is_multi_line() {
            self.apply_edit(self.selected_range.into(), "\n", true, true, window, cx);
        } else {
            if let Some(on_submit) = &self.on_submit {
                let text = self.text.to_string();
                on_submit(&text, window, cx);
            }
            cx.emit(InputEvent::PressEnter {
                secondary: action.secondary,
            });
        }
    }

    pub(crate) fn on_action_escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        // Canceling an IME composition restores the pre-composition document.
        // A cancel is not a business edit and therefore emits no Change event.
        if let Some((initial_text, initial_selection)) = self.ime_initial_state.take() {
            self.text = initial_text;
            self.selected_range = initial_selection;
            self.selection_reversed = false;
            self.ime_marked_range = None;
            self.text_wrapper.reset(&self.text, cx);
            self.mode.update_auto_grow(&self.text_wrapper);
            if let Some(matcher) = &mut self.search_matcher {
                matcher.update(&self.text);
            }
        } else if let Some(ime_range) = self.ime_marked_range.take()
            && !ime_range.is_empty()
            && ime_range.end <= self.text.len()
        {
            self.text.replace(ime_range.start..ime_range.end, "");
            self.selected_range = (ime_range.start..ime_range.start).into();
            self.selection_reversed = false;
            self.text_wrapper.reset(&self.text, cx);
            self.mode.update_auto_grow(&self.text_wrapper);
        }

        if let Some(ref mut panel) = self.search_panel {
            panel.open = false;
        }
        self.hide_context_menu(cx);
        cx.notify();
    }

    pub(crate) fn on_action_search(
        &mut self,
        _: &Search,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.search_matcher.is_none() {
            cx.propagate();
            return;
        }
        if let Some(panel) = &mut self.search_panel {
            panel.open = !panel.open;
            if panel.open {
                panel.query_input.read(cx).focus_handle.focus(window);
            }
            cx.notify();
            return;
        }

        let query_input = cx.new(|cx| InputState::new(window, cx).searchable(false));
        let replace_input = cx.new(|cx| InputState::new(window, cx).searchable(false));
        let query_subscription = cx.subscribe(
            &query_input,
            |this: &mut Self, query, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = query.read(cx).value();
                    let case_sensitive = this
                        .search_panel
                        .as_ref()
                        .is_some_and(|panel| !panel.case_insensitive);
                    this.set_search_query(query.as_ref(), case_sensitive, cx);
                }
            },
        );
        self._subscriptions.push(query_subscription);
        let selected = self.selected_value();
        self.search_panel = Some(SearchPanel::new(query_input.clone(), replace_input));
        if !selected.is_empty() {
            query_input.update(cx, |state, cx| {
                state.set_value(selected.as_ref(), window, cx)
            });
        }
        query_input.read(cx).focus_handle.focus(window);
        cx.notify();
    }

    pub(crate) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    pub(crate) fn go_to_definition(
        &mut self,
        _: &GoToDefinition,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        super::lsp::warn_unsupported("go to definition");
        cx.propagate();
    }

    pub(crate) fn toggle_code_actions(
        &mut self,
        _: &ToggleCodeActions,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        super::lsp::warn_unsupported("code actions");
        cx.propagate();
    }

    pub(crate) fn on_key_down(&mut self, _: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // Pause blink cursor on any key press, the blink will resume after PAUSE_DELAY
        self.pause_blink_cursor(cx);
    }

    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.focus(window, cx);
        if let Some(offset) = self.character_index_for_point(event.position, window, cx) {
            if event.modifiers.shift {
                self.select_to(offset, cx);
            } else if event.click_count >= 3 {
                self.select_line(offset, window, cx);
            } else if event.click_count == 2 {
                self.select_word(offset, window, cx);
            } else {
                self.move_to(offset, None, cx);
            }
        }
    }

    pub(crate) fn on_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        _event: &MouseMoveEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    pub(crate) fn on_drag_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(offset) = self.character_index_for_point(event.position, window, cx) {
            self.select_to(offset, cx);
        }
    }

    pub(crate) fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line_height = self
            .last_layout
            .as_ref()
            .map(|layout| layout.line_height)
            .unwrap_or(input_line_height(window));
        let previous = self.scroll_handle.offset();
        let delta = event.delta.pixel_delta(line_height);
        self.update_scroll_offset(previous + delta, cx);
        if self.scroll_handle.offset() != previous {
            cx.stop_propagation();
        }
    }

    fn update_scroll_offset(&mut self, mut offset: Point<Pixels>, cx: &mut Context<Self>) {
        if self.mode.is_single_line() {
            offset.y = px(0.0);
        } else {
            let line_height = self
                .last_layout
                .as_ref()
                .map(|layout| layout.line_height)
                .unwrap_or(px(20.0));
            let content_height =
                line_height * self.text_wrapper.len().max(1) as f32 + TEXT_PADDING * 2.;
            let min_y = (self.input_bounds.size.height - content_height).min(px(0.0));
            offset.y = offset.y.clamp(min_y, px(0.0));
        }
        offset.x = offset.x.min(px(0.0));
        self.scroll_handle.set_offset(offset);
        cx.notify();
    }

    fn scroll_cursor_into_view(&mut self, cx: &mut Context<Self>) {
        if self.mode.is_single_line() {
            let offset = self.scroll_handle.offset();
            if offset.y != px(0.0) {
                self.update_scroll_offset(point(offset.x, px(0.0)), cx);
            }
            return;
        }
        if self.input_bounds.size.height <= px(0.0) {
            return;
        }
        let line_height = self
            .last_layout
            .as_ref()
            .map(|layout| layout.line_height)
            .unwrap_or(px(20.0));
        let row = self.text_wrapper.display_row_for_offset(self.cursor());
        let mut offset = self.scroll_handle.offset();
        let cursor_top = row as f32 * line_height + offset.y + TEXT_PADDING;
        let cursor_bottom = cursor_top + line_height;
        if cursor_top < TEXT_PADDING {
            offset.y += TEXT_PADDING - cursor_top;
        } else if cursor_bottom > self.input_bounds.size.height - TEXT_PADDING {
            offset.y -= cursor_bottom - (self.input_bounds.size.height - TEXT_PADDING);
        }
        self.update_scroll_offset(offset, cx);
    }

    pub(crate) fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_height = input_line_height(window);
        let font_size = input_font_size(window);
        let segments = super::element::display_segments(self);
        let content_y = point.y
            - input_text_top(self.input_bounds, self.mode.is_multi_line(), window)
            - self.scroll_handle.offset().y;
        if content_y < px(0.0) {
            return None;
        }
        let display_row = (content_y / line_height).floor() as usize;
        let segment = segments.get(display_row)?;
        let line_number_width = if self.mode.line_number() {
            px(44.0)
        } else {
            px(0.0)
        };
        let rel_x = point.x
            - self.input_bounds.left()
            - TEXT_PADDING
            - line_number_width
            - self.scroll_handle.offset().x;
        if rel_x < px(0.0) {
            return Some(segment.start);
        }
        let display_text = if self.masked {
            std::iter::repeat('•')
                .take(segment.text.chars().count())
                .collect::<String>()
        } else {
            segment.text.clone()
        };
        let run = TextRun {
            len: display_text.len(),
            font: window.text_style().font(),
            color: crate::token::tokens(cx).foreground,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped =
            window
                .text_system()
                .shape_line(display_text.clone().into(), font_size, &[run], None);
        let display_offset = shaped.closest_index_for_x(rel_x).min(display_text.len());
        let local_offset = if self.masked {
            let char_count = display_text[..display_offset].chars().count();
            segment
                .text
                .char_indices()
                .nth(char_count)
                .map_or(segment.text.len(), |(offset, _)| offset)
        } else {
            display_offset
        };
        Some((segment.start + local_offset).min(segment.end))
    }

    pub(crate) fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let line_height = input_line_height(window);
        let font_size = input_font_size(window);
        let segments = super::element::display_segments(self);
        let (display_row, segment) = segments
            .iter()
            .enumerate()
            .rfind(|(_, segment)| range.start >= segment.start && range.start <= segment.end)?;
        let local_start = range
            .start
            .saturating_sub(segment.start)
            .min(segment.text.len());
        let local_end = range
            .end
            .saturating_sub(segment.start)
            .min(segment.text.len());
        let measure = |text: &str, window: &Window| {
            let run = TextRun {
                len: text.len(),
                font: window.text_style().font(),
                color: crate::token::tokens(cx).foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_line(text.to_string().into(), font_size, &[run], None)
                .width
        };
        let start_x = measure(&segment.text[..local_start], window);
        let end_x = measure(&segment.text[..local_end.max(local_start)], window);
        let line_number_width = if self.mode.line_number() {
            px(44.0)
        } else {
            px(0.0)
        };
        let origin = point(
            bounds.left() + TEXT_PADDING + line_number_width + self.scroll_handle.offset().x,
            input_text_top(bounds, self.mode.is_multi_line(), window)
                + self.scroll_handle.offset().y
                + line_height * display_row as f32,
        );
        Some(Bounds::new(
            point(origin.x + start_x, origin.y),
            size((end_x - start_x).max(px(1.0)), line_height),
        ))
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
            reversed: self.selection_reversed,
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
        let range = if let Some((initial_text, initial_selection)) = self.ime_initial_state.take() {
            self.history.end_grouping();
            self.text = initial_text;
            self.selected_range = initial_selection;
            self.text_wrapper.reset(&self.text, cx);
            Range::from(initial_selection)
        } else {
            range_utf16
                .as_ref()
                .map(|range| self.range_from_utf16(range))
                .unwrap_or_else(|| self.selected_range.into())
        };
        self.apply_edit(range, new_text, true, true, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }

        if self.ime_initial_state.is_none() {
            self.ime_initial_state = Some((self.text.clone(), self.selected_range));
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.ime_marked_range.map(|range| {
                let range = self.range_to_utf16(&(range.start..range.end)).range;
                self.range_from_utf16(&range)
            }))
            .unwrap_or(self.selected_range.into());

        if new_text.is_empty() {
            if let Some((initial_text, initial_selection)) = self.ime_initial_state.take() {
                self.text = initial_text;
                self.selected_range = initial_selection;
            } else {
                let start = range.start.min(self.text.len());
                let end = range.end.min(self.text.len());
                self.text.replace(start..end, "");
                self.selected_range = (start..start).into();
            }
            self.ime_marked_range = None;
        } else {
            let clamped_start = range.start.min(self.text.len());
            let clamped_end = range.end.min(self.text.len());
            self.text.replace(clamped_start..clamped_end, new_text);
            self.ime_marked_range = Some((clamped_start..clamped_start + new_text.len()).into());
            self.selected_range = new_selected_range_utf16
                .as_ref()
                .map(|selection| {
                    let composition = Rope::from(new_text);
                    let start = composition.offset_utf16_to_offset(selection.start);
                    let end = composition.offset_utf16_to_offset(selection.end);
                    Selection::from(clamped_start + start..clamped_start + end)
                })
                .unwrap_or_else(|| {
                    (clamped_start + new_text.len()..clamped_start + new_text.len()).into()
                });
        }
        self.selection_reversed = false;
        self.text_wrapper.reset(&self.text, cx);
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
        InputState::bounds_for_range(self, range_utf16, bounds, window, cx)
    }
    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        InputState::character_index_for_point(self, point, window, cx)
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
            .w_full()
            .h_full()
            .overflow_x_hidden()
            .child(
                super::element::TextElement::new(cx.entity().clone())
                    .placeholder(self.placeholder.clone()),
            )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::input::Change;
    use crate::components::input::cursor::Selection;
    use gpui::{MouseButton, TestAppContext, VisualTestContext};

    #[test]
    fn clamped_replace_range_start_exceeds_len() {
        let r = InputState::clamped_replace_range(5, 10, 8);
        assert_eq!(r, 5..8);
    }

    #[test]
    fn clamped_replace_range_start_beyond_total() {
        let r = InputState::clamped_replace_range(100, 10, 8);
        assert_eq!(r, 8..8);
    }

    #[test]
    fn clamped_replace_range_len_max_no_overflow() {
        let r = InputState::clamped_replace_range(3, usize::MAX, 10);
        assert_eq!(r, 3..10);
    }

    #[test]
    fn clamped_replace_range_normal() {
        let r = InputState::clamped_replace_range(2, 4, 10);
        assert_eq!(r, 2..6);
    }

    #[test]
    fn clamped_replace_range_start_at_zero() {
        let r = InputState::clamped_replace_range(0, 5, 10);
        assert_eq!(r, 0..5);
    }

    #[gpui::test]
    fn undo_with_out_of_bounds_change_does_not_panic(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.text = Rope::from("hi");
            let new_text = "x".repeat(100);
            state.history.undo_stack.push(Change::new(
                Selection::new(0, 0),
                "",
                Selection::new(0, 100),
                &new_text,
            ));
            state
        });

        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                state.undo(&Undo {}, window, cx);
            });
        });

        entity.read_with(cx, |state, _cx| {
            assert!(state.selected_range.start <= state.text.len());
            assert!(state.selected_range.end <= state.text.len());
        });
    }

    #[gpui::test]
    fn undo_uses_post_replace_length_for_selection(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.history.undo_stack.push(Change::new(
                Selection::new(0, 0),
                "abc",
                Selection::new(0, 0),
                "",
            ));
            state
        });

        cx.update(|window, cx| {
            entity.update(cx, |state, cx| state.undo(&Undo {}, window, cx));
        });

        entity.read_with(cx, |state, _cx| {
            assert_eq!(state.text.to_string(), "abc");
            assert_eq!(state.selected_range, Selection::new(0, 3));
        });
    }

    #[gpui::test]
    fn is_masked_reflects_set_masked(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            assert!(!state.is_masked());
            state.set_masked(true, window, cx);
            state
        });

        entity.read_with(cx, |state, _cx| {
            assert!(state.is_masked());
        });
    }

    #[gpui::test]
    fn redo_with_out_of_bounds_change_does_not_panic(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.text = Rope::from("hello world");
            let new_text = "x".repeat(50);
            state.history.redo_stack.push(Change::new(
                Selection::new(0, 5),
                "hello",
                Selection::new(0, 50),
                &new_text,
            ));
            state
        });

        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                state.redo(&Redo {}, window, cx);
            });
        });

        entity.read_with(cx, |state, _cx| {
            assert!(state.selected_range.start <= state.text.len());
            assert!(state.selected_range.end <= state.text.len());
        });
    }

    #[gpui::test]
    fn redo_uses_post_replace_length_for_selection(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.history.redo_stack.push(Change::new(
                Selection::new(0, 0),
                "",
                Selection::new(0, 3),
                "abc",
            ));
            state
        });

        cx.update(|window, cx| {
            entity.update(cx, |state, cx| state.redo(&Redo {}, window, cx));
        });

        entity.read_with(cx, |state, _cx| {
            assert_eq!(state.text.to_string(), "abc");
            assert_eq!(state.selected_range, Selection::new(0, 3));
        });
    }

    #[gpui::test]
    fn indent_and_outdent_do_not_move_read_only_selection(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.mode = InputMode::code_editor("rust");
            state.text = Rope::from("one\ntwo");
            state.selected_range = (0..7).into();
            state.set_read_only(true, cx);
            state
        });

        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                state.indent(&Indent {}, window, cx);
                state.outdent(&Outdent {}, window, cx);
            });
        });

        entity.read_with(cx, |state, _cx| {
            assert_eq!(state.text.to_string(), "one\ntwo");
            assert_eq!(state.selected_range, Selection::new(0, 7));
        });
    }

    #[gpui::test]
    fn multi_line_indent_expands_selection_by_all_insertions(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.mode = InputMode::code_editor("rust");
            state.text = Rope::from("one\ntwo");
            state.selected_range = (0..7).into();
            state
        });

        cx.update(|window, cx| {
            entity.update(cx, |state, cx| state.indent(&Indent {}, window, cx));
        });

        entity.read_with(cx, |state, _cx| {
            assert_eq!(state.text.to_string(), "  one\n  two");
            assert_eq!(state.selected_range, Selection::new(0, 11));
        });
    }

    // ── Write-protection regression tests (FIX-014) ──────────────────────

    /// Helper: create an entity with text "hello" and run a closure on it.
    fn with_input(
        cx: &mut TestAppContext,
        f: impl FnOnce(Entity<InputState>, &mut VisualTestContext),
    ) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.text = Rope::from("hello");
            state.selected_range = (5..5).into();
            state
        });
        f(entity, cx);
    }

    #[gpui::test]
    fn backspace_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.backspace(&Backspace {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn backspace_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.backspace(&Backspace {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn delete_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.selected_range = (0..0).into();
                    state.delete(&Delete {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn delete_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.selected_range = (0..0).into();
                    state.delete(&Delete {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn cut_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.selected_range = (0..3).into();
                    state.cut(&Cut {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn cut_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.selected_range = (0..3).into();
                    state.cut(&Cut {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn paste_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    cx.write_to_clipboard(ClipboardItem::new_string("world".to_string()));
                    state.paste(&Paste {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn paste_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    cx.write_to_clipboard(ClipboardItem::new_string("world".to_string()));
                    state.paste(&Paste {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn undo_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.history.undo_stack.push(Change::new(
                        Selection::new(0, 0),
                        "",
                        Selection::new(0, 5),
                        "hello",
                    ));
                    state.undo(&Undo {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn undo_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.history.undo_stack.push(Change::new(
                        Selection::new(0, 0),
                        "",
                        Selection::new(0, 5),
                        "hello",
                    ));
                    state.undo(&Undo {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn redo_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.history.redo_stack.push(Change::new(
                        Selection::new(0, 0),
                        "",
                        Selection::new(0, 5),
                        "hello",
                    ));
                    state.redo(&Redo {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn redo_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.history.redo_stack.push(Change::new(
                        Selection::new(0, 0),
                        "",
                        Selection::new(0, 5),
                        "hello",
                    ));
                    state.redo(&Redo {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn clean_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.clean(window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn clean_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.clean(window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn set_value_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.set_value("world", window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn set_value_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.set_value("world", window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn enter_newline_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.mode = state.mode.clone().multi_line(true);
                    state.on_action_enter(&Enter { secondary: false }, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn enter_newline_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.mode = state.mode.clone().multi_line(true);
                    state.on_action_enter(&Enter { secondary: false }, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn read_only_allows_copy(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.selected_range = (0..5).into();
                    state.copy(&Copy {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
            let clipboard = cx.read_from_clipboard();
            assert!(clipboard.is_some());
            assert_eq!(clipboard.unwrap().text(), Some("hello".to_string()));
        });
    }

    #[gpui::test]
    fn read_only_allows_selection(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|_window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.select_to(3, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.selected_range.start, 3);
                assert_eq!(state.selected_range.end, 5);
            });
        });
    }

    #[gpui::test]
    fn read_only_allows_movement(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|_window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.move_to(2, None, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.selected_range.start, 2);
                assert_eq!(state.selected_range.end, 2);
            });
        });
    }

    #[gpui::test]
    fn disabled_mouse_down_no_focus(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            let mut state = InputState::new(window, cx);
            state.text = Rope::from("hello");
            state.set_disabled(true, cx);
            state
        });

        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                let event = MouseDownEvent {
                    position: point(px(10.0), px(10.0)),
                    button: MouseButton::Left,
                    modifiers: gpui::Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                };
                state.on_mouse_down(&event, window, cx);
                assert!(!state.focus_handle.is_focused(window));
            });
        });
    }

    #[gpui::test]
    fn ime_replace_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.replace_text_in_range(Some(0..5), "world", window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn ime_replace_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.replace_text_in_range(Some(0..5), "world", window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn ime_replace_and_mark_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.replace_and_mark_text_in_range(Some(0..5), "world", None, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn ime_replace_and_mark_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.replace_and_mark_text_in_range(Some(0..5), "world", None, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn normal_mode_allows_editing(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.backspace(&Backspace {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hell");
            });
        });
    }

    #[gpui::test]
    fn delete_to_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.text = Rope::from("hello world");
                    state.selected_range = (11..11).into();
                    state.set_read_only(true, cx);
                    state.delete_to_beginning_of_line(&DeleteToBeginningOfLine {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello world");
            });
        });
    }

    #[gpui::test]
    fn delete_to_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.text = Rope::from("hello world");
                    state.selected_range = (11..11).into();
                    state.set_disabled(true, cx);
                    state.delete_to_beginning_of_line(&DeleteToBeginningOfLine {}, window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello world");
            });
        });
    }

    #[gpui::test]
    fn replace_text_in_range_silent_blocked_in_read_only(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    state.replace_text_in_range_silent(None, "world", window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn replace_text_in_range_silent_blocked_in_disabled(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_disabled(true, cx);
                    state.replace_text_in_range_silent(None, "world", window, cx);
                });
            });
            entity.read_with(cx, |state, _cx| {
                assert_eq!(state.text.to_string(), "hello");
            });
        });
    }

    #[gpui::test]
    fn set_disabled_and_set_read_only_notify(cx: &mut TestAppContext) {
        with_input(cx, |entity, cx| {
            cx.update(|_window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_read_only(true, cx);
                    assert!(state.read_only);
                    state.set_disabled(true, cx);
                    assert!(state.disabled);
                    state.set_read_only(false, cx);
                    assert!(!state.read_only);
                    state.set_disabled(false, cx);
                    assert!(!state.disabled);
                });
            });
        });
    }

    #[gpui::test]
    fn consecutive_typing_is_one_undo_group_and_new_edit_clears_redo(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                assert!(state.apply_edit(0..0, "a", true, true, window, cx));
                assert!(state.apply_edit(1..1, "b", true, true, window, cx));
                assert_eq!(state.value().as_ref(), "ab");
                assert_eq!(state.history.undo_stack.len(), 1);
                state.undo(&Undo, window, cx);
                assert_eq!(state.value().as_ref(), "");
                assert_eq!(state.history.redo_stack.len(), 1);
                assert!(state.apply_edit(0..0, "c", true, true, window, cx));
                state.redo(&Redo, window, cx);
                assert_eq!(state.value().as_ref(), "c");
            });
        });
    }

    #[gpui::test]
    fn compact_single_line_typing_keeps_vertical_offset_fixed(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(InputState::new);
        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                for height in [28.0, 32.0] {
                    state.text = Rope::new();
                    state.selected_range = Selection::default();
                    state.text_wrapper.reset(&state.text.clone(), cx);
                    state.input_bounds =
                        Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(height)));
                    state.scroll_handle.set_offset(point(px(0.0), px(0.0)));

                    for character in ["a", "b", "c"] {
                        let cursor = state.cursor();
                        assert!(state.apply_edit(
                            cursor..cursor,
                            character,
                            true,
                            true,
                            window,
                            cx,
                        ));
                        assert_eq!(state.scroll_offset().y, px(0.0));
                    }
                }
            });
        });
    }

    #[gpui::test]
    fn single_line_rejects_vertical_scroll_but_preserves_horizontal_offset(
        cx: &mut TestAppContext,
    ) {
        let (entity, cx) = cx.add_window_view(InputState::new);
        cx.update(|_window, cx| {
            entity.update(cx, |state, cx| {
                state.input_bounds =
                    Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(28.0)));
                state.set_scroll_offset(point(px(-12.0), px(-8.0)), cx);

                assert_eq!(state.scroll_offset(), point(px(-12.0), px(0.0)));
            });
        });
    }

    #[gpui::test]
    fn multi_line_keeps_vertical_scroll_clamping(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(InputState::new);
        cx.update(|_window, cx| {
            entity.update(cx, |state, cx| {
                state.mode = state.mode.clone().multi_line(true);
                state.text = Rope::from("one\ntwo\nthree");
                state.text_wrapper.reset(&state.text.clone(), cx);
                state.input_bounds =
                    Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(28.0)));
                state.set_scroll_offset(point(px(0.0), px(-100.0)), cx);

                assert_eq!(state.scroll_offset().y, px(-48.0));
            });
        });
    }

    #[gpui::test]
    fn reverse_unicode_selection_preserves_direction(cx: &mut TestAppContext) {
        let (entity, cx) = cx
            .add_window_view(|window, cx| InputState::new(window, cx).default_value("a💝b".into()));
        cx.update(|_, cx| {
            entity.update(cx, |state, cx| {
                state.set_selected_range(6..1, cx);
                assert_eq!(state.selected_range(), 1..6);
                assert_eq!(state.selected_value().as_ref(), "💝b");
                assert_eq!(state.cursor(), 1);
                assert!(state.selection_reversed);
            });
        });
    }

    #[gpui::test]
    fn mask_applies_to_typing_and_undo(cx: &mut TestAppContext) {
        let (entity, cx) =
            cx.add_window_view(|window, cx| InputState::new(window, cx).mask_pattern("99-99"));
        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                for digit in ["1", "2", "3", "4"] {
                    let cursor = state.cursor();
                    assert!(state.apply_edit(cursor..cursor, digit, true, true, window, cx));
                }
                assert_eq!(state.value().as_ref(), "12-34");
                assert_eq!(state.unmask_value().as_ref(), "1234");
                state.undo(&Undo, window, cx);
                assert_eq!(state.value().as_ref(), "");
            });
        });
    }

    #[gpui::test]
    fn ime_commit_is_one_edit_and_escape_restores_original(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            InputState::new(window, cx).default_value("hello".into())
        });
        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                state.set_selected_range(1..4, cx);
                state.replace_and_mark_text_in_range(None, "你", None, window, cx);
                assert_eq!(state.value().as_ref(), "h你o");
                state.on_action_escape(&Escape, window, cx);
                assert_eq!(state.value().as_ref(), "hello");
                assert_eq!(state.selected_range(), 1..4);

                state.replace_and_mark_text_in_range(None, "你", None, window, cx);
                state.replace_and_mark_text_in_range(None, "你好", None, window, cx);
                state.replace_text_in_range(None, "你好", window, cx);
                assert_eq!(state.value().as_ref(), "h你好o");
                assert_eq!(state.history.undo_stack.len(), 1);
                state.undo(&Undo, window, cx);
                assert_eq!(state.value().as_ref(), "hello");
            });
        });
    }

    #[gpui::test]
    fn replace_all_is_single_undoable_edit(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|window, cx| {
            InputState::new(window, cx)
                .default_value("foo 世界 foo".into())
                .searchable(true)
        });
        cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                assert_eq!(state.set_search_query("foo", true, cx), 2);
                assert_eq!(state.replace_all_matches("bar", window, cx), 2);
                assert_eq!(state.value().as_ref(), "bar 世界 bar");
                assert_eq!(state.history.undo_stack.len(), 1);
                state.undo(&Undo, window, cx);
                assert_eq!(state.value().as_ref(), "foo 世界 foo");
            });
        });
    }
}
