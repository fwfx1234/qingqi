//! Code action provider trait.

use anyhow::Result;
use gpui::{Context, Entity, Task, Window};

use crate::components::input::InputState;

pub trait CodeActionProvider {
    fn id(&self) -> &str;

    fn code_actions(
        &self,
        state: Entity<InputState>,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<Vec<lsp_types::CodeActionOrCommand>>>;

    fn perform_code_action(
        &self,
        state: Entity<InputState>,
        action: lsp_types::CodeActionOrCommand,
        resolve: bool,
        window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<()>>;
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct CodeActionItem {
    pub provider_id: String,
    pub action: lsp_types::CodeActionOrCommand,
}
