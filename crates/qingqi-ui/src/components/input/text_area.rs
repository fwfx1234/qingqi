use gpui::*;

use super::TextInputState;
use super::TextInputElement;

pub struct TextArea {
    pub state: Entity<TextInputState>,
    pub min_rows: usize,
    pub max_rows: usize,
    pub soft_wrap: bool,
    pub element: TextInputElement,
}

impl TextArea {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        let mut element = TextInputElement::new(state);
        element.height = px(80.0);
        Self { state: state.clone(), min_rows: 2, max_rows: 10, soft_wrap: true, element }
    }

    pub fn min_rows(mut self, r: usize) -> Self { self.min_rows = r; self }
    pub fn max_rows(mut self, r: usize) -> Self { self.max_rows = r; self }
    pub fn soft_wrap(mut self, w: bool) -> Self { self.soft_wrap = w; self }
}

impl RenderOnce for TextArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.element.render(window, cx)
    }
}
