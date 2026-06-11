//! 外部编辑完成后，询问是否回传远程

use gpui::prelude::*;
use gpui::*;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::glass;

const ACCENT: PluginAccent = PluginAccent::Cyan;

pub fn render_file_edit_confirm_overlay(
    handle: Entity<super::SshView>,
    file_name: &str,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
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
                .rounded(theme::radius_md())
                .border_1()
                .border_color(glass::border(dark))
                .bg(theme::semantic().bg_elevated)
                .shadow_lg()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(theme::font_size_body())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::text_primary())
                        .child("上传回服务器？"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::text_secondary())
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
                            button("不上传", ButtonVariant::Secondary, Some(ACCENT), dark)
                                .id("file-edit-confirm-cancel")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.cancel_external_edit(cx));
                                })
                        })
                        .child({
                            let h = handle;
                            button("上传", ButtonVariant::Primary, Some(ACCENT), dark)
                                .id("file-edit-confirm-upload")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.confirm_upload_external_edit(cx));
                                })
                        }),
                ),
        )
}
