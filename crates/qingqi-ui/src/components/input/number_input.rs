use gpui::*;

use super::TextInputState;
use super::TextInputElement;

pub struct NumberInput {
    pub state: Entity<TextInputState>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
    pub element: TextInputElement,
}

impl NumberInput {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        Self { state: state.clone(), min: None, max: None, step: 1.0, element: TextInputElement::new(state) }
    }

    pub fn min(mut self, v: f64) -> Self { self.min = Some(v); self }
    pub fn max(mut self, v: f64) -> Self { self.max = Some(v); self }
    pub fn step(mut self, v: f64) -> Self { self.step = v; self }
}

impl RenderOnce for NumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.element.render(window, cx)
    }
}
