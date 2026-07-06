//! LSP integration stubs for the input field.

use anyhow::Result;
use gpui::{App, Context, Hsla, Task, Window};
use ropey::Rope;
use std::rc::Rc;

use crate::components::input::InputState;

pub mod completions;
pub mod definitions;
pub mod hover;
pub mod code_actions;
pub mod document_colors;

pub use completions::*;
pub use definitions::*;
pub use hover::*;
pub use code_actions::*;
pub use document_colors::*;

/// LSP ServerCapabilities
pub struct Lsp {
    pub completion_provider: Option<Rc<dyn CompletionProvider>>,
    pub code_action_providers: Vec<Rc<dyn CodeActionProvider>>,
    pub hover_provider: Option<Rc<dyn HoverProvider>>,
    pub definition_provider: Option<Rc<dyn DefinitionProvider>>,
    pub document_color_provider: Option<Rc<dyn DocumentColorProvider>>,

    document_colors: Vec<(lsp_types::Range, Hsla)>,
    _hover_task: Task<Result<()>>,
    _document_color_task: Task<Result<()>>,
}

impl Default for Lsp {
    fn default() -> Self {
        Self {
            completion_provider: None,
            code_action_providers: vec![],
            hover_provider: None,
            definition_provider: None,
            document_color_provider: None,
            document_colors: vec![],
            _hover_task: Task::ready(Ok(())),
            _document_color_task: Task::ready(Ok(())),
        }
    }
}

impl Lsp {
    pub(crate) fn update(&mut self, _text: &Rope, _window: &mut Window, _cx: &mut Context<InputState>) {
        // Stub
    }

    pub(crate) fn reset(&mut self) {
        self.document_colors.clear();
        self._hover_task = Task::ready(Ok(()));
        self._document_color_task = Task::ready(Ok(()));
    }
}
