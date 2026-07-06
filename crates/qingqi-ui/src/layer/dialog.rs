use gpui::*;
use std::rc::Rc;

use crate::token::tokens;
use crate::components::button::{Button, ButtonVariant, ButtonVariants};

#[derive(Clone)]
pub struct DialogButton {
    pub label: SharedString,
    pub variant: ButtonVariant,
    pub on_click: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool>,
}

pub struct Dialog {
    pub title: Option<SharedString>,
    pub content: Option<AnyElement>,
    pub primary_button: Option<DialogButton>,
    pub secondary_button: Option<DialogButton>,
    pub width: Pixels,
    pub max_width: Option<Pixels>,
    pub overlay: bool,
    pub overlay_closable: bool,
    pub on_close: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

pub struct ActiveDialog {
    pub dialog: Dialog,
    pub focus_handle: FocusHandle,
}

impl Dialog {
    pub fn new() -> Self {
        Self {
            title: None, content: None, primary_button: None, secondary_button: None,
            width: px(400.0), max_width: None, overlay: true, overlay_closable: true,
            on_close: None,
        }
    }
    pub fn title(mut self, t: impl Into<SharedString>) -> Self { self.title = Some(t.into()); self }
    pub fn content(mut self, c: impl IntoElement) -> Self { self.content = Some(c.into_any_element()); self }
    pub fn width(mut self, w: Pixels) -> Self { self.width = w; self }
    pub fn max_width(mut self, w: Pixels) -> Self { self.max_width = Some(w); self }
    pub fn overlay(mut self, o: bool) -> Self { self.overlay = o; self }
    pub fn overlay_closable(mut self, c: bool) -> Self { self.overlay_closable = c; self }
    pub fn on_close(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self { self.on_close = Some(Rc::new(f)); self }
    pub fn primary_button(mut self, label: impl Into<SharedString>, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static) -> Self {
        self.primary_button = Some(DialogButton { label: label.into(), variant: ButtonVariant::Primary, on_click: Rc::new(on_click) }); self
    }
    pub fn secondary_button(mut self, label: impl Into<SharedString>, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static) -> Self {
        self.secondary_button = Some(DialogButton { label: label.into(), variant: ButtonVariant::Secondary, on_click: Rc::new(on_click) }); self
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui::prelude::*;
        let token = tokens(cx);

        let overlay = if self.overlay {
            let mut overlay = div().absolute().top_0().left_0().size_full().bg(token.overlay);
            if self.overlay_closable {
                if let Some(on_close) = &self.on_close {
                    let on_close = on_close.clone();
                    overlay.interactivity().on_click(move |e, w, cx| on_close(e, w, cx));
                }
            }
            overlay
        } else {
            div()
        };

        let mut dialog = div()
            .bg(token.surface).rounded(px(12.0)).border_1().border_color(token.border)
            .shadow_lg().p_4().flex().flex_col().gap_3().w(self.width);

        if let Some(max_w) = self.max_width { dialog = dialog.max_w(max_w); }
        if let Some(title) = &self.title {
            dialog = dialog.child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).text_color(token.foreground).child(title.clone()));
        }
        if let Some(content) = self.content { dialog = dialog.child(content); }

        if self.primary_button.is_some() || self.secondary_button.is_some() {
            let mut buttons = div().flex().gap_2().justify_end();
            if let Some(btn) = &self.secondary_button {
                let on_click = btn.on_click.clone();
                let btn_elem = RenderOnce::render(
                    Button::new("dialog-secondary").label(btn.label.clone())
                        .with_variant(ButtonVariant::Secondary)
                        .on_click(move |e, w, cx| { let _ = on_click(e, w, cx); }),
                    window, cx,
                );
                buttons = buttons.child(btn_elem);
            }
            if let Some(btn) = &self.primary_button {
                let on_click = btn.on_click.clone();
                let btn_elem = RenderOnce::render(
                    Button::new("dialog-primary").label(btn.label.clone())
                        .with_variant(ButtonVariant::Primary)
                        .on_click(move |e, w, cx| { let _ = on_click(e, w, cx); }),
                    window, cx,
                );
                buttons = buttons.child(btn_elem);
            }
            dialog = dialog.child(buttons);
        }

        div().absolute().top_0().left_0().size_full().flex().items_center().justify_center()
            .child(overlay).child(dialog)
    }
}
