use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder,
};

use crate::token::tokens;

#[derive(IntoElement)]
pub struct Label {
    label: SharedString,
    style: StyleRefinement,
}

impl Label {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            style: StyleRefinement::default(),
        }
    }

    pub fn text_size(mut self, size: impl Into<gpui::Pixels>) -> Self {
        self.style = self.style.text_size(size.into());
        self
    }
}

impl Styled for Label {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        div().text_color(t.foreground).child(self.label.clone())
    }
}
