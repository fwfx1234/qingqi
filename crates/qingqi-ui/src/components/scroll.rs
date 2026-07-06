//! Scrollable extension trait — local replacement for qingqi-ui::scroll::ScrollableElement.

use gpui::{prelude::*, Div, IntoElement, Stateful, StatefulInteractiveElement, InteractiveElement, Element};

#[derive(Clone, Copy)]
pub enum ScrollbarAxis {
    Horizontal,
    Vertical,
    Both,
}

pub trait ScrollableElement: Sized {
    fn vertical_scrollbar(self) -> Self;
    fn overflow_scrollbar(self) -> Self;
}

impl<E: InteractiveElement + Element> ScrollableElement for Stateful<E> {
    fn vertical_scrollbar(self) -> Self { self.overflow_scroll() }
    fn overflow_scrollbar(self) -> Self { self.overflow_scroll() }
}

/// Extension trait for overflow_y_scrollbar (plugin compat).
pub trait ScrollbarExt: Sized {
    fn overflow_y_scrollbar(self) -> Self;
}

impl<E: InteractiveElement + Element> ScrollbarExt for Stateful<E> {
    fn overflow_y_scrollbar(self) -> Self { self.overflow_scroll() }
}

pub use super::scrollbar::ScrollbarShow;

impl ScrollbarExt for gpui::Div {
    fn overflow_y_scrollbar(self) -> Self {
        self.overflow_y_hidden().overflow_x_hidden()
    }
}

pub use super::scrollbar::Scrollbar;
