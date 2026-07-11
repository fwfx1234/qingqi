use gpui::*;

use crate::token::tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

pub struct Tooltip {
    pub content: SharedString,
    pub side: Side,
}

impl Tooltip {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            side: Side::Bottom,
        }
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);
        div()
            .absolute()
            .mt(px(4.0))
            .bg(token.surface)
            .rounded(px(6.0))
            .border_1()
            .border_color(token.border)
            .shadow_md()
            .px_2()
            .py_1()
            .text_xs()
            .text_color(token.muted_foreground)
            .child(self.content)
    }
}
