//! 左侧 Profile 边栏 — 毛玻璃底 + 悬浮卡片

use gpui::*;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::ui;
use qingqi_ui::ui::glass;

use super::ProfileItem;

pub fn render_sidebar(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    div()
        .w(px(272.0)).h_full().flex().flex_col()
        .bg(glass::panel(true))
        .border_r_1().border_color(glass::border(true))
        .child(render_top_bar(cx))
        .child(render_profile_list(profiles, selected_id, cx))
        .child(render_bottom_bar(cx))
}

fn render_top_bar(cx: &mut Context<super::SshView>) -> impl IntoElement {
    div()
        .h(px(48.0)).flex().items_center().px_3()
        .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("远程管理"))
        .child(div().flex_1())
        .child(
            div()
                .id("btn-new-profile")
                .size(px(28.0)).flex().items_center().justify_center()
                .rounded(px(6.0)).cursor_pointer()
                .bg(glass::hover_bg(true))
                .hover(|s| s.bg(hsla(1.0, 1.0, 1.0, 0.12)))
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| view.toggle_settings(cx)))
                .child("+"),
        )
}

fn render_profile_list(
    profiles: &[ProfileItem],
    _selected_id: Option<i64>,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let count = profiles.len();
    if count > 20 {
        let items: Vec<_> = profiles.iter().map(|p| {
            (p.name.clone(), p.endpoint.clone(), p.protocol_badge.clone())
        }).collect();
        uniform_list("ssh-profile-list", count, move |range, _w, _cx| {
            items[range].iter().map(|(name, endpoint, badge)| {
                div()
                    .mx(px(8.0)).mb(px(6.0))
                    .p_3().rounded(px(8.0))
                    .bg(hsla(1.0, 1.0, 1.0, 0.18))
                    .border_1().border_color(ui::border_light())
                    .shadow_md()
                    .child(div().flex().flex_col().gap(px(4.0))
                        .child(div().flex().items_center().gap(px(6.0))
                            .child(div().text_size(px(10.0)).px(px(6.0)).py(px(2.0)).rounded(px(4.0))
                                .bg(hsla(0.55, 0.5, 0.6, 0.15)).text_color(hsla(0.55, 0.5, 0.5, 1.0))
                                .child(badge.clone()))
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(name.clone())))
                        .child(div().text_size(px(11.0)).text_color(ui::text_secondary()).font_family("Menlo").child(endpoint.clone())))
                    .into_any_element()
            }).collect::<Vec<_>>()
        }).flex_1().py(px(4.0)).into_any_element()
    } else {
        div()
            .flex_1().overflow_y_scrollbar().py(px(4.0))
            .children(profiles.iter().map(|p| {
                let pid = p.id;
                div()
                    .id(("ssh-profile", pid as u64))
                    .mx(px(8.0)).mb(px(6.0)).p_3().rounded(px(8.0)).cursor_pointer()
                    .bg(hsla(1.0, 1.0, 1.0, 0.18))
                    .border_1().border_color(ui::border_light())
                    .shadow_md()
                    .hover(|s| s.shadow_lg())
                    .on_click(cx.listener(move |view, _: &ClickEvent, _w, cx| view.connect_profile(pid, cx)))
                    .child(div().flex().flex_col().gap(px(4.0))
                        .child(div().flex().items_center().gap(px(6.0))
                            .child(div().text_size(px(10.0)).px(px(6.0)).py(px(2.0)).rounded(px(4.0))
                                .bg(hsla(0.55, 0.5, 0.6, 0.15)).text_color(hsla(0.55, 0.5, 0.5, 1.0))
                                .child(p.protocol_badge.clone()))
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(p.name.clone())))
                        .child(div().text_size(px(11.0)).text_color(ui::text_secondary()).font_family("Menlo").child(p.endpoint.clone())))
            }))
            .into_any_element()
    }
}

fn render_bottom_bar(cx: &mut Context<super::SshView>) -> impl IntoElement {
    div()
        .h(px(44.0)).flex().items_center().justify_center()
        .border_t_1().border_color(ui::border_light())
        .child(
            div()
                .id("btn-settings")
                .cursor_pointer().text_size(px(12.0)).text_color(ui::text_secondary())
                .hover(|s| s.text_color(ui::text_primary()))
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| view.toggle_settings(cx)))
                .child("设置"),
        )
}
