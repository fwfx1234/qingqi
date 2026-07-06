//! Diagnostic (error/warning) popover.

use gpui::{Context, Entity, IntoElement, Render, Window, div};

use crate::components::input::InputState;

pub struct DiagnosticPopoverModel {
    pub is_open: bool,
    pub message: Option<String>,
    pub severity: DiagnosticSeverity,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticPopoverModel {
    pub fn new(_editor: Entity<InputState>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { is_open: false, message: None, severity: DiagnosticSeverity::Error }
    }

    pub fn show(&mut self, _message: String, _severity: DiagnosticSeverity, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_open = true;
    }

    pub fn hide(&mut self, _cx: &mut Context<Self>) {
        self.is_open = false;
    }
}

impl Render for DiagnosticPopoverModel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
