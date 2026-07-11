//! Completion provider trait.

use anyhow::Result;
use gpui::{Context, Task};
use ropey::Rope;

use crate::components::input::InputState;

pub trait CompletionProvider {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        cx: &mut Context<InputState>,
    ) -> Task<Result<Vec<String>>>;

    fn is_completion_trigger(
        &self,
        _offset: usize,
        _new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        false
    }
}

pub struct InlineCompletion {
    pub item: Option<String>,
}

impl Default for InlineCompletion {
    fn default() -> Self {
        Self { item: None }
    }
}
