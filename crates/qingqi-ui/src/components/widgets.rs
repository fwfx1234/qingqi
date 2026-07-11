//! Lightweight widget stubs (Tag, Badge, Progress, DropdownMenu, Slider, SelectItem)
//! — migrated APIs from vendor/qingqi-ui with local minimal implementations.

use gpui::{
    AnyElement, App, Div, IntoElement, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, percentage, prelude::*, px,
    relative,
};

use super::styled::Size;
use super::theme::ActiveTheme;

// ── Tag ────────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct Tag {
    label: Option<SharedString>,
    variant: TagVariant,
    size: Size,
}

#[derive(Clone, Copy, Default)]
pub enum TagVariant {
    #[default]
    Primary,
    Info,
    Success,
    Warning,
    Danger,
    Secondary,
}

impl Tag {
    pub fn new() -> Self {
        Self {
            label: None,
            variant: TagVariant::Primary,
            size: Size::Medium,
        }
    }
    pub fn primary() -> Self {
        Self::new().with_variant(TagVariant::Primary)
    }
    pub fn info() -> Self {
        Self::new().with_variant(TagVariant::Info)
    }
    pub fn success() -> Self {
        Self::new().with_variant(TagVariant::Success)
    }
    pub fn warning() -> Self {
        Self::new().with_variant(TagVariant::Warning)
    }
    pub fn danger() -> Self {
        Self::new().with_variant(TagVariant::Danger)
    }
    pub fn secondary() -> Self {
        Self::new().with_variant(TagVariant::Secondary)
    }
    fn with_variant(mut self, v: TagVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn small(mut self) -> Self {
        self.size = Size::Small;
        self
    }
    pub fn child(mut self, s: impl Into<SharedString>) -> Self {
        self.label = Some(s.into());
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (bg, fg) = match self.variant {
            TagVariant::Primary => (cx.theme().primary(), cx.theme().primary_foreground()),
            TagVariant::Info => (cx.theme().info(), cx.theme().info_foreground()),
            TagVariant::Success => (cx.theme().success(), cx.theme().success_foreground()),
            TagVariant::Warning => (cx.theme().warning(), cx.theme().warning_foreground()),
            TagVariant::Danger => (cx.theme().danger(), cx.theme().danger_foreground()),
            TagVariant::Secondary => (cx.theme().secondary(), cx.theme().secondary_foreground()),
        };
        let h = if self.size == Size::Small {
            px(20.)
        } else {
            px(24.)
        };
        div()
            .flex()
            .items_center()
            .gap_1()
            .h(h)
            .px_2()
            .rounded(px(4.))
            .bg(bg)
            .text_color(fg)
            .text_size(px(11.))
            .children(self.label)
    }
}

// ── Badge ──────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct Badge {
    count: Option<usize>,
    dot: bool,
    child: Option<AnyElement>,
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
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.child = Some(el.into_any_element());
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .relative()
            .child(self.child.unwrap_or_else(|| div().into_any_element()))
            .when(self.dot, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .w(px(6.))
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(cx.theme().danger()),
                )
            })
            .when_some(self.count, |d, n| {
                d.child(
                    div()
                        .absolute()
                        .right(px(0.))
                        .top(px(0.))
                        .min_w(px(16.))
                        .h(px(16.))
                        .rounded(px(8.))
                        .bg(cx.theme().danger())
                        .text_color(gpui::white())
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.))
                        .child(format!("{}", n)),
                )
            })
    }
}

// ── Progress ───────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct Progress {
    value: f32,
    bg: Option<gpui::Hsla>,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            bg: None,
        }
    }
    pub fn value(mut self, v: f32) -> Self {
        self.value = v.clamp(0.0, 100.0);
        self
    }
    pub fn bg(mut self, c: impl Into<gpui::Hsla>) -> Self {
        self.bg = Some(c.into());
        self
    }
    pub fn h(mut self, h: Pixels) -> Self {
        self
    }
}

impl RenderOnce for Progress {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let bar_bg = self.bg.unwrap_or_else(|| cx.theme().primary());
        let track_bg = cx.theme().muted();
        div()
            .w_full()
            .h(px(6.))
            .rounded(px(3.))
            .bg(track_bg)
            .overflow_hidden()
            .child(div().h_full().w(relative(self.value / 100.0)).bg(bar_bg))
    }
}

// ── DropdownMenu ───────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct DropdownMenu {
    items: Vec<gpui::AnyElement>,
}

impl DropdownMenu {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    pub fn item(mut self, item: impl IntoElement) -> Self {
        self.items.push(item.into_any_element());
        self
    }
    pub fn popup_item(
        mut self,
        item: impl Into<crate::layer::context_menu::PopupMenuItem>,
    ) -> Self {
        // Store raw AnyElement for simplicity
        self.items.push(item.into().into_any_element());
        self
    }
}

impl RenderOnce for DropdownMenu {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().flex().flex_col().children(self.items)
    }
}

// ── Slider ─────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct Slider {
    value: f32,
    min: f32,
    max: f32,
}

impl Slider {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 100.0,
        }
    }
    pub fn value(mut self, v: f32) -> Self {
        self.value = v;
        self
    }
    pub fn min(mut self, v: f32) -> Self {
        self.min = v;
        self
    }
    pub fn max(mut self, v: f32) -> Self {
        self.max = v;
        self
    }
    pub fn horizontal(mut self) -> Self {
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let pct = (self.value - self.min) / (self.max - self.min).max(0.001);
        div()
            .w_full()
            .h(px(4.))
            .rounded(px(2.))
            .bg(cx.theme().muted())
            .child(
                div()
                    .h_full()
                    .w(relative(pct))
                    .rounded(px(2.))
                    .bg(cx.theme().primary()),
            )
    }
}

// ── SelectItem trait ──────────────────────────────────────────────────

pub trait SelectItem: Clone {
    type Value: Clone;
    fn title(&self) -> SharedString;
    fn display_title(&self) -> Option<gpui::AnyElement> {
        None
    }
    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.title().into_any_element()
    }
    fn value(&self) -> &Self::Value;
    fn matches(&self, query: &str) -> bool {
        self.title().to_lowercase().contains(&query.to_lowercase())
    }
}

pub type SliderEvent = ();

pub type SliderValue = f32;

#[derive(Clone)]
pub struct SliderState {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl std::default::Default for SliderState {
    fn default() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
        }
    }
}

impl SliderState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(mut self, v: f32) -> Self {
        self.value = v;
        self
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

    pub fn default_value(self, v: f32) -> Self {
        self.value(v)
    }
}

impl gpui::EventEmitter<()> for SliderState {}
