use gpui::*;

use super::TextInputState;
use super::TextInputElement;

pub struct PasswordInput {
    pub state: Entity<TextInputState>,
    pub mask_toggle: bool,
    pub element: TextInputElement,
}

impl PasswordInput {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        let mut element = TextInputElement::new(state);
        element.masked = true;
        Self { state: state.clone(), mask_toggle: false, element }
    }

    pub fn mask_toggle(mut self, t: bool) -> Self { self.mask_toggle = t; self }
}

impl RenderOnce for PasswordInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut element = self.element;
        element.masked = true;
        if self.mask_toggle {
            let state = self.state.clone();
            element = element.suffix(
                div().id("toggle-mask").child("👁").cursor_pointer()
                    .on_click(move |_, window, cx| {
                        state.update(cx, |s, cx| { s.masked = !s.masked; cx.notify(); });
                    }),
            );
        }
        element.render(window, cx)
    }
}
