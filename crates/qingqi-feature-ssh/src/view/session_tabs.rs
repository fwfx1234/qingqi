//! Session Tab 栏

use gpui::*;
use qingqi_ui::ui;

use super::SessionTabItem;

pub fn render_session_tabs(sessions: &[SessionTabItem]) -> impl IntoElement {
    div()
        .h(px(44.0)).flex().items_center().px_2()
        .bg(ui::bg_surface()).border_b_1().border_color(ui::border_light())
        .children(sessions.iter().map(|s| {
            div()
                .px_3().py_1().mr_1().rounded_t_md().cursor_pointer()
                .bg(if s.is_selected { hsla(0.55, 0.05, 0.35, 0.5) } else { hsla(0.0, 0.0, 0.0, 0.0) })
                .border_b_2()
                .border_color(if s.is_selected { s.status_color } else { hsla(0.0, 0.0, 0.0, 0.0) })
                .child(div().flex().items_center().gap(px(6.0))
                    .child(div().size(px(8.0)).rounded_full().bg(s.status_color))
                    .child(div().text_size(px(12.0)).child(s.title.clone()))
                    .child(div().ml_1().size(px(14.0)).flex().items_center().justify_center()
                        .rounded_full().text_size(px(10.0)).cursor_pointer()
                        .hover(|s| s.bg(hsla(0.0, 0.8, 0.5, 0.2)))
                        .child("✕")))
        }))
        .child(div().ml_2().size(px(24.0)).flex().items_center().justify_center()
            .rounded_md().cursor_pointer().hover(|s| s.bg(ui::bg_hover())).child("+"))
}
