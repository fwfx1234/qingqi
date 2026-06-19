//! 文件重命名弹窗

use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::Theme;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::ui;
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::glass;

pub fn render_file_rename_overlay(
    handle: Entity<super::SshView>,
    rename_input: Entity<TextInput>,
    cx: &App,
) -> impl IntoElement {
    let backdrop = handle.clone();

    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .occlude()
        .child(
            div()
                .id("file-rename-backdrop")
                .size_full()
                .absolute()
                .bg(hsla(0.0, 0.0, 0.0, 0.24))
                .on_mouse_down(MouseButton::Left, {
                    let h = backdrop.clone();
                    move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                        h.update(cx, |v, cx| v.close_file_rename(cx));
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top_1_2()
                .left_1_2()
                .w(px(320.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(glass::border(cx))
                .bg(Theme::global(cx).popover)
                .shadow_lg()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::text_primary(cx))
                        .child("重命名"),
                )
                .child(
                    div()
                        .id("file-rename-input")
                        .h(px(32.0))
                        .child(rename_input),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child({
                            let h = handle.clone();
                            button("取消", ButtonVariant::Secondary, None, cx)
                                .id("file-rename-cancel")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.close_file_rename(cx));
                                })
                        })
                        .child({
                            let h = handle;
                            button("确定", ButtonVariant::Primary, None, cx)
                                .id("file-rename-ok")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.confirm_file_rename(cx));
                                })
                        }),
                ),
        )
}
