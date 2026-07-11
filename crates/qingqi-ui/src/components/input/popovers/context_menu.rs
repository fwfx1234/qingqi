//! Right-click context menu.

use gpui::prelude::FluentBuilder as _;
use gpui::{Context, Entity, IntoElement, Pixels, Render, Window, div, px};

use crate::components::input::InputState;

pub struct ContextMenuAction {
    pub label: String,
    pub action: String,
}

pub struct ContextMenuModel {
    pub is_open: bool,
    pub position: gpui::Point<Pixels>,
    pub actions: Vec<ContextMenuAction>,
}

impl ContextMenuModel {
    pub fn new(_editor: Entity<InputState>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            is_open: false,
            position: gpui::point(px(0.0), px(0.0)),
            actions: vec![],
        }
    }

    pub fn show(
        &mut self,
        _position: gpui::Point<Pixels>,
        _actions: Vec<ContextMenuAction>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.is_open = true;
    }

    pub fn hide(&mut self, _cx: &mut Context<Self>) {
        self.is_open = false;
    }
}

impl Render for ContextMenuModel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
