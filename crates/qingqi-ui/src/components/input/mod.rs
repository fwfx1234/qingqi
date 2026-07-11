//! Input 组件 — 完整复刻 qingqi-ui::input

use gpui::{Bounds, Pixels, Rems, Window, px};

/// Horizontal padding from input border to text baseline.
pub const TEXT_PADDING: Pixels = px(8.0);

/// Default line height for the input field (in rems).
pub const LINE_HEIGHT_REMS: Rems = Rems(1.25);

/// Resolve the effective font size after the Input's inherited text style has
/// been applied. This keeps custom `.text_size()` values consistent with the
/// custom text element and the platform IME geometry.
pub(crate) fn input_font_size(window: &Window) -> Pixels {
    window.text_style().font_size.to_pixels(window.rem_size())
}

/// Input text line height in pixels. Keep custom painting, IME candidate bounds,
/// scrolling and the outer element on this single metric.
pub(crate) fn input_line_height(window: &Window) -> Pixels {
    window.rem_size() * LINE_HEIGHT_REMS.0
}

/// Top edge for the first text baseline box. Single-line fields center their
/// line box in the allocated height; multi-line editors retain their padding.
pub(crate) fn input_text_top(bounds: Bounds<Pixels>, multi_line: bool, window: &Window) -> Pixels {
    if multi_line {
        bounds.top() + TEXT_PADDING
    } else {
        bounds.top() + ((bounds.size.height - input_line_height(window)).max(px(0.0)) / 2.)
    }
}

mod blink_cursor;
mod change;
mod cursor;
mod element;
mod indent;
mod input;
mod lsp;
mod mask_pattern;
mod mode;
mod movement;
mod number_input;
mod otp_input;
pub(crate) mod popovers;
mod rope_ext;
mod search;
mod selection;
mod state;
mod text_wrapper;
mod theme_adapter;

// Re-export key types
pub(crate) use blink_cursor::BlinkCursor;
pub use change::Change;
pub use cursor::Selection;
pub use element::TextElement;
pub use indent::TabSize;
pub use input::Input;
pub use lsp::{
    CodeActionProvider, CompletionProvider, DefinitionProvider, DocumentColorProvider,
    HoverProvider, InlineCompletion, LSP_SUPPORTED, Lsp,
};
pub use mask_pattern::{MaskPattern, MaskToken};
pub use mode::{DiagnosticSet, DiagnosticSeverity, HighlightSpan, InputDiagnostic, InputMode};
pub use number_input::{Decrement, Increment, NumberInput, NumberInputEvent, StepAction};
pub use otp_input::{OtpInput, OtpState};
pub use rope_ext::{Point as RopePoint, RopeExt};
pub use search::SearchMatcher;
pub use state::{CONTEXT, InputEvent, InputState, LastLayout, Position, init};
pub use text_wrapper::{DisplayPoint, TextWrapper};

// Re-export action types
pub use state::{
    Backspace, Copy, Cut, Delete, DeleteToBeginningOfLine, DeleteToEndOfLine, DeleteToNextWordEnd,
    DeleteToPreviousWordStart, Enter, Escape, GoToDefinition, Indent, IndentInline, MoveDown,
    MoveEnd, MoveHome, MoveLeft, MovePageDown, MovePageUp, MoveRight, MoveToEnd, MoveToEndOfLine,
    MoveToNextWord, MoveToPreviousWord, MoveToStart, MoveToStartOfLine, MoveUp, Outdent,
    OutdentInline, Paste, Redo, Search, SelectAll, SelectDown, SelectLeft, SelectRight,
    SelectToEnd, SelectToEndOfLine, SelectToNextWordEnd, SelectToPreviousWordStart, SelectToStart,
    SelectToStartOfLine, SelectUp, ShowCharacterPalette, ToggleCodeActions, Undo,
};

// Re-export popover types
pub use popovers::{
    CodeActionItem, CodeActionMenu, CompletionMenu, ContextMenu, DiagnosticPopover, HoverPopover,
    MouseContextMenu,
};

// Re-export theme adapter
pub use theme_adapter::ThemeAdapter;

// Re-export Rope
pub use ropey::Rope;
