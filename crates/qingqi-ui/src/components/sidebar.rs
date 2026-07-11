//! Sidebar component — local replacement for qingqi-ui::sidebar.

use std::rc::Rc;

use super::styled::StyledExt;
use super::theme::ActiveTheme;
use gpui::{
    AnyElement, App, ClickEvent, ElementId, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::*, px,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn is_left(&self) -> bool {
        matches!(self, Side::Left)
    }
}

/// Trait for elements that can be collapsed inside a Sidebar.
pub trait Collapsible {
    fn collapsed(self, collapsed: bool) -> Self;
    fn is_collapsed(&self) -> bool;
}

// ── SidebarMenu ───────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct SidebarMenu {
    style: StyleRefinement,
    collapsed: bool,
    items: Vec<SidebarMenuItem>,
}

impl SidebarMenu {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            items: Vec::new(),
            collapsed: false,
        }
    }
    pub fn child(mut self, child: impl Into<SidebarMenuItem>) -> Self {
        self.items.push(child.into());
        self
    }
    pub fn children(
        mut self,
        children: impl IntoIterator<Item = impl Into<SidebarMenuItem>>,
    ) -> Self {
        self.items = children.into_iter().map(Into::into).collect();
        self
    }
}

impl Default for SidebarMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl Collapsible for SidebarMenu {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }
    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl Styled for SidebarMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SidebarMenu {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .refine_style(&self.style)
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                div()
                    .id(ElementId::Integer(ix as u64))
                    .child(item.collapsed(self.collapsed))
            }))
    }
}

// ── SidebarMenuItem ───────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct SidebarMenuItem {
    id: ElementId,
    label: SharedString,
    handler: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
    active: bool,
    collapsed: bool,
    disabled: bool,
}

impl SidebarMenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            id: ElementId::Integer(0),
            label: label.into(),
            handler: Rc::new(|_, _, _| {}),
            active: false,
            collapsed: false,
            disabled: false,
        }
    }
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handler = Rc::new(handler);
        self
    }
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
    fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }
}

impl From<&'static str> for SidebarMenuItem {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}
impl From<String> for SidebarMenuItem {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl Collapsible for SidebarMenuItem {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }
    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl RenderOnce for SidebarMenuItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_active = self.active;
        div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap_2()
            .h(px(32.))
            .rounded(px(6.))
            .px_3()
            .text_size(px(13.))
            .when(is_active, |d| {
                d.bg(cx.theme().sidebar_accent())
                    .text_color(cx.theme().sidebar_accent_foreground())
            })
            .when(!is_active, |d| d.text_color(cx.theme().foreground()))
            .when(self.disabled, |d| d.text_color(cx.theme().muted()))
            .child(self.label.clone())
            .when(!self.disabled, |d| {
                d.cursor_pointer()
                    .on_click(move |ev, window, cx| (self.handler)(ev, window, cx))
            })
    }
}

// ── Sidebar ───────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct Sidebar {
    style: StyleRefinement,
    content: Vec<SidebarMenu>,
    header: Option<AnyElement>,
    side: Side,
    collapsible: bool,
    collapsed: bool,
}

impl Sidebar {
    pub fn new(side: Side) -> Self {
        Self {
            style: StyleRefinement::default(),
            content: vec![],
            header: None,
            side,
            collapsible: true,
            collapsed: false,
        }
    }
    pub fn left() -> Self {
        Self::new(Side::Left)
    }
    pub fn right() -> Self {
        Self::new(Side::Right)
    }
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }
    pub fn child(mut self, child: SidebarMenu) -> Self {
        self.content.push(child);
        self
    }
    pub fn children(mut self, children: impl IntoIterator<Item = SidebarMenu>) -> Self {
        self.content.extend(children);
        self
    }
}

impl Styled for Sidebar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Sidebar {
    fn render(mut self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let w: Pixels = px(255.);
        div()
            .id("sidebar")
            .w(w)
            .flex_shrink_0()
            .h_full()
            .overflow_hidden()
            .relative()
            .bg(cx.theme().sidebar())
            .text_color(cx.theme().foreground())
            .border_color(cx.theme().border())
            .map(|d| match self.side {
                Side::Left => d.border_r_1(),
                Side::Right => d.border_l_1(),
            })
            .refine_style(&self.style)
            .when_some(self.header.take(), |d, header| {
                d.child(
                    div()
                        .id(ElementId::Name("header".into()))
                        .pt_3()
                        .px_3()
                        .gap_2()
                        .child(header),
                )
            })
            .child(
                div()
                    .id(ElementId::Name("content".into()))
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id(ElementId::Name("inner".into()))
                            .p_3()
                            .overflow_scroll()
                            .children(self.content.into_iter().enumerate().map(|(ix, c)| {
                                div()
                                    .id(ElementId::Integer(ix as u64))
                                    .mt_3()
                                    .child(c.collapsed(self.collapsed))
                            })),
                    ),
            )
    }
}
