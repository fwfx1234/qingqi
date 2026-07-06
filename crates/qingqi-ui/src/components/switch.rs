//! Switch component — local replacement for qingqi-ui::switch.

use gpui::{
    div, prelude::FluentBuilder as _, px, App, ElementId, InteractiveElement, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, Window,
};
use std::rc::Rc;

use super::styled::{Size, Sizable, Disableable, StyledExt};
use super::theme::ActiveTheme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side2 { Left, Right }

impl Side2 {
    pub fn is_left(&self) -> bool { matches!(self, Side2::Left) }
}

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    style: StyleRefinement,
    checked: bool,
    disabled: bool,
    label: Option<SharedString>,
    label_side: Side2,
    on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
    size: Size,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(), style: StyleRefinement::default(), checked: false, disabled: false,
            label: None, label_side: Side2::Right, on_click: None, size: Size::Medium,
        }
    }
    pub fn checked(mut self, checked: bool) -> Self { self.checked = checked; self }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self { self.label = Some(label.into()); self }
    pub fn on_click<F>(mut self, handler: F) -> Self
    where F: Fn(&bool, &mut Window, &mut App) + 'static {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for Switch {
    fn style(&mut self) -> &mut StyleRefinement { &mut self.style }
}
impl Sizable for Switch {
    fn with_size(mut self, size: impl Into<Size>) -> Self { self.size = size.into(); self }
}
impl Disableable for Switch {
    fn disabled(mut self, disabled: bool) -> Self { self.disabled = disabled; self }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let (bg, toggle_bg) = if checked {
            (cx.theme().primary(), cx.theme().switch_thumb())
        } else {
            (cx.theme().switch(), cx.theme().switch_thumb())
        };
        let (bg_width, bg_height) = match self.size {
            Size::XSmall | Size::Small => (px(28.), px(16.)),
            _ => (px(36.), px(20.)),
        };
        let bar_width = match self.size {
            Size::XSmall | Size::Small => px(12.),
            _ => px(16.),
        };
        let inset = px(2.);
        let x = if checked { bg_width - bar_width - inset * 2.0 } else { px(0.) };
        div().refine_style(&self.style).child(
            div()
                .flex().flex_row().gap_2().items_center()
                .when(self.label_side.is_left(), |this| this.flex_row_reverse())
                .child(
                    div()
                        .w(bg_width).h(bg_height).rounded(bg_height)
                        .flex().items_center()
                        .border(inset).border_color(cx.theme().transparent())
                        .bg(if self.disabled { bg.alpha(0.5) } else { bg })
                        .child(
                            div().rounded(bg_height)
                                .bg(if self.disabled { toggle_bg.alpha(0.35) } else { toggle_bg })
                                .w(bar_width).h(bar_width).ml(x),
                        ),
                )
                .when_some(self.label.clone(), |this, _label| this),
        )
    }
}
