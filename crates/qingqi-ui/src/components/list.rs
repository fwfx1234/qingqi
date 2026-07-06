//! List component — local replacement for qingqi-ui::list.

use std::sync::atomic::{AtomicU64, Ordering};
use gpui::{
    prelude::*, App, IntoElement, Pixels, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, px,
};

use super::styled::StyledExt;

static LIST_ITEM_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(IntoElement)]
pub struct ListItem {
    base: gpui::Stateful<gpui::Div>,
    style: StyleRefinement,
}

impl ListItem {
    pub fn new(id: impl Into<gpui::ElementId>) -> Self {
        let d = div().id(id).h(px(32.)).px_3().gap_2().items_center().text_size(px(13.));
        Self { base: d, style: StyleRefinement::default() }
    }

    pub fn pl(mut self, padding: Pixels) -> Self {
        let uid = LIST_ITEM_COUNTER.fetch_add(1, Ordering::SeqCst);
        self.base = div()
            .id(gpui::ElementId::Name(format!("pli{}", uid).into()))
            .h(px(32.))
            .pl(padding)
            .pr(px(12.))
            .gap_2()
            .items_center()
            .text_size(px(13.));
        self
    }
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.base = self.base.child(child);
        self
    }
}

impl Styled for ListItem {
    fn style(&mut self) -> &mut StyleRefinement { &mut self.style }
}

impl RenderOnce for ListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.base.flex().refine_style(&self.style)
    }
}
