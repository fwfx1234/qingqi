use gpui::*;

use crate::token::tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement { Top, Bottom, Left, Right }

pub struct Sheet {
    pub placement: Placement,
    pub size: Pixels,
    pub title: Option<SharedString>,
    pub content: Option<AnyElement>,
    pub resizable: bool,
    pub overlay: bool,
}

impl Sheet {
    pub fn new(placement: Placement) -> Self {
        Self { placement, size: px(400.0), title: None, content: None, resizable: true, overlay: true }
    }
    pub fn size(mut self, s: Pixels) -> Self { self.size = s; self }
    pub fn title(mut self, t: impl Into<SharedString>) -> Self { self.title = Some(t.into()); self }
    pub fn content(mut self, c: impl IntoElement) -> Self { self.content = Some(c.into_any_element()); self }
    pub fn resizable(mut self, r: bool) -> Self { self.resizable = r; self }
    pub fn overlay(mut self, o: bool) -> Self { self.overlay = o; self }
}

pub struct ActiveSheet {
    pub sheet: Sheet,
    pub focus_handle: FocusHandle,
}

impl RenderOnce for Sheet {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);

        let (pos_style, size_style) = match self.placement {
            Placement::Top => (
                div().top_0().left_0().right_0().h(self.size),
                div().w_full().h(self.size),
            ),
            Placement::Bottom => (
                div().bottom_0().left_0().right_0().h(self.size),
                div().w_full().h(self.size),
            ),
            Placement::Left => (
                div().top_0().left_0().bottom_0().w(self.size),
                div().h_full().w(self.size),
            ),
            Placement::Right => (
                div().top_0().right_0().bottom_0().w(self.size),
                div().h_full().w(self.size),
            ),
        };

        let overlay = if self.overlay {
            div()
                .absolute()
                .top_0().left_0()
                .size_full()
                .bg(token.overlay)
        } else {
            div()
        };

        let mut sheet = size_style
            .bg(token.surface)
            .border_1()
            .border_color(token.border)
            .shadow_lg()
            .p_4()
            .flex()
            .flex_col()
            .gap_3();

        if let Some(title) = &self.title {
            sheet = sheet.child(div()
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(token.foreground)
                .child(title.clone()));
        }

        if let Some(content) = self.content {
            sheet = sheet.child(content);
        }

        div()
            .absolute()
            .top_0().left_0()
            .size_full()
            .child(overlay)
            .child(pos_style.child(sheet))
    }
}
