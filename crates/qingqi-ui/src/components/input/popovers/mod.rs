//! Popover menus for the input field.

use gpui::{App, IntoElement, Pixels, div};

pub mod code_action_menu;
pub mod completion_menu;

pub use code_action_menu::*;
pub use completion_menu::*;

#[derive(Clone)]
pub enum ContextMenu {
    Completion,
    CodeAction,
    MouseContext { position: gpui::Point<Pixels> },
}

impl ContextMenu {
    pub fn is_open(&self, _cx: &App) -> bool {
        true
    }
    pub fn render(&self) -> impl IntoElement {
        div()
    }
}

#[derive(Clone)]
pub struct HoverPopover;
impl HoverPopover {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone)]
pub struct DiagnosticPopover;
impl DiagnosticPopover {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone)]
pub struct MouseContextMenu {
    pub position: gpui::Point<Pixels>,
}
impl MouseContextMenu {
    pub fn new(position: gpui::Point<Pixels>) -> Self {
        Self { position }
    }
}
