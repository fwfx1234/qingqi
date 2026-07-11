//! Local implementations of qingqi-ui widgets that have no qingqi-ui equivalent.

use gpui::{
    App, Element, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Window, div, hsla,
    prelude::FluentBuilder, px,
};
use qingqi_ui::token::tokens;

// ── Badge ──

#[derive(IntoElement)]
pub struct Badge {
    count: Option<usize>,
    dot: bool,
    child: Option<gpui::AnyElement>,
}

impl Badge {
    pub fn new() -> Self {
        Self {
            count: None,
            dot: false,
            child: None,
        }
    }

    pub fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }

    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_element().into_any());
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);
        match (self.count, self.dot) {
            (Some(n), _) => {
                let badge = div()
                    .absolute()
                    .right(px(-4.0))
                    .top(px(-4.0))
                    .rounded(px(999.0))
                    .bg(token.danger)
                    .px(px(4.0))
                    .min_w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(9.0))
                    .text_color(gpui::white())
                    .child(format!("{n}"));
                match self.child {
                    Some(child) => div().relative().child(child).child(badge),
                    None => div().child(badge),
                }
            }
            (_, true) => {
                let dot_el = div()
                    .absolute()
                    .right(px(-3.0))
                    .top(px(-3.0))
                    .size(px(8.0))
                    .rounded(px(999.0))
                    .bg(token.danger);
                match self.child {
                    Some(child) => div().relative().child(child).child(dot_el),
                    None => div().child(dot_el),
                }
            }
            _ => match self.child {
                Some(child) => div().child(div().absolute().child(child)),
                None => div(),
            },
        }
    }
}

// ── Checkbox ──

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            label: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);
        let checked = self.checked;
        let checkbox = div()
            .id(self.id)
            .size(px(16.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(if checked { token.accent } else { token.border })
            .bg(if checked {
                token.accent
            } else {
                token.background
            })
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.0))
            .text_color(gpui::white())
            .when(checked, |c| c.child("✓"));

        match self.label {
            Some(label) => div().flex().items_center().gap_2().child(checkbox).child(
                div()
                    .text_size(px(13.0))
                    .text_color(token.foreground)
                    .child(label),
            ),
            None => div().child(checkbox),
        }
    }
}

// ── Slider ──

#[derive(Clone)]
pub struct SliderState {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl SliderState {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
        }
    }

    pub fn min(mut self, v: f32) -> Self {
        self.min = v;
        self
    }

    pub fn max(mut self, v: f32) -> Self {
        self.max = v;
        self
    }

    pub fn step(mut self, v: f32) -> Self {
        self.step = v;
        self
    }

    pub fn default_value(mut self, v: f32) -> Self {
        self.value = v;
        self
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

impl gpui::EventEmitter<()> for SliderState {}

#[derive(IntoElement)]
pub struct Slider {
    state: Entity<SliderState>,
    horizontal: bool,
    style: StyleRefinement,
}

impl Slider {
    pub fn new(state: &Entity<SliderState>) -> Self {
        Self {
            state: state.clone(),
            horizontal: false,
            style: StyleRefinement::default(),
        }
    }

    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);
        let state = self.state.read(cx);
        let pct = ((state.value - state.min) / (state.max - state.min)).clamp(0.0, 1.0);

        div().w_full().h(px(24.0)).flex().items_center().child(
            div()
                .flex_1()
                .h(px(4.0))
                .rounded(px(2.0))
                .bg(token.border)
                .relative()
                .child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(px(0.0))
                        .h_full()
                        .w(gpui::relative(pct))
                        .rounded(px(2.0))
                        .bg(token.accent),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(pct * 100.0))
                        .top(px(-6.0))
                        .size(px(16.0))
                        .rounded(px(8.0))
                        .bg(token.accent)
                        .border_2()
                        .border_color(token.background)
                        .ml(px(-8.0)),
                ),
        )
    }
}

// ── TabBar ──

#[derive(IntoElement)]
pub struct TabBar {
    id: ElementId,
    tabs: Vec<SharedString>,
    selected_index: usize,
    underline: bool,
    segmented: bool,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            selected_index: 0,
            underline: false,
            segmented: false,
        }
    }

    pub fn children(mut self, tabs: impl IntoIterator<Item: Into<SharedString>>) -> Self {
        self.tabs = tabs.into_iter().map(Into::into).collect();
        self
    }

    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn segmented(mut self) -> Self {
        self.segmented = true;
        self
    }
}

impl RenderOnce for TabBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);
        let selected = self.selected_index;

        if self.segmented {
            div()
                .id(self.id)
                .flex()
                .gap(px(2.0))
                .p(px(2.0))
                .rounded(qingqi_ui::theme::radius_md())
                .border_1()
                .border_color(token.border)
                .bg(token.muted)
                .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                    let active = i == selected;
                    div()
                        .px(px(12.0))
                        .py(px(4.0))
                        .rounded(qingqi_ui::theme::radius_sm())
                        .when(active, |d| d.bg(token.surface))
                        .text_size(px(12.0))
                        .text_color(token.foreground)
                        .child(tab.clone())
                }))
        } else {
            div()
                .id(self.id)
                .flex()
                .gap(px(4.0))
                .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                    let active = i == selected;
                    div()
                        .px(px(4.0))
                        .py(px(6.0))
                        .border_b_2()
                        .border_color(if active {
                            token.accent
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .text_size(px(12.0))
                        .text_color(if active {
                            token.foreground
                        } else {
                            token.muted_foreground
                        })
                        .child(tab.clone())
                }))
        }
    }
}

// ── Tag ──

#[derive(IntoElement)]
pub struct Tag {
    variant: TagVariant,
    child: Option<gpui::AnyElement>,
}

#[derive(Clone, Copy)]
pub enum TagVariant {
    Warning,
    Success,
    Info,
    Danger,
}

impl Tag {
    pub fn warning() -> Self {
        Self {
            variant: TagVariant::Warning,
            child: None,
        }
    }

    pub fn success() -> Self {
        Self {
            variant: TagVariant::Success,
            child: None,
        }
    }

    pub fn info() -> Self {
        Self {
            variant: TagVariant::Info,
            child: None,
        }
    }

    pub fn danger() -> Self {
        Self {
            variant: TagVariant::Danger,
            child: None,
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_element().into_any());
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);
        let (bg, fg) = match self.variant {
            TagVariant::Warning => (token.warning, gpui::black()),
            TagVariant::Success => (token.success, gpui::white()),
            TagVariant::Info => (token.info, gpui::white()),
            TagVariant::Danger => (token.danger, gpui::white()),
        };

        div()
            .px(px(6.0))
            .py(px(2.0))
            .rounded(px(4.0))
            .bg(bg)
            .text_size(px(10.0))
            .text_color(fg)
            .child(
                self.child
                    .map(|c| c.into_any_element())
                    .unwrap_or_else(|| div().into_any_element()),
            )
    }
}
