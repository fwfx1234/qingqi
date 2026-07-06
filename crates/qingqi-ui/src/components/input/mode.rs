//! InputMode — defines the behavior and capabilities of the input field.

use std::rc::Rc;
use std::{cell::RefCell, ops::Range};

use gpui::{App, Context, SharedString};
use ropey::Rope;
use tree_sitter::InputEdit;

use super::indent::TabSize;

/// Stub type for syntax highlighter (simplified for standalone use).
pub struct SyntaxHighlighter;

impl SyntaxHighlighter {
    pub fn new(_language: &str) -> Self {
        Self
    }

    pub fn update(&mut self, _edit: Option<InputEdit>, _text: &Rope) {
        // Stub: no syntax highlighting in standalone mode
    }

    pub fn styles(&self, _range: &Range<usize>, _theme: &()) -> Vec<()> {
        vec![]
    }
}

/// Stub type for diagnostic set.
#[derive(Clone)]
pub struct DiagnosticSet;

impl DiagnosticSet {
    pub fn new(_text: &Rope) -> Self {
        Self
    }

    pub fn reset(&mut self, _text: &Rope) {}
}

/// Defines the mode (behavior) of the input field.
#[derive(Clone)]
pub enum InputMode {
    PlainText {
        multi_line: bool,
        tab: TabSize,
        rows: usize,
    },
    AutoGrow {
        rows: usize,
        min_rows: usize,
        max_rows: usize,
    },
    CodeEditor {
        multi_line: bool,
        tab: TabSize,
        rows: usize,
        line_number: bool,
        language: SharedString,
        indent_guides: bool,
        highlighter: Rc<RefCell<Option<SyntaxHighlighter>>>,
        diagnostics: DiagnosticSet,
    },
}

impl Default for InputMode {
    fn default() -> Self {
        InputMode::plain_text()
    }
}

impl InputMode {
    pub fn plain_text() -> Self {
        InputMode::PlainText {
            multi_line: false,
            tab: TabSize::default(),
            rows: 1,
        }
    }

    pub fn code_editor(language: impl Into<SharedString>) -> Self {
        InputMode::CodeEditor {
            rows: 2,
            multi_line: true,
            tab: TabSize::default(),
            language: language.into(),
            highlighter: Rc::new(RefCell::new(None)),
            line_number: true,
            indent_guides: true,
            diagnostics: DiagnosticSet::new(&Rope::new()),
        }
    }

    pub fn auto_grow(min_rows: usize, max_rows: usize) -> Self {
        InputMode::AutoGrow {
            rows: min_rows,
            min_rows,
            max_rows,
        }
    }

    pub fn multi_line(mut self, multi_line: bool) -> Self {
        match &mut self {
            InputMode::PlainText { multi_line: ml, .. } => *ml = multi_line,
            InputMode::CodeEditor { multi_line: ml, .. } => *ml = multi_line,
            InputMode::AutoGrow { .. } => {}
        }
        self
    }

    #[inline]
    pub fn is_single_line(&self) -> bool {
        !self.is_multi_line()
    }

    #[inline]
    pub fn is_code_editor(&self) -> bool {
        matches!(self, InputMode::CodeEditor { .. })
    }

    #[inline]
    pub fn is_auto_grow(&self) -> bool {
        matches!(self, InputMode::AutoGrow { .. })
    }

    #[inline]
    pub fn is_multi_line(&self) -> bool {
        match self {
            InputMode::PlainText { multi_line, .. } => *multi_line,
            InputMode::CodeEditor { multi_line, .. } => *multi_line,
            InputMode::AutoGrow { max_rows, .. } => *max_rows > 1,
        }
    }

    pub fn set_rows(&mut self, new_rows: usize) {
        match self {
            InputMode::PlainText { rows, .. } => { *rows = new_rows; }
            InputMode::CodeEditor { rows, .. } => { *rows = new_rows; }
            InputMode::AutoGrow { rows, min_rows, max_rows } => {
                *rows = new_rows.clamp(*min_rows, *max_rows);
            }
        }
    }

    pub fn update_auto_grow(&mut self, text_wrapper: &super::TextWrapper) {
        if self.is_single_line() {
            return;
        }
        let wrapped_lines = text_wrapper.len();
        self.set_rows(wrapped_lines);
    }

    pub fn rows(&self) -> usize {
        if !self.is_multi_line() {
            return 1;
        }
        match self {
            InputMode::PlainText { rows, .. } => *rows,
            InputMode::CodeEditor { rows, .. } => *rows,
            InputMode::AutoGrow { rows, .. } => *rows,
        }.max(1)
    }

    pub fn min_rows(&self) -> usize {
        match self {
            InputMode::AutoGrow { min_rows, .. } => *min_rows,
            _ => 1,
        }.max(1)
    }

    pub fn max_rows(&self) -> usize {
        if !self.is_multi_line() {
            return 1;
        }
        match self {
            InputMode::AutoGrow { max_rows, .. } => *max_rows,
            _ => usize::MAX,
        }
    }

    #[inline]
    pub fn line_number(&self) -> bool {
        match self {
            InputMode::CodeEditor { line_number, multi_line, .. } => *line_number && *multi_line,
            _ => false,
        }
    }

    #[inline]
    pub fn language(&self) -> Option<&SharedString> {
        match self {
            InputMode::CodeEditor { language, .. } => Some(language),
            _ => None,
        }
    }

    pub fn set_line_number(&mut self, line_number: bool) {
        match self {
            InputMode::CodeEditor { line_number: ln, .. } => *ln = line_number,
            _ => {}
        }
    }

    pub fn set_indent_guides(&mut self, indent_guides: bool) {
        match self {
            InputMode::CodeEditor { indent_guides: ig, .. } => *ig = indent_guides,
            _ => {}
        }
    }

    pub fn set_language(&mut self, language: impl Into<SharedString>) {
        match self {
            InputMode::CodeEditor { highlighter, language: lang, .. } => {
                *lang = language.into();
                *highlighter.borrow_mut() = None;
            }
            _ => {}
        }
    }

    pub(super) fn has_indent_guides(&self) -> bool {
        match self {
            InputMode::CodeEditor { indent_guides, multi_line, .. } => *indent_guides && *multi_line,
            _ => false,
        }
    }

    pub fn is_indentable(&self) -> bool {
        match self {
            InputMode::AutoGrow { .. } => false,
            _ => true,
        }
    }

    pub(super) fn tab_size(&self) -> TabSize {
        match self {
            InputMode::PlainText { tab, .. } => *tab,
            InputMode::CodeEditor { tab, .. } => *tab,
            _ => TabSize::default(),
        }
    }

    #[allow(unused)]
    pub(super) fn diagnostics(&self) -> Option<&DiagnosticSet> {
        match self {
            InputMode::CodeEditor { diagnostics, .. } => Some(diagnostics),
            _ => None,
        }
    }

    pub(super) fn diagnostics_mut(&mut self) -> Option<&mut DiagnosticSet> {
        match self {
            InputMode::CodeEditor { diagnostics, .. } => Some(diagnostics),
            _ => None,
        }
    }
}
