//! Hover information popover.

use gpui::{Context, Entity, IntoElement, Render, Window, div};

use crate::components::input::InputState;

pub struct HoverPopoverModel {
    pub is_open: bool,
    pub content: Option<String>,
}

impl HoverPopoverModel {
    pub fn new(_editor: Entity<InputState>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            is_open: false,
            content: None,
        }
    }

    pub fn show(&mut self, _content: String, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_open = true;
    }

    pub fn hide(&mut self, _cx: &mut Context<Self>) {
        self.is_open = false;
    }
}

impl Render for HoverPopoverModel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
