//! 上传覆盖确认弹窗

use gpui::prelude::*;
use gpui::*;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::glass;

const ACCENT: PluginAccent = PluginAccent::Cyan;

pub fn render_upload_overwrite_overlay(
    handle: Entity<super::SshView>,
    conflict_count: usize,
    sample_name: &str,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
    let backdrop = handle.clone();
    let detail = if conflict_count <= 1 {
        format!("远程已存在「{sample_name}」，是否覆盖？")
    } else {
        format!(
            "远程已有 {conflict_count} 个同名文件/文件夹（如「{sample_name}」），是否覆盖？"
        )
    };

    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .occlude()
        .child(
            div()
                .id("upload-overwrite-backdrop")
                .size_full()
                .absolute()
                .bg(hsla(0.0, 0.0, 0.0, 0.24))
                .on_mouse_down(MouseButton::Left, {
                    let h = backdrop.clone();
                    move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                        h.update(cx, |v, cx| v.cancel_pending_upload(cx));
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top_1_2()
                .left_1_2()
                .w(px(400.0))
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
                        .child("文件已存在"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::text_secondary())
                        .child(detail),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child({
                            let h = handle.clone();
                            button("取消", ButtonVariant::Secondary, Some(ACCENT), dark)
                                .id("upload-overwrite-cancel")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.cancel_pending_upload(cx));
                                })
                        })
                        .child({
                            let h = handle.clone();
                            button("跳过已有", ButtonVariant::Secondary, Some(ACCENT), dark)
                                .id("upload-overwrite-skip")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.confirm_pending_upload(false, cx));
                                })
                        })
                        .child({
                            let h = handle;
                            button("全部覆盖", ButtonVariant::Primary, Some(ACCENT), dark)
                                .id("upload-overwrite-replace")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.confirm_pending_upload(true, cx));
                                })
                        }),
                ),
        )
}
