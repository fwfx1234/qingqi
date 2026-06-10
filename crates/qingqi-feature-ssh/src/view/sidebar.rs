//! 左侧 Profile 边栏 — macOS Source List 风格

use gpui::prelude::*;
use gpui::*;
use gpui_component::scroll::ScrollableElement;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::glass;

use super::ProfileItem;

const ACCENT: PluginAccent = PluginAccent::Cyan;

pub fn render_sidebar(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
    context_menu_profile_id: Option<i64>,
    context_menu_position: Option<Point<Pixels>>,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let _ = (selected_id, context_menu_profile_id, context_menu_position);
    let dark = theme_mode::is_dark();

    div()
        .w(px(248.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(glass::sidebar(dark))
        .border_r_1()
        .border_color(glass::border(dark))
        .child(render_top_bar(dark, cx))
        .child(render_profile_list(profiles, dark, cx))
        .child(render_bottom_bar(dark, cx))
}

fn render_top_bar(dark: bool, cx: &mut Context<super::SshView>) -> impl IntoElement {
    div()
        .h(px(40.0))
        .flex()
        .items_end()
        .pb(px(6.0))
        .pl(px(80.0))
        .pr(px(10.0))
        .child(
            div()
                .flex_1()
                .text_size(theme::font_size_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_primary())
                .child("远程管理"),
        )
        .child(
            div()
                .id("btn-new-profile")
                .size(px(22.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .cursor_pointer()
                .text_size(px(15.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ui::text_secondary())
                .hover(|s| {
                    s.bg(glass::hover_bg(dark))
                        .text_color(ui::accent_color(ACCENT))
                })
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| {
                    view.open_profile_editor(None, cx)
                }))
                .child("+"),
        )
}

fn selection_bg(dark: bool) -> Hsla {
    let accent = theme::blue_500();
    theme::rgba_with_alpha(accent, if dark { 0.22 } else { 0.14 })
}

fn profile_row(
    name: &str,
    endpoint: &str,
    badge: &str,
    is_connected: bool,
    is_selected: bool,
    dark: bool,
) -> Div {
    let accent = ui::accent_color(ACCENT);
    let accent_soft = if dark {
        theme::accent_soft_dark(ACCENT)
    } else {
        theme::accent_soft(ACCENT)
    };

    div()
        .w_full()
        .min_w_0()
        .mx(px(8.0))
        .mb(px(1.0))
        .px(px(8.0))
        .py(px(5.0))
        .rounded(px(6.0))
        .bg(if is_selected {
            selection_bg(dark)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(30.0))
                .flex_shrink_0()
                .rounded(px(7.0))
                .bg(accent_soft)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .text_color(accent)
                .child(badge.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .min_w_0()
                        .child(
                            div()
                                .text_size(theme::font_size_body())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if is_selected {
                                    theme::blue_600()
                                } else {
                                    ui::text_primary()
                                })
                                .truncate()
                                .child(name.to_string()),
                        )
                        .when(is_connected, |el| {
                            el.child(
                                div()
                                    .size(px(6.0))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(ui::success()),
                            )
                        }),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(ui::text_tertiary())
                        .font_family(ui::font_mono())
                        .truncate()
                        .child(endpoint.to_string()),
                ),
        )
}

fn render_profile_list(
    profiles: &[ProfileItem],
    dark: bool,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let count = profiles.len();
    if count > 20 {
        let items: Vec<_> = profiles
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    p.endpoint.clone(),
                    p.protocol_badge.clone(),
                    p.is_connected,
                    p.is_selected,
                )
            })
            .collect();

        uniform_list("ssh-profile-list", count, move |range, _w, _cx| {
            items[range]
                .iter()
                .map(|(name, endpoint, badge, is_connected, is_selected)| {
                    profile_row(
                        name,
                        endpoint,
                        badge,
                        *is_connected,
                        *is_selected,
                        dark,
                    )
                    .into_any_element()
                })
                .collect::<Vec<_>>()
        })
        .flex_1()
        .pt(px(4.0))
        .into_any_element()
    } else if count == 0 {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .px(px(20.0))
            .child(
                div()
                    .text_size(theme::font_size_body())
                    .text_color(ui::text_tertiary())
                    .child("暂无连接"),
            )
            .child(
                div()
                    .text_size(theme::font_size_caption())
                    .text_color(ui::text_tertiary())
                    .child("双击连接 · 点击 + 添加"),
            )
            .into_any_element()
    } else {
        div()
            .flex_1()
            .overflow_y_scrollbar()
            .pt(px(4.0))
            .children(profiles.iter().map(|p| {
                let pid = p.id;
                let pid_right = p.id;

                profile_row(
                    &p.name,
                    &p.endpoint,
                    &p.protocol_badge,
                    p.is_connected,
                    p.is_selected,
                    dark,
                )
                .id(("ssh-profile", pid as u64))
                .cursor_pointer()
                .hover(|s| {
                    if !p.is_selected {
                        s.bg(glass::hover_bg(dark))
                    } else {
                        s
                    }
                })
                .on_click(cx.listener(move |view, event: &ClickEvent, _w, cx| {
                    if event.click_count() >= 2 {
                        view.connect_profile(pid, cx);
                    } else {
                        view.select_profile(pid, cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |view, event: &MouseDownEvent, _w, cx| {
                        view.open_context_menu(pid_right, event.position, cx);
                    }),
                )
            }))
            .into_any_element()
    }
}

fn render_bottom_bar(dark: bool, cx: &mut Context<super::SshView>) -> impl IntoElement {
    div()
        .h(px(36.0))
        .flex()
        .items_center()
        .justify_center()
        .border_t_1()
        .border_color(glass::divider(dark))
        .child(
            div()
                .id("btn-settings")
                .px(px(10.0))
                .py(px(4.0))
                .rounded(px(5.0))
                .cursor_pointer()
                .text_size(theme::font_size_caption())
                .text_color(ui::text_tertiary())
                .hover(|s| {
                    s.bg(glass::hover_bg(dark))
                        .text_color(ui::text_primary())
                })
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| {
                    view.open_app_settings(cx)
                }))
                .child("设置"),
        )
}
