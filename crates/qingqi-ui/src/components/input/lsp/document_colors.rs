//! Document color provider trait.

use anyhow::Result;
use gpui::{App, Task, Window};
use lsp_types::ColorInformation;
use ropey::Rope;

pub trait DocumentColorProvider {
    fn document_colors(
        &self,
        text: &Rope,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<ColorInformation>>>;
}
