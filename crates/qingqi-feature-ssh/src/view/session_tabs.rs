//! Session Tab 栏 — Chrome 风

use gpui::*;
use qingqi_ui::ui;

use super::SessionTabItem;

pub fn render_session_tabs(
    sessions: &[SessionTabItem],
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    div()
        .h(px(38.0))
        .flex()
        .items_end()
        .px(px(6.0))
        .gap(px(2.0))
        .bg(ui::bg_surface())
        .border_b_1()
        .border_color(ui::border_light())
        .children(sessions.iter().map(|s| {
            let sid = s.session_id;
            let sid_close = s.session_id;
            let bg_normal: Hsla = hsla(0.0, 0.0, 0.0, 0.0);
            let bg_selected: Hsla = hsla(0.0, 0.0, 1.0, 1.0);
            let border_none: Hsla = hsla(0.0, 0.0, 0.0, 0.0);
            let border_visible: Hsla = hsla(0.6, 0.05, 0.85, 1.0);

            div()
                .id(("ssh-tab", sid.0.as_u128() as u64))
                .px_3()
                .py(px(6.0))
                .rounded_t(px(8.0))
                .cursor_pointer()
                .bg(if s.is_selected {
                    bg_selected
                } else {
                    bg_normal
                })
                .border_1()
                .border_color(if s.is_selected {
                    border_visible
                } else {
                    border_none
                })
                .border_b_1()
                .border_color(if s.is_selected {
                    bg_selected
                } else {
                    border_none
                })
                .on_click(
                    cx.listener(move |view, _: &ClickEvent, _w, cx| view.select_session(sid, cx)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .size(px(7.0))
                                .rounded_full()
                                .bg(s.status_color)
                                .shadow(small_shadow()),
                        )
                        .child(div().text_size(px(12.0)).child(s.title.clone()))
                        .child(
                            div()
                                .id(("ssh-tab-close", sid_close.0.as_u128() as u64))
                                .ml_1()
                                .size(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .text_size(px(10.0))
                                .cursor_pointer()
                                .text_color(if s.is_selected {
                                    hsla(0.6, 0.05, 0.5, 1.0)
                                } else {
                                    hsla(0.0, 0.0, 0.0, 0.0)
                                })
                                .hover(|s| {
                                    s.bg(hsla(0.0, 0.8, 0.5, 0.15))
                                        .text_color(hsla(0.0, 0.8, 0.5, 1.0))
                                })
                                .on_click(cx.listener(move |view, _: &ClickEvent, _w, cx| {
                                    view.close_session(sid_close, cx)
                                }))
                                .child("✕"),
                        ),
                )
        }))
}

fn small_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0.0, 0.0, 0.0, 0.3),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(4.0),
        spread_radius: px(0.0),
    }]
}
