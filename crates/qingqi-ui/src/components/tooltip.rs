use gpui::{
    div, prelude::FluentBuilder, App, IntoElement, ParentElement, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, px,
};

use crate::token::tokens;

#[derive(IntoElement)]
pub struct Tooltip {
    text: SharedString,
    style: StyleRefinement,
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Tooltip {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        div()
            .bg(t.popover)
            .border_1()
            .border_color(t.border)
            .rounded(px(6.0))
            .px_2()
            .py_1()
            .text_size(px(12.0))
            .text_color(t.foreground)
            .shadow_md()
            .child(self.text.clone())
            
    }
}
