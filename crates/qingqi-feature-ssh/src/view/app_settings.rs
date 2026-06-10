//! 插件设置弹窗 — 字体等全局配置（非 Profile 编辑）

use gpui::prelude::*;
use gpui::*;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::components::overlay_host;
use qingqi_ui::ui::glass;

const ACCENT: PluginAccent = PluginAccent::Cyan;

pub struct AppSettingsInputs {
    pub terminal_font_size: Entity<TextInput>,
}

pub fn render_app_settings(
    handle: Entity<super::SshView>,
    inputs: &AppSettingsInputs,
    terminal_font_size: f32,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
    let h = handle.clone();
    let dialog = handle.clone();
    let font_input = inputs.terminal_font_size.clone();

    overlay_host(
        dark,
        "app-settings-backdrop",
        move |_, _, cx| {
            h.update(cx, |v, cx| v.close_app_settings(cx));
        },
        div()
            .w(px(360.0))
            .rounded(theme::radius_sheet())
            .bg(theme::semantic().bg_elevated)
            .border_1()
            .border_color(theme::semantic().border_default)
            .shadow(glass::shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(render_header(&dialog, dark))
            .child(
                div()
                    .px(theme::space_5())
                    .py(theme::space_4())
                    .flex()
                    .flex_col()
                    .gap(theme::space_4())
                    .child(
                        div()
                            .text_size(theme::font_size_caption())
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ui::text_tertiary())
                            .child("显示"),
                    )
                    .child(
                        div()
                            .rounded(theme::radius_lg())
                            .border_1()
                            .border_color(glass::border(dark))
                            .bg(theme::semantic().bg_surface)
                            .px(theme::space_3())
                            .py(theme::space_2())
                            .flex()
                            .items_center()
                            .gap(theme::space_3())
                            .child(
                                div()
                                    .w(px(88.0))
                                    .text_size(theme::font_size_body())
                                    .child("终端字号"),
                            )
                            .child(div().flex_1().child(font_input)),
                    )
                    .child(
                        div()
                            .text_size(theme::font_size_caption())
                            .text_color(ui::text_tertiary())
                            .child(format!("当前: {}px", terminal_font_size.round() as i32)),
                    ),
            )
            .child(render_footer(&dialog, dark)),
    )
}

fn render_header(handle: &Entity<super::SshView>, dark: bool) -> impl IntoElement {
    let h = handle.clone();
    div()
        .h(px(48.0))
        .flex()
        .items_center()
        .px(theme::space_5())
        .justify_between()
        .border_b_1()
        .border_color(glass::divider(dark))
        .child(
            div()
                .text_size(theme::font_size_heading())
                .font_weight(FontWeight::SEMIBOLD)
                .child("插件设置"),
        )
        .child(
            div()
                .id("app-settings-close")
                .size(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .cursor_pointer()
                .text_size(px(14.0))
                .text_color(ui::text_tertiary())
                .hover(|s| s.bg(glass::hover_bg(dark)))
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h.update(cx, |v, cx| v.close_app_settings(cx));
                })
                .child("×"),
        )
}

fn render_footer(handle: &Entity<super::SshView>, dark: bool) -> impl IntoElement {
    let h_save = handle.clone();
    let h_cancel = handle.clone();
    div()
        .h(px(52.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_end()
        .gap(theme::space_2())
        .px(theme::space_5())
        .border_t_1()
        .border_color(glass::divider(dark))
        .child(
            button("取消", ButtonVariant::Secondary, None, dark)
                .id("app-settings-cancel")
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h_cancel.update(cx, |v, cx| v.close_app_settings(cx));
                }),
        )
        .child(
            button("保存", ButtonVariant::Primary, Some(ACCENT), dark)
                .id("app-settings-save")
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h_save.update(cx, |v, cx| v.save_app_settings(cx));
                }),
        )
}
