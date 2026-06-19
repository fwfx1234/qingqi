//! 上传覆盖确认弹窗

use gpui::prelude::*;
use gpui::*;
use gpui_component::theme::Theme;
use qingqi_ui::ui;
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::glass;

pub fn render_upload_overwrite_overlay(
    handle: Entity<super::SshView>,
    total_items: usize,
    conflict_count: usize,
    sample_name: &str,
    cx: &App,
) -> impl IntoElement {
    let backdrop = handle.clone();
    let single_file = total_items <= 1 && conflict_count <= 1;
    let detail = if single_file {
        format!("远程已存在「{sample_name}」，是否覆盖？")
    } else {
        format!("远程已有 {conflict_count} 个同名文件/文件夹（如「{sample_name}」），是否覆盖？")
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
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .w(px(400.0))
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
                        .child(if single_file {
                            "文件已存在"
                        } else {
                            "部分文件已存在"
                        }),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::text_secondary(cx))
                        .child(detail),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child({
                            let h = handle.clone();
                            button("取消", ButtonVariant::Secondary, None, cx)
                                .id("upload-overwrite-cancel")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.cancel_pending_upload(cx));
                                })
                        })
                        .when(!single_file, |row| {
                            row.child({
                                let h = handle.clone();
                                button("跳过已有", ButtonVariant::Secondary, None, cx)
                                    .id("upload-overwrite-skip")
                                    .on_click(
                                        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                            h.update(cx, |v, cx| {
                                                v.confirm_pending_upload(false, cx)
                                            });
                                        },
                                    )
                            })
                        })
                        .child({
                            let h = handle;
                            let label = if single_file {
                                "覆盖"
                            } else {
                                "全部覆盖"
                            };
                            button(label, ButtonVariant::Primary, None, cx)
                                .id("upload-overwrite-replace")
                                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    h.update(cx, |v, cx| v.confirm_pending_upload(true, cx));
                                })
                        }),
                ),
        )
}
