use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::token::tokens;

#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    label: Option<SharedString>,
    checked: bool,
    #[allow(dead_code)]
    disabled: bool,
    on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    style: gpui::StyleRefinement,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            checked: false,
            disabled: false,
            on_click: None,
            style: gpui::StyleRefinement::default(),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + 'static,
    {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for Radio {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Radio {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        let checked = self.checked;

        let radio = div()
            .id(self.id.clone())
            .w(px(16.0))
            .h(px(16.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(if checked { t.accent } else { t.border })
            .bg(t.surface)
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(if checked {
                t.accent
            } else {
                gpui::hsla(0.0, 0.0, 0.0, 0.0)
            }));

        let radio = if let Some(on_click) = self.on_click {
            radio.on_click(move |_, window, cx| {
                let new_state = !checked;
                on_click(&new_state, window, cx);
            })
        } else {
            radio
        };

        match self.label {
            Some(label) => div()
                .flex()
                .items_center()
                .gap_2()
                .child(radio)
                .child(div().text_color(t.foreground).child(label)),
            None => div().child(radio),
        }
    }
}
