//! 外部编辑完成后，询问是否回传远程

use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::Theme;
use qingqi_ui::ui;
use qingqi_ui::ui::glass;

pub fn render_file_edit_confirm_overlay(
    handle: Entity<super::SshView>,
    file_name: &str,
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
                .id("file-edit-confirm-backdrop")
                .size_full()
                .absolute()
                .bg(hsla(0.0, 0.0, 0.0, 0.24))
                .on_mouse_down(MouseButton::Left, {
                    let h = backdrop.clone();
                    move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                        h.update(cx, |v, cx| v.cancel_external_edit(cx));
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top_1_2()
                .left_1_2()
                .w(px(360.0))
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
                        .child("上传回服务器？"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::text_secondary(cx))
                        .child(format!(
                            "「{file_name}」已在系统编辑器中保存，是否将本地更改上传至远程？"
                        )),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child({
                            let h = handle.clone();
                            ui::secondary_btn("file-edit-confirm-cancel", "不上传")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.cancel_external_edit(cx));
                                })
                        })
                        .child({
                            let h = handle;
                            ui::primary_btn("file-edit-confirm-upload", "上传")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.confirm_upload_external_edit(cx));
                                })
                        }),
                ),
        )
}
