use gpui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side { #[default] Top, Bottom, Left, Right }

pub struct Tooltip {
    pub content: SharedString,
    pub side: Side,
}

impl Tooltip {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self { content: content.into(), side: Side::Bottom }
    }
}
