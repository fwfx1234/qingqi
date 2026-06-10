//! 左侧 Profile 边栏

use gpui::*;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::ui;

use super::ProfileItem;

/// 渲染左侧完整边栏列（交通灯 + Profile 列表 + 设置按钮）
pub fn render_sidebar(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
    _cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    div()
        .w(px(280.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(ui::bg_surface())
        .border_r_1()
        .border_color(ui::border_light())
        .child(render_top_bar())
        .child(render_profile_list(profiles, selected_id))
        .child(render_bottom_bar())
}

fn render_top_bar() -> impl IntoElement {
    div()
        .h(px(52.0))
        .flex()
        .items_center()
        .px_3()
        .border_b_1()
        .border_color(ui::border_light())
        .child(mac_traffic_lights())
        .child(
            div()
                .ml_2()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("远程管理"),
        )
        .child(div().flex_1())
        .child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()))
                .child("+"),
        )
}

pub fn mac_traffic_lights() -> impl IntoElement {
    div()
        .flex()
        .gap(px(8.0))
        .px(px(4.0))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0xED6A5E)))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0xF5BF4F)))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0x61C554)))
}

fn render_profile_list(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
) -> impl IntoElement {
    div()
        .flex_1()
        .overflow_y_scrollbar()
        .p_2()
        .children(profiles.iter().map(|p| {
            render_profile_card(p, selected_id == Some(p.id))
        }))
}

fn render_profile_card(
    profile: &ProfileItem,
    is_selected: bool,
) -> impl IntoElement {
    div()
        .p_2()
        .mb_1()
        .rounded_md()
        .cursor_pointer()
        .bg(if is_selected {
            hsla(0.55, 0.3, 0.5, 0.15)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .hover(|s| {
            if !is_selected {
                s.bg(ui::bg_hover())
            } else {
                s
            }
        })
        .border_l_3()
        .border_color(if profile.is_connected {
            hsla(0.4, 0.8, 0.5, 1.0)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .px(px(4.0))
                                .py(px(1.0))
                                .rounded_sm()
                                .bg(hsla(0.55, 0.5, 0.6, 0.2))
                                .text_color(hsla(0.55, 0.5, 0.5, 1.0))
                                .child(profile.protocol_badge.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .child(profile.name.clone()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child(profile.endpoint.clone()),
                ),
        )
}

fn render_bottom_bar() -> impl IntoElement {
    div()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .border_t_1()
        .border_color(ui::border_light())
        .child(
            div()
                .cursor_pointer()
                .text_size(px(12.0))
                .text_color(ui::text_secondary())
                .hover(|s| s.text_color(ui::text_primary()))
                .child("设置"),
        )
}
