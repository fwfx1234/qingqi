use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, hsla, prelude::FluentBuilder, px,
};

use crate::token::tokens;

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    label: Option<SharedString>,
    checked: bool,
    disabled: bool,
    on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    style: gpui::StyleRefinement,
}

impl Checkbox {
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

impl Styled for Checkbox {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        let checked = self.checked;

        let checkbox = div()
            .id(self.id.clone())
            .w(px(16.0))
            .h(px(16.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(if checked { t.accent } else { t.border })
            .bg(if checked { t.accent } else { t.surface })
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.0))
            .text_color(if checked {
                gpui::white()
            } else {
                gpui::hsla(0.0, 0.0, 0.0, 0.0)
            })
            .child(if checked { "✓" } else { "" });

        let checkbox = if let Some(on_click) = self.on_click {
            let id = self.id.clone();
            checkbox.on_click(move |_, window, cx| {
                let new_state = !checked;
                on_click(&new_state, window, cx);
            })
        } else {
            checkbox
        };

        match self.label {
            Some(label) => div()
                .flex()
                .items_center()
                .gap_2()
                .child(checkbox)
                .child(div().text_color(t.foreground).child(label)),
            None => div().child(checkbox),
        }
    }
}
