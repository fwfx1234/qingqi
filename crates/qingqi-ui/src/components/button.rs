//! Button component — local replacement for qingqi-ui::button.

use std::rc::Rc;

use super::icon::Icon;
use super::styled::focus_ring;
use super::styled::{Sizable, Size};
use super::theme::ActiveTheme;
use gpui::{
    AnyElement, App, ClickEvent, Corners, Div, Edges, ElementId, Hsla, InteractiveElement,
    Interactivity, IntoElement, MouseButton, ParentElement, Pixels, RenderOnce, SharedString,
    Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

#[derive(Default, Clone, Copy)]
pub enum ButtonRounded {
    None,
    Small,
    #[default]
    Medium,
    Large,
    Size(Pixels),
}

impl From<Pixels> for ButtonRounded {
    fn from(px: Pixels) -> Self {
        ButtonRounded::Size(px)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ButtonCustomVariant {
    color: Hsla,
    foreground: Hsla,
    border: Hsla,
    shadow: bool,
    hover: Hsla,
    active: Hsla,
}

pub trait ButtonVariants: Sized {
    fn with_variant(self, variant: ButtonVariant) -> Self;
    fn primary(self) -> Self {
        self.with_variant(ButtonVariant::Primary)
    }
    fn danger(self) -> Self {
        self.with_variant(ButtonVariant::Danger)
    }
    fn warning(self) -> Self {
        self.with_variant(ButtonVariant::Warning)
    }
    fn success(self) -> Self {
        self.with_variant(ButtonVariant::Success)
    }
    fn info(self) -> Self {
        self.with_variant(ButtonVariant::Info)
    }
    fn ghost(self) -> Self {
        self.with_variant(ButtonVariant::Ghost)
    }
    fn link(self) -> Self {
        self.with_variant(ButtonVariant::Link)
    }
    fn text(self) -> Self {
        self.with_variant(ButtonVariant::Text)
    }
    fn custom(self, style: ButtonCustomVariant) -> Self {
        self.with_variant(ButtonVariant::Custom(style))
    }
}

impl ButtonCustomVariant {
    pub fn new(cx: &App) -> Self {
        Self {
            color: cx.theme().transparent(),
            foreground: cx.theme().foreground(),
            border: cx.theme().transparent(),
            hover: cx.theme().transparent(),
            active: cx.theme().transparent(),
            shadow: false,
        }
    }
    pub fn color(mut self, c: Hsla) -> Self {
        self.color = c;
        self
    }
    pub fn foreground(mut self, c: Hsla) -> Self {
        self.foreground = c;
        self
    }
    pub fn border(mut self, c: Hsla) -> Self {
        self.border = c;
        self
    }
    pub fn hover(mut self, c: Hsla) -> Self {
        self.hover = c;
        self
    }
    pub fn active(mut self, c: Hsla) -> Self {
        self.active = c;
        self
    }
    pub fn shadow(mut self, s: bool) -> Self {
        self.shadow = s;
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    Primary,
    #[default]
    Secondary,
    Danger,
    Info,
    Success,
    Warning,
    Ghost,
    Link,
    Text,
    Custom(ButtonCustomVariant),
}

impl ButtonVariant {
    fn is_link(&self) -> bool {
        matches!(self, Self::Link)
    }
    fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }
    #[allow(dead_code)]
    fn is_ghost(&self) -> bool {
        matches!(self, Self::Ghost)
    }
    fn no_padding(&self) -> bool {
        self.is_link() || self.is_text()
    }

    fn bg_color(&self, outline: bool, cx: &mut App) -> Hsla {
        if outline {
            return cx.theme().background();
        }
        match self {
            Self::Primary => cx.theme().primary(),
            Self::Secondary => cx.theme().secondary(),
            Self::Danger => cx.theme().danger(),
            Self::Warning => cx.theme().warning(),
            Self::Success => cx.theme().success(),
            Self::Info => cx.theme().info(),
            Self::Ghost | Self::Link | Self::Text => cx.theme().transparent(),
            Self::Custom(colors) => colors.color,
        }
    }

    fn text_color(&self, outline: bool, cx: &mut App) -> Hsla {
        match self {
            Self::Primary => {
                if outline {
                    cx.theme().primary()
                } else {
                    cx.theme().primary_foreground()
                }
            }
            Self::Secondary | Self::Ghost => cx.theme().secondary_foreground(),
            Self::Danger => {
                if outline {
                    cx.theme().danger()
                } else {
                    cx.theme().danger_foreground()
                }
            }
            Self::Warning => {
                if outline {
                    cx.theme().warning()
                } else {
                    cx.theme().warning_foreground()
                }
            }
            Self::Success => {
                if outline {
                    cx.theme().success()
                } else {
                    cx.theme().success_foreground()
                }
            }
            Self::Info => {
                if outline {
                    cx.theme().info()
                } else {
                    cx.theme().info_foreground()
                }
            }
            Self::Link => cx.theme().link(),
            Self::Text => cx.theme().foreground(),
            Self::Custom(colors) => {
                if outline {
                    colors.color
                } else {
                    colors.foreground
                }
            }
        }
    }

    fn border_color(&self, bg: Hsla, outline: bool, cx: &mut App) -> Hsla {
        match self {
            Self::Secondary => {
                if outline {
                    cx.theme().border()
                } else {
                    bg
                }
            }
            Self::Primary => {
                if outline {
                    cx.theme().primary()
                } else {
                    bg
                }
            }
            Self::Danger => {
                if outline {
                    cx.theme().danger()
                } else {
                    bg
                }
            }
            Self::Info => {
                if outline {
                    cx.theme().info()
                } else {
                    bg
                }
            }
            Self::Warning => {
                if outline {
                    cx.theme().warning()
                } else {
                    bg
                }
            }
            Self::Success => {
                if outline {
                    cx.theme().success()
                } else {
                    bg
                }
            }
            Self::Ghost | Self::Link | Self::Text => cx.theme().transparent(),
            Self::Custom(colors) => colors.border,
        }
    }

    fn underline(&self, _: &App) -> bool {
        matches!(self, Self::Link)
    }
    fn shadow(&self, outline: bool, _: &App) -> bool {
        match self {
            Self::Primary | Self::Secondary | Self::Danger => outline,
            Self::Custom(c) => c.shadow,
            _ => false,
        }
    }

    fn normal(&self, outline: bool, cx: &mut App) -> StyledState {
        let bg = self.bg_color(outline, cx);
        let border = self.border_color(bg, outline, cx);
        let fg = self.text_color(outline, cx);
        StyledState {
            bg,
            border,
            fg,
            underline: self.underline(cx),
            shadow: self.shadow(outline, cx),
        }
    }

    fn hovered(&self, outline: bool, cx: &mut App) -> StyledState {
        let bg = match self {
            Self::Primary => {
                if outline {
                    cx.theme().secondary_hover()
                } else {
                    cx.theme().primary_hover()
                }
            }
            Self::Secondary => cx.theme().secondary_hover(),
            Self::Danger => {
                if outline {
                    cx.theme().secondary_hover()
                } else {
                    cx.theme().danger_hover()
                }
            }
            Self::Warning => {
                if outline {
                    cx.theme().secondary_hover()
                } else {
                    cx.theme().warning_hover()
                }
            }
            Self::Success => {
                if outline {
                    cx.theme().secondary_hover()
                } else {
                    cx.theme().success_hover()
                }
            }
            Self::Info => {
                if outline {
                    cx.theme().secondary_hover()
                } else {
                    cx.theme().info_hover()
                }
            }
            Self::Ghost => {
                if cx.theme().is_dark() {
                    {
                        let mut c = cx.theme().secondary();
                        c.l = (c.l + 0.1).clamp(0.0, 1.0);
                        c.opacity(0.8)
                    }
                } else {
                    {
                        let mut c = cx.theme().secondary();
                        c.l = (c.l - 0.1).clamp(0.0, 1.0);
                        c.opacity(0.8)
                    }
                }
            }
            Self::Link | Self::Text => cx.theme().transparent(),
            Self::Custom(colors) => {
                if outline {
                    cx.theme().secondary_hover()
                } else {
                    colors.hover
                }
            }
        };
        let border = self.border_color(bg, outline, cx);
        let fg = if matches!(self, Self::Link) {
            cx.theme().link_hover()
        } else {
            self.text_color(outline, cx)
        };
        StyledState {
            bg,
            border,
            fg,
            underline: self.underline(cx),
            shadow: self.shadow(outline, cx),
        }
    }

    fn active(&self, outline: bool, cx: &mut App) -> StyledState {
        let bg = match self {
            Self::Primary => {
                if outline {
                    cx.theme().primary_active().opacity(0.1)
                } else {
                    cx.theme().primary_active()
                }
            }
            Self::Secondary => cx.theme().secondary_active(),
            Self::Ghost => {
                if cx.theme().is_dark() {
                    {
                        let mut c = cx.theme().secondary();
                        c.l = (c.l + 0.2).clamp(0.0, 1.0);
                        c.opacity(0.8)
                    }
                } else {
                    {
                        let mut c = cx.theme().secondary();
                        c.l = (c.l - 0.2).clamp(0.0, 1.0);
                        c.opacity(0.8)
                    }
                }
            }
            Self::Danger => {
                if outline {
                    cx.theme().danger_active().opacity(0.1)
                } else {
                    cx.theme().danger_active()
                }
            }
            Self::Warning => {
                if outline {
                    cx.theme().warning_active().opacity(0.1)
                } else {
                    cx.theme().warning_active()
                }
            }
            Self::Success => {
                if outline {
                    cx.theme().success_active().opacity(0.1)
                } else {
                    cx.theme().success_active()
                }
            }
            Self::Info => {
                if outline {
                    cx.theme().info_active().opacity(0.1)
                } else {
                    cx.theme().info_active()
                }
            }
            Self::Link | Self::Text => cx.theme().transparent(),
            Self::Custom(colors) => {
                if outline {
                    colors.active.opacity(0.1)
                } else {
                    colors.active
                }
            }
        };
        let border = self.border_color(bg, outline, cx);
        let fg = match self {
            Self::Link => cx.theme().link_active(),
            Self::Text => cx.theme().foreground().opacity(0.7),
            _ => self.text_color(outline, cx),
        };
        StyledState {
            bg,
            border,
            fg,
            underline: self.underline(cx),
            shadow: self.shadow(outline, cx),
        }
    }

    fn selected(&self, outline: bool, cx: &mut App) -> StyledState {
        let bg = match self {
            Self::Primary => cx.theme().primary_active(),
            Self::Secondary | Self::Ghost => cx.theme().secondary_active(),
            Self::Danger => cx.theme().danger_active(),
            Self::Warning => cx.theme().warning_active(),
            Self::Success => cx.theme().success_active(),
            Self::Info => cx.theme().info_active(),
            Self::Link | Self::Text => cx.theme().transparent(),
            Self::Custom(colors) => colors.active,
        };
        let border = self.border_color(bg, outline, cx);
        let fg = match self {
            Self::Link => cx.theme().link_active(),
            Self::Text => cx.theme().foreground().opacity(0.7),
            _ => self.text_color(false, cx),
        };
        StyledState {
            bg,
            border,
            fg,
            underline: self.underline(cx),
            shadow: self.shadow(outline, cx),
        }
    }

    fn disabled(&self, outline: bool, cx: &mut App) -> StyledState {
        let bg = match self {
            Self::Link | Self::Ghost | Self::Text => cx.theme().transparent(),
            Self::Primary => cx.theme().primary().opacity(0.15),
            Self::Danger => cx.theme().danger().opacity(0.15),
            Self::Warning => cx.theme().warning().opacity(0.15),
            Self::Success => cx.theme().success().opacity(0.15),
            Self::Info => cx.theme().info().opacity(0.15),
            Self::Secondary => cx.theme().secondary(),
            Self::Custom(style) => style.color.opacity(0.15),
        };
        let fg = cx.theme().muted_foreground.opacity(0.5);
        let (bg, border) = if outline {
            (cx.theme().transparent(), cx.theme().border().opacity(0.5))
        } else {
            (bg, bg)
        };
        StyledState {
            bg,
            border,
            fg,
            underline: self.underline(cx),
            shadow: false,
        }
    }
}

struct StyledState {
    bg: Hsla,
    border: Hsla,
    fg: Hsla,
    underline: bool,
    #[allow(dead_code)]
    shadow: bool,
}

// ── Button ─────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    base: Stateful<Div>,
    style: StyleRefinement,
    icon: Option<Icon>,
    label: Option<SharedString>,
    children: Vec<AnyElement>,
    disabled: bool,
    pub(crate) selected: bool,
    variant: ButtonVariant,
    rounded: ButtonRounded,
    outline: bool,
    #[allow(dead_code)]
    border_corners: Corners<bool>,
    #[allow(dead_code)]
    border_edges: Edges<bool>,
    size: Size,
    compact: bool,
    tooltip: Option<SharedString>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_hover: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
    loading: bool,
    dropdown_menu_fn: Option<
        Rc<
            dyn Fn(
                super::widgets::DropdownMenu,
                &mut Window,
                &mut App,
            ) -> super::widgets::DropdownMenu,
        >,
    >,
}

impl From<Button> for AnyElement {
    fn from(button: Button) -> Self {
        button.into_any_element()
    }
}

impl Button {
    pub fn new(id: impl Into<ElementId> + Clone) -> Self {
        Self {
            id: id.clone().into(),
            base: div().flex_shrink_0().id(id),
            style: StyleRefinement::default(),
            icon: None,
            label: None,
            disabled: false,
            selected: false,
            variant: ButtonVariant::default(),
            rounded: ButtonRounded::Medium,
            border_corners: Corners::all(true),
            border_edges: Edges::all(true),
            size: Size::Medium,
            tooltip: None,
            on_click: None,
            on_hover: None,
            loading: false,
            compact: false,
            outline: false,
            children: Vec::new(),
            dropdown_menu_fn: None,
        }
    }

    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }
    pub fn rounded(mut self, r: impl Into<ButtonRounded>) -> Self {
        self.rounded = r.into();
        self
    }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
    pub fn dropdown_menu<F>(mut self, f: F) -> Self
    where
        F: Fn(super::widgets::DropdownMenu, &mut Window, &mut App) -> super::widgets::DropdownMenu
            + 'static,
    {
        self.dropdown_menu_fn = Some(Rc::new(f));
        self
    }

    fn clickable(&self) -> bool {
        !(self.disabled || self.loading) && self.on_click.is_some()
    }
    fn hoverable(&self) -> bool {
        !(self.disabled || self.loading) && self.on_hover.is_some()
    }
}

impl super::styled::Disableable for Button {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl super::styled::Selectable for Button {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl ButtonVariants for Button {
    fn with_variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}
impl InteractiveElement for Button {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let style = self.variant;
        let clickable = self.clickable();
        let is_disabled = self.disabled;
        let hoverable = self.hoverable();
        let normal_style = style.normal(self.outline, cx);
        let icon_size = match self.size {
            Size::Size(v) => Size::Size(v * 0.75),
            _ => self.size,
        };

        let focus_handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus_handle.is_focused(window);

        self.base
            .when(!self.disabled, |this| this.track_focus(&focus_handle))
            .cursor_default()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .cursor_default()
            .when(self.variant.is_link(), |this| this.cursor_pointer())
            .when(!style.no_padding(), |this| {
                if self.label.is_none() && self.children.is_empty() {
                    match self.size {
                        Size::Size(px) => this.size(px),
                        Size::XSmall => this.size(px(20.0)),
                        Size::Small => this.size(px(28.0)),
                        Size::Medium => this.size(px(32.0)),
                        Size::Large => this.size(px(40.0)),
                    }
                } else {
                    match self.size {
                        Size::Size(px) => this.h(px).px(px * 0.5),
                        Size::XSmall => this.h_5().px_2(),
                        Size::Small => this.h_6().px_2(),
                        _ if self.compact => this.h_8().px_2(),
                        Size::Medium => this.h_8().px_3(),
                        Size::Large => this.h_10().px_4(),
                    }
                }
            })
            .rounded(cx.theme().radius())
            .border_1()
            .text_color(normal_style.fg)
            .when(self.selected, |this| {
                let s = style.selected(self.outline, cx);
                this.bg(s.bg).border_color(s.border).text_color(s.fg)
            })
            .when(!self.disabled && !self.selected, |this| {
                this.border_color(normal_style.border)
                    .bg(normal_style.bg)
                    .when(normal_style.underline, |this| this.text_decoration_1())
                    .hover(|this| {
                        let h = style.hovered(self.outline, cx);
                        this.bg(h.bg).border_color(h.border).text_color(h.fg)
                    })
                    .active(|this| {
                        let a = style.active(self.outline, cx);
                        this.bg(a.bg).border_color(a.border).text_color(a.fg)
                    })
            })
            .when(self.disabled, |this| {
                let d = style.disabled(self.outline, cx);
                this.bg(d.bg)
                    .text_color(d.fg)
                    .border_color(d.border)
                    .shadow_none()
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if is_disabled {
                    cx.stop_propagation();
                    return;
                }
                window.prevent_default();
            })
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, cx| {
                    if !clickable {
                        cx.stop_propagation();
                        return;
                    }
                    (on_click)(event, window, cx);
                })
            })
            .when_some(self.on_hover.filter(|_| hoverable), |this, on_hover| {
                this.on_hover(move |hovered, window, cx| {
                    (on_hover)(hovered, window, cx);
                })
            })
            .child(
                super::styled::h_flex()
                    .id("label")
                    .items_center()
                    .justify_center()
                    .map(|this| match self.size {
                        Size::XSmall => this.text_xs(),
                        Size::Small => this.text_sm(),
                        _ => this.text_size(px(14.0)),
                    })
                    .when(!self.loading, |this| {
                        this.when_some(self.icon, |this, icon| {
                            this.child(icon.with_size(icon_size))
                        })
                    })
                    .when_some(self.label, |this, label| {
                        this.child(div().flex_none().line_height(relative(1.)).child(label))
                    })
                    .children(self.children),
            )
            .when_some(self.tooltip.clone(), |this, _tooltip| this)
            .when(is_focused, |this| {
                focus_ring(this, is_focused, px(2.0), window, cx)
            })
    }
}
