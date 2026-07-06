use std::rc::Rc;

use gpui::{
    div, prelude::FluentBuilder, App, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, hsla, px,
};

use crate::token::tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabVariant {
    Default,
    Pill,
}

impl Default for TabVariant {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Clone, IntoElement)]
pub struct Tab {
    id: ElementId,
    label: SharedString,
    selected: bool,
    variant: TabVariant,
    on_click: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Tab {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            variant: TabVariant::Default,
            on_click: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Tab {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        let selected = self.selected;

        let mut tab = div()
            .id(self.id.clone())
            .px_3()
            .py_1p5()
            .text_size(px(13.0))
            .text_color(if selected { t.foreground } else { t.muted_foreground })
            .font_weight(if selected {
                gpui::FontWeight::SEMIBOLD
            } else {
                gpui::FontWeight::NORMAL
            })
            .cursor_pointer();

        match self.variant {
            TabVariant::Default => {
                tab = tab
                    .border_b_2()
                    .border_color(if selected { t.accent } else { gpui::hsla(0.0, 0.0, 0.0, 0.0) });
            }
            TabVariant::Pill => {
                tab = tab.rounded(px(6.0)).bg(if selected {
                    t.surface_active
                } else {
                    gpui::hsla(0.0, 0.0, 0.0, 0.0)
                });
            }
        }

        tab = tab.child(self.label.clone());

        if let Some(on_click) = self.on_click {
            tab.on_click(move |_, window, cx| {
                on_click(window, cx);
            })
        } else {
            tab
        }
    }
}

#[derive(IntoElement)]
pub struct TabBar {
    id: ElementId,
    tabs: Vec<Tab>,
    selected_index: Option<usize>,
    variant: TabVariant,
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    style: gpui::StyleRefinement,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            selected_index: None,
            variant: TabVariant::Default,
            on_click: None,
            style: gpui::StyleRefinement::default(),
        }
    }

    pub fn tab(mut self, tab: Tab) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn tabs(mut self, tabs: Vec<Tab>) -> Self {
        self.tabs = tabs;
        self
    }

    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = Some(index);
        self
    }

    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for TabBar {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TabBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        let selected = self.selected_index;

        div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .border_b_1()
            .border_color(t.border)
            .gap_1()
            .children(self.tabs.into_iter().enumerate().map(|(i, mut tab)| {
                tab.selected = selected == Some(i);
                tab.variant = self.variant;
                if let Some(on_click) = self.on_click.clone() {
                    tab.on_click = Some(Rc::new(move |window, cx| {
                        on_click(&i, window, cx);
                    }));
                }
                tab
            }))
            
    }
}
