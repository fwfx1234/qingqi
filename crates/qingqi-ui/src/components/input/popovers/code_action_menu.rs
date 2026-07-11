//! Code action menu popover.

use gpui::{Context, Entity, IntoElement, Render, Window, div};

use crate::components::input::InputState;

#[derive(Clone)]
pub struct CodeActionItem {
    pub provider_id: String,
    pub action: String,
}

pub struct CodeActionMenu {
    pub is_open: bool,
}

impl CodeActionMenu {
    pub fn new(_editor: Entity<InputState>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { is_open: false }
    }

    pub fn handle_action(
        &mut self,
        _action: Box<dyn gpui::Action>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> bool {
        false
    }

    pub fn show(
        &mut self,
        _cursor: usize,
        _items: Vec<CodeActionItem>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.is_open = true;
    }

    pub fn hide(&mut self, _cx: &mut Context<Self>) {
        self.is_open = false;
    }
}

impl Render for CodeActionMenu {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
