//! LSP integration stubs for the input field.

use std::rc::Rc;
use std::sync::Once;

pub mod code_actions;
pub mod completions;
pub mod definitions;
pub mod document_colors;
pub mod hover;

pub use code_actions::*;
pub use completions::*;
pub use definitions::*;
pub use document_colors::*;
pub use hover::*;

/// The compatibility types remain exported, but no LSP provider is executed in this release.
pub const LSP_SUPPORTED: bool = false;

static WARN_UNSUPPORTED: Once = Once::new();

pub(crate) fn warn_unsupported(feature: &str) {
    WARN_UNSUPPORTED.call_once(|| {
        eprintln!(
            "qingqi-ui input: LSP integration is disabled; `{feature}` was propagated to the parent"
        );
    });
}

/// LSP ServerCapabilities
pub struct Lsp {
    pub completion_provider: Option<Rc<dyn CompletionProvider>>,
    pub code_action_providers: Vec<Rc<dyn CodeActionProvider>>,
    pub hover_provider: Option<Rc<dyn HoverProvider>>,
    pub definition_provider: Option<Rc<dyn DefinitionProvider>>,
    pub document_color_provider: Option<Rc<dyn DocumentColorProvider>>,
}

impl Default for Lsp {
    fn default() -> Self {
        Self {
            completion_provider: None,
            code_action_providers: vec![],
            hover_provider: None,
            definition_provider: None,
            document_color_provider: None,
        }
    }
}
