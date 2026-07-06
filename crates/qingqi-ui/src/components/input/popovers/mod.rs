//! Popover menus for the input field.

use gpui::{App, IntoElement, Pixels, div};
use std::rc::Rc;

pub mod completion_menu;
pub mod code_action_menu;
pub mod context_menu;
pub mod hover_popover;
pub mod diagnostic_popover;

pub use completion_menu::*;
pub use code_action_menu::*;
pub use context_menu::*;
pub use hover_popover::*;
pub use diagnostic_popover::*;

#[derive(Clone)]
pub enum ContextMenu {
    Completion,
    CodeAction,
    MouseContext { position: gpui::Point<Pixels> },
}

impl ContextMenu {
    pub fn is_open(&self, _cx: &App) -> bool { true }
    pub fn render(&self) -> impl IntoElement { div() }
}

#[derive(Clone)]
pub struct HoverPopover;
impl HoverPopover { pub fn new() -> Self { Self } }

#[derive(Clone)]
pub struct DiagnosticPopover;
impl DiagnosticPopover { pub fn new() -> Self { Self } }

#[derive(Clone)]
pub struct MouseContextMenu {
    pub position: gpui::Point<Pixels>,
}
impl MouseContextMenu {
    pub fn new(position: gpui::Point<Pixels>) -> Self { Self { position } }
}
