//! 插件设置 — 独立子窗口

use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::Theme;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::ui;
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::glass;

pub struct AppSettingsInputs {
    pub terminal_font_size: Entity<TextInput>,
}

pub fn render_app_settings_panel(
    handle: Entity<super::SshView>,
    inputs: &AppSettingsInputs,
    terminal_font_size: f32,
    cx: &App,
) -> impl IntoElement {
    let dialog = handle.clone();
    let font_input = inputs.terminal_font_size.clone();

    div()
        .size_full()
        .bg(Theme::global(cx).popover)
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .flex_1()
                .px(px(20.0))
                .py(px(16.0))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ui::text_tertiary(cx))
                        .child("显示"),
                )
                .child(
                    div()
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(glass::border(cx))
                        .bg(Theme::global(cx).list)
                        .px(px(12.0))
                        .py(px(8.0))
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(div().w(px(88.0)).text_size(px(13.0)).child("终端字号"))
                        .child(div().flex_1().child(font_input)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_tertiary(cx))
                        .child(format!("当前: {}px", terminal_font_size.round() as i32)),
                ),
        )
        .child(render_footer(&dialog, cx))
}

fn render_footer(handle: &Entity<super::SshView>, cx: &App) -> impl IntoElement {
    let h_save = handle.clone();
    let h_cancel = handle.clone();
    div()
        .h(px(52.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(8.0))
        .px(px(20.0))
        .border_t_1()
        .border_color(glass::divider(cx))
        .child(
            button("取消", ButtonVariant::Secondary, None, cx)
                .id("app-settings-cancel")
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h_cancel.update(cx, |v, cx| v.close_app_settings(cx));
                }),
        )
        .child(
            button("保存", ButtonVariant::Primary, None, cx)
                .id("app-settings-save")
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h_save.update(cx, |v, cx| v.save_app_settings(cx));
                }),
        )
}
