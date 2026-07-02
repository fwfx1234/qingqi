use gpui::*;
use gpui::prelude::FluentBuilder;

use super::TextInputState;

pub struct TextInputElement {
    pub state: Entity<TextInputState>,
    pub placeholder: SharedString,
    pub disabled: bool,
    pub masked: bool,
    pub appearance: bool,
    pub bordered: bool,
    pub cleanable: bool,
    pub prefix: Option<AnyElement>,
    pub suffix: Option<AnyElement>,
    pub height: Pixels,
    pub font_size: Pixels,
    pub font_family: Option<String>,
    pub text_color: Option<Hsla>,
    pub placeholder_color: Option<Hsla>,
}

impl TextInputElement {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        Self {
            state: state.clone(),
            placeholder: SharedString::new_static(""),
            disabled: false,
            masked: false,
            appearance: true,
            bordered: true,
            cleanable: false,
            prefix: None,
            suffix: None,
            height: px(38.0),
            font_size: px(13.0),
            font_family: None,
            text_color: None,
            placeholder_color: None,
        }
    }

    pub fn placeholder(mut self, p: impl Into<SharedString>) -> Self { self.placeholder = p.into(); self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    pub fn masked(mut self, m: bool) -> Self { self.masked = m; self }
    pub fn appearance(mut self, a: bool) -> Self { self.appearance = a; self }
    pub fn bordered(mut self, b: bool) -> Self { self.bordered = b; self }
    pub fn cleanable(mut self, c: bool) -> Self { self.cleanable = c; self }
    pub fn prefix(mut self, p: impl IntoElement) -> Self { self.prefix = Some(p.into_any_element()); self }
    pub fn suffix(mut self, s: impl IntoElement) -> Self { self.suffix = Some(s.into_any_element()); self }
    pub fn h(mut self, h: Pixels) -> Self { self.height = h; self }
    pub fn text_size(mut self, s: Pixels) -> Self { self.font_size = s; self }
    pub fn font_family(mut self, f: impl Into<String>) -> Self { self.font_family = Some(f.into()); self }
}

impl RenderOnce for TextInputElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = crate::token::tokens(cx);
        let state = self.state.read(cx);
        let value = state.value.clone();
        let has_value = !value.is_empty();
        let focused = state.focus_handle().is_focused(window);

        let bg = if self.appearance { token.surface } else { gpui::hsla(0.0, 0.0, 0.0, 0.0) };
        let border_color = if focused { token.border_focus }
            else if self.bordered { token.border }
            else { gpui::hsla(0.0, 0.0, 0.0, 0.0) };
        let text_color = self.text_color.unwrap_or(token.foreground);
        let display_value = if self.masked { "•".repeat(value.len()) } else { value.clone() };
        let show_text = if has_value { display_value } else { self.placeholder.to_string() };
        let text_color = if has_value { text_color } else { self.placeholder_color.unwrap_or(token.foreground_placeholder) };

        let mut input = div()
            .id(("text-input", self.state.entity_id()))
            .h(self.height)
            .px_3()
            .flex()
            .items_center()
            .gap_1()
            .rounded(px(8.0))
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .text_size(self.font_size)
            .text_color(text_color)
            .when_some(self.font_family.clone(), |this, f| this.font_family(f));

        if !self.disabled {
            input = input.hover(|s| s.bg(token.surface_hover));
        } else {
            input = input.opacity(0.5);
        }

        if let Some(prefix) = self.prefix {
            input = input.child(prefix);
        }

        input = input.child(show_text);

        if let Some(suffix) = self.suffix {
            input = input.child(suffix);
        }

        input
    }
}
