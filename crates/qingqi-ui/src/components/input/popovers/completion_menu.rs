//! Completion menu popover.

use gpui::{App, Context, Entity, InteractiveElement as _, IntoElement, Render, Window, div, prelude::FluentBuilder as _};

use crate::components::input::InputState;

pub struct CompletionMenu {
    pub trigger_start_offset: Option<usize>,
    pub is_open: bool,
}

impl CompletionMenu {
    pub fn new(_editor: Entity<InputState>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { trigger_start_offset: None, is_open: false }
    }

    pub fn handle_action(&mut self, _action: Box<dyn gpui::Action>, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        false
    }

    pub fn show(&mut self, _cursor: usize, _items: Vec<String>, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_open = true;
    }

    pub fn hide(&mut self, _cx: &mut Context<Self>) {
        self.is_open = false;
    }
}

impl Render for CompletionMenu {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
