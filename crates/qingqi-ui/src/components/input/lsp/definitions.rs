//! Definition provider trait (Go to Definition).

use anyhow::Result;
use gpui::{Context, Task, Window};
use ropey::Rope;
use std::rc::Rc;

use super::InputState;

pub trait DefinitionProvider {
    fn definition(
        &self,
        text: &Rope,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<Option<lsp_types::GotoDefinitionResponse>>>;
}

#[derive(Default)]
pub struct HoverDefinition {
    pub offset: Option<usize>,
}
