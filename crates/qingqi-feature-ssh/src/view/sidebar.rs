//! 左侧 Profile 边栏 — macOS Source List 风格

use gpui::prelude::*;
use gpui::*;
use gpui_component::menu::ContextMenuExt;
use gpui_component::theme::Theme;

use super::context_menu;
use super::virtual_list;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::theme;
use qingqi_ui::ui;
use qingqi_ui::ui::glass;

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

    div()
        .w(px(248.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(glass::sidebar(cx))
        .border_r_1()
        .border_color(glass::border(cx))
        .child(render_top_bar(cx))
        .child(render_profile_list(profiles, list_scroll, cx))
        .child(render_bottom_bar(cx))
}

fn render_top_bar(cx: &mut Context<super::SshView>) -> impl IntoElement {
    let hover_bg = glass::hover_bg(cx);
    div()
        .h(px(36.0))
        .flex()
        .items_center()
        .pl(px(86.0))
        .pr(px(10.0))
        .border_b_1()
        .border_color(glass::divider(cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(theme::font_size_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_primary(cx))
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
                .text_color(ui::text_secondary(cx))
                .hover(move |s| s.bg(hover_bg).text_color(ui::accent_color(ACCENT)))
                .on_click(
                    cx.listener(|view, _: &ClickEvent, _w, cx| view.open_profile_editor(None, cx)),
                )
                .child("+"),
        )
}

fn selection_bg(cx: &App) -> Hsla {
    let accent = Theme::global(cx).blue;
    theme::rgba_with_alpha(
        accent.into(),
        if Theme::global(cx).is_dark() {
            0.22
        } else {
            0.14
        },
    )
}

fn profile_row(name: &str, endpoint: &str, badge: &str, is_selected: bool, cx: &App) -> Div {
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
            selection_bg(cx)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .child(
            div()
                .text_size(theme::font_size_body())
                .font_weight(FontWeight::MEDIUM)
                .text_color(if is_selected {
                    Theme::global(cx).blue
                } else {
                    ui::text_primary(cx)
                })
                .truncate()
                .child(format!("{badge} · {name}")),
        )
        .child(
            div()
                .text_size(theme::font_size_caption())
                .text_color(ui::text_tertiary(cx))
                .font_family(ui::font_mono())
                .truncate()
                .child(endpoint.to_string()),
        )
}

fn profile_list_item(
    profile: &ProfileItem,
    handle: Entity<super::SshView>,
    cx: &App,
) -> AnyElement {
    let pid = profile.id;
    let hover_bg = glass::hover_bg(cx);
    profile_row(
        &profile.name,
        &profile.endpoint,
        &profile.protocol_badge,
        profile.is_selected,
        cx,
    )
    .id(("ssh-profile", pid as u64))
    .cursor_pointer()
    .when(!profile.is_selected, move |row| {
        row.hover(move |s| s.bg(hover_bg))
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
    list_scroll: UniformListScrollHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let app: &App = cx;
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
                    .text_color(ui::text_tertiary(app))
                    .child("暂无连接"),
            )
            .child(
                div()
                    .text_size(theme::font_size_caption())
                    .text_color(ui::text_tertiary(app))
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
            move |range, _window, cx| {
                range
                    .map(|i| {
                        div()
                            .w_full()
                            .h(px(PROFILE_ROW_HEIGHT))
                            .child(profile_list_item(&items[i], handle.clone(), cx))
                            .into_any_element()
                    })
                    .collect()
            },
        ))
        .into_any_element()
}

fn render_bottom_bar(cx: &mut Context<super::SshView>) -> impl IntoElement {
    let hover_bg = glass::hover_bg(cx);
    let hover_text = ui::text_primary(cx);
    div()
        .h(px(36.0))
        .flex()
        .items_center()
        .justify_center()
        .border_t_1()
        .border_color(glass::divider(cx))
        .child(
            div()
                .id("btn-settings")
                .px(px(10.0))
                .py(px(4.0))
                .rounded(px(5.0))
                .cursor_pointer()
                .text_size(theme::font_size_caption())
                .text_color(ui::text_tertiary(cx))
                .hover(move |s| s.bg(hover_bg).text_color(hover_text))
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| view.open_app_settings(cx)))
                .child("设置"),
        )
}
