//! Profile 编辑弹窗 (Overlay)

use gpui::*;
use qingqi_ui::ui;

/// 渲染 Profile 编辑/新建 Overlay
pub fn render_profile_editor(
    _is_new: bool,
) -> impl IntoElement {
    // 半透明遮罩
    div()
        .absolute()
        .size_full()
        .top_0()
        .left_0()
        .bg(hsla(0.0, 0.0, 0.0, 0.4))
        .flex()
        .items_center()
        .justify_center()
        .child(render_dialog())
}

fn render_dialog() -> impl IntoElement {
    div()
        .w(px(520.0))
        .rounded_lg()
        .bg(ui::bg_surface())
        .shadow_lg()
        .flex()
        .flex_col()
        .child(render_dialog_header())
        .child(render_dialog_body())
        .child(render_dialog_footer(false))
}

fn render_dialog_header() -> impl IntoElement {
    div()
        .h(px(48.0))
        .flex()
        .items_center()
        .px_4()
        .justify_between()
        .border_b_1()
        .border_color(ui::border_light())
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("新建连接"),
        )
        .child(
            div()
                .size(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()))
                .child("✕"),
        )
}

fn render_dialog_body() -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .p_4()
        .child(render_field("名称", "我的服务器"))
        .child(render_field("主机", "192.168.1.1"))
        .child(render_field("端口", "22"))
        .child(render_field("用户名", "root"))
        .child(render_field("密码", "••••••••"))
        .child(render_field("远程根目录", "/home/user"))
        .child(render_field("本地下载目录", "~/Downloads"))
}

fn render_field(label: &str, placeholder: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(ui::text_secondary())
                .child(label.to_string()),
        )
        .child(
            div()
                .h(px(32.0))
                .flex()
                .items_center()
                .px_3()
                .rounded_md()
                .border_1()
                .border_color(ui::border_light())
                .text_size(px(13.0))
                .child(placeholder.to_string()),
        )
}

fn render_dialog_footer(_is_edit: bool) -> impl IntoElement {
    div()
        .h(px(56.0))
        .flex()
        .items_center()
        .justify_end()
        .gap(px(8.0))
        .px_4()
        .border_t_1()
        .border_color(ui::border_light())
        .child(
            div()
                .px_4()
                .py_2()
                .rounded_md()
                .text_size(px(13.0))
                .cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()))
                .child("测试连接"),
        )
        .child(
            div()
                .px_4()
                .py_2()
                .rounded_md()
                .text_size(px(13.0))
                .bg(hsla(0.55, 0.7, 0.5, 1.0))
                .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                .cursor_pointer()
                .hover(|s| s.bg(hsla(0.55, 0.7, 0.5, 0.8)))
                .child("保存"),
        )
        .child(
            div()
                .px_4()
                .py_2()
                .rounded_md()
                .text_size(px(13.0))
                .cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()))
                .child("取消"),
        )
}
