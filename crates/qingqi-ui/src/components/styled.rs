//! Style helper traits for qingqi-ui components.

use gpui::prelude::Refineable as _;
use gpui::{
    App, Div, Hsla, ParentElement, Pixels, Stateful, StyleRefinement, Styled, Window, div, px,
};
use serde::{Deserialize, Serialize};

#[inline(always)]
pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}
#[inline(always)]
pub fn v_flex() -> Div {
    div().flex().flex_col()
}

#[derive(Clone, Default, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub enum Size {
    Size(Pixels),
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
}

impl Size {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "xs" | "xsmall" => Size::XSmall,
            "sm" | "small" => Size::Small,
            "md" | "medium" => Size::Medium,
            "lg" | "large" => Size::Large,
            _ => Size::Medium,
        }
    }
    pub fn smaller(&self) -> Self {
        match self {
            Size::XSmall => Size::XSmall,
            Size::Small => Size::XSmall,
            Size::Medium => Size::Small,
            Size::Large => Size::Medium,
            Size::Size(p) => Size::Size((*p) * 0.8),
        }
    }
    pub fn larger(&self) -> Self {
        match self {
            Size::XSmall => Size::Small,
            Size::Small => Size::Medium,
            Size::Medium => Size::Large,
            Size::Large => Size::Large,
            Size::Size(p) => Size::Size((*p) * 1.2),
        }
    }
    pub fn input_px(&self) -> Pixels {
        match self {
            Size::Large => px(16.),
            Size::Medium => px(12.),
            Size::Small => px(8.),
            Size::XSmall => px(4.),
            Size::Size(p) => *p,
        }
    }
    pub fn input_py(&self) -> Pixels {
        match self {
            Size::Large => px(10.),
            Size::Medium => px(8.),
            Size::Small => px(2.),
            Size::XSmall => px(0.),
            Size::Size(p) => *p,
        }
    }
}

impl From<Pixels> for Size {
    fn from(size: Pixels) -> Self {
        Size::Size(size)
    }
}

pub trait Selectable: Sized {
    fn selected(self, selected: bool) -> Self;
    fn is_selected(&self) -> bool;
    fn secondary_selected(self, _: bool) -> Self {
        self
    }
}

pub trait Disableable {
    fn disabled(self, disabled: bool) -> Self;
}

pub trait Sizable: Sized {
    fn with_size(self, size: impl Into<Size>) -> Self;
    #[inline(always)]
    fn xsmall(self) -> Self {
        self.with_size(Size::XSmall)
    }
    #[inline(always)]
    fn small(self) -> Self {
        self.with_size(Size::Small)
    }
    #[inline(always)]
    fn large(self) -> Self {
        self.with_size(Size::Large)
    }
}

pub trait StyleSized<E: Styled> {
    fn button_text_size(self, size: Size) -> Self;
}

impl<E: Styled> StyleSized<E> for E {
    fn button_text_size(self, size: Size) -> Self {
        match size {
            Size::XSmall => self.text_xs(),
            Size::Small => self.text_sm(),
            _ => self.text_size(px(14.0)),
        }
    }
}

pub(crate) fn focus_ring(
    el: Stateful<Div>,
    is_focused: bool,
    margins: Pixels,
    _window: &Window,
    cx: &App,
) -> Stateful<Div> {
    if !is_focused {
        return el;
    }

    let border_focus = crate::token::tokens(cx).border_focus;
    let ring_bw = px(1.5);
    let inset = ring_bw + margins;

    el.child(
        div()
            .flex_none()
            .absolute()
            .top(-inset)
            .left(-inset)
            .right(-inset)
            .bottom(-inset)
            .border(ring_bw)
            .border_color(Hsla {
                a: 0.5,
                ..border_focus
            }),
    )
}

/// Extension trait adding `refine_style`, `h_flex`, `v_flex` etc to any `Styled` type.
pub trait StyledExt: Styled + Sized {
    fn refine_style(mut self, style: &StyleRefinement) -> Self {
        self.style().refine(style);
        self
    }
    #[inline(always)]
    fn h_flex(self) -> Self {
        self.flex().flex_row().items_center()
    }
    #[inline(always)]
    fn v_flex(self) -> Self {
        self.flex().flex_col()
    }
}

impl<E: Styled> StyledExt for E {}
