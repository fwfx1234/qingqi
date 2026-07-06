//! Input 组件 — 完整复刻 qingqi-ui::input

mod blink_cursor;
mod change;
mod cursor;
mod element;
mod indent;
mod input;
mod keyboard;
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
pub use lsp::{Lsp, CompletionProvider, HoverProvider, DefinitionProvider, CodeActionProvider, DocumentColorProvider, InlineCompletion};
pub use mask_pattern::{MaskPattern, MaskToken};
pub use mode::InputMode;
pub use number_input::{NumberInput, NumberInputEvent, StepAction, Increment, Decrement};
pub use otp_input::{OtpInput, OtpState};
pub use rope_ext::{RopeExt, Point as RopePoint};
pub use search::SearchMatcher;
pub use state::{InputState, InputEvent, Position, LastLayout, init, CONTEXT};
pub use text_wrapper::{DisplayPoint, TextWrapper};

// Re-export action types
pub use state::{
    Enter, Backspace, Delete, Escape,
    Copy, Cut, Paste, Undo, Redo,
    SelectAll,
    MoveUp, MoveDown, MoveLeft, MoveRight,
    MoveHome, MoveEnd, MovePageUp, MovePageDown,
    MoveToStart, MoveToEnd,
    MoveToStartOfLine, MoveToEndOfLine,
    MoveToPreviousWord, MoveToNextWord,
    SelectToStartOfLine, SelectToEndOfLine,
    SelectToStart, SelectToEnd,
    SelectToPreviousWordStart, SelectToNextWordEnd,
    DeleteToBeginningOfLine, DeleteToEndOfLine,
    DeleteToPreviousWordStart, DeleteToNextWordEnd,
    Indent, Outdent, IndentInline, OutdentInline,
    ShowCharacterPalette, ToggleCodeActions, Search, GoToDefinition,
    SelectLeft, SelectRight, SelectUp, SelectDown,
};

// Re-export popover types
pub use popovers::{
    ContextMenu, HoverPopover, DiagnosticPopover, MouseContextMenu,
    CompletionMenu, CodeActionMenu,
};

// Re-export theme adapter
pub use theme_adapter::ThemeAdapter;

// Re-export Rope
pub use ropey::Rope;
