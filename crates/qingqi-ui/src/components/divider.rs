//! Divider component — local replacement for qingqi-ui::divider.

use gpui::{
    App, Axis, Div, Hsla, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _,
};

use super::styled::StyledExt;
use super::theme::ActiveTheme;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum DividerStyle {
    #[default]
    Solid,
    Dashed,
}

#[derive(IntoElement)]
pub struct Divider {
    base: Div,
    style: StyleRefinement,
    axis: Axis,
    color: Option<Hsla>,
    line_style: DividerStyle,
}

impl Divider {
    pub fn vertical() -> Self {
        Self {
            base: div().h_full(),
            axis: Axis::Vertical,
            color: None,
            style: StyleRefinement::default(),
            line_style: DividerStyle::Solid,
        }
    }

    pub fn horizontal() -> Self {
        Self {
            base: div(),
            axis: Axis::Horizontal,
            color: None,
            style: StyleRefinement::default(),
            line_style: DividerStyle::Solid,
        }
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn dashed(mut self) -> Self {
        self.line_style = DividerStyle::Dashed;
        self
    }
}

impl Styled for Divider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Divider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or_else(|| cx.theme().border());
        self.base
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .refine_style(&self.style)
            .child(
                div()
                    .absolute()
                    .map(|d| match self.axis {
                        Axis::Vertical => d.w(gpui::px(1.)).h_full(),
                        Axis::Horizontal => d.h(gpui::px(1.)).w_full(),
                    })
                    .bg(color),
            )
    }
}
