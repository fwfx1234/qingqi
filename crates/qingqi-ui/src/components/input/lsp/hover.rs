//! Hover provider trait.

use anyhow::Result;
use gpui::{Context, Task, Window};
use ropey::Rope;
use std::rc::Rc;

use super::InputState;

pub trait HoverProvider {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<Option<String>>>;
}
