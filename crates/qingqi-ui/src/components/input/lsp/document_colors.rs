//! Document color provider trait.

use anyhow::Result;
use gpui::{App, Context, Hsla, Task, Window};
use lsp_types::ColorInformation;
use ropey::Rope;
use std::rc::Rc;

use crate::components::input::InputState;

pub trait DocumentColorProvider {
    fn document_colors(
        &self,
        text: &Rope,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<ColorInformation>>>;
}
