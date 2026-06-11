//! 左侧 Profile 边栏 — macOS Source List 风格

use gpui::prelude::*;
use gpui::*;
use gpui_component::menu::ContextMenuExt;

use super::context_menu;
use super::virtual_list;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::ui::glass;
use qingqi_ui::{theme, theme_mode, ui};

use super::ProfileItem;

const ACCENT: PluginAccent = PluginAccent::Cyan;
const PROFILE_ROW_HEIGHT: f32 = 48.0;

pub fn render_sidebar(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
    list_scroll: UniformListScrollHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let _ = selected_id;
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
        .child(render_profile_list(profiles, dark, list_scroll, cx))
        .child(render_bottom_bar(dark, cx))
}

fn render_top_bar(dark: bool, cx: &mut Context<super::SshView>) -> impl IntoElement {
    div()
        .h(px(36.0))
        .flex()
        .items_center()
        .pl(px(86.0))
        .pr(px(10.0))
        .border_b_1()
        .border_color(glass::divider(dark))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(theme::font_size_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_primary())
                .truncate()
                .child("远程管理"),
        )
        .child(
            div()
                .id("btn-new-profile")
                .size(px(22.0))
                .flex_shrink_0()
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
                .on_click(
                    cx.listener(|view, _: &ClickEvent, _w, cx| view.open_profile_editor(None, cx)),
                )
                .child("+"),
        )
}

fn selection_bg(dark: bool) -> Hsla {
    let accent = theme::blue_500();
    theme::rgba_with_alpha(accent, if dark { 0.22 } else { 0.14 })
}

fn profile_row(name: &str, endpoint: &str, badge: &str, is_selected: bool, dark: bool) -> Div {
    div()
        .size_full()
        .min_w_0()
        .mx(px(8.0))
        .px(px(10.0))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.0))
        .rounded(px(6.0))
        .bg(if is_selected {
            selection_bg(dark)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
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
                .child(format!("{badge} · {name}")),
        )
        .child(
            div()
                .text_size(theme::font_size_caption())
                .text_color(ui::text_tertiary())
                .font_family(ui::font_mono())
                .truncate()
                .child(endpoint.to_string()),
        )
}

fn profile_list_item(
    profile: &ProfileItem,
    dark: bool,
    handle: Entity<super::SshView>,
) -> AnyElement {
    let pid = profile.id;
    profile_row(
        &profile.name,
        &profile.endpoint,
        &profile.protocol_badge,
        profile.is_selected,
        dark,
    )
    .id(("ssh-profile", pid as u64))
    .cursor_pointer()
    .when(!profile.is_selected, |row| {
        row.hover(|s| s.bg(glass::hover_bg(dark)))
    })
    .on_click({
        let h = handle.clone();
        move |event: &ClickEvent, _: &mut Window, cx: &mut App| {
            h.update(cx, |view, cx| {
                if event.click_count() >= 2 {
                    view.connect_profile(pid, cx);
                } else {
                    view.select_profile(pid, cx);
                }
            });
        }
    })
    .context_menu({
        let h = handle;
        move |menu, _window, _cx| context_menu::profile_menu(menu, pid, h.clone())
    })
    .into_any_element()
}

fn render_profile_list(
    profiles: &[ProfileItem],
    dark: bool,
    list_scroll: UniformListScrollHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let count = profiles.len();
    if count == 0 {
        return div()
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
            .into_any_element();
    }

    let items: Vec<ProfileItem> = profiles.to_vec();
    let handle = cx.entity().clone();

    div()
        .flex_1()
        .min_h(px(0.0))
        .pt(px(4.0))
        .child(virtual_list::vertical(
            "ssh-profile-list",
            count,
            list_scroll,
            move |range, _window, _cx| {
                range
                    .map(|i| {
                        div()
                            .w_full()
                            .h(px(PROFILE_ROW_HEIGHT))
                            .child(profile_list_item(&items[i], dark, handle.clone()))
                            .into_any_element()
                    })
                    .collect()
            },
        ))
        .into_any_element()
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
                .hover(|s| s.bg(glass::hover_bg(dark)).text_color(ui::text_primary()))
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| view.open_app_settings(cx)))
                .child("设置"),
        )
}
