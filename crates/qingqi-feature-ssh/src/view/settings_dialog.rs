//! Profile 编辑弹窗 (Overlay)
//!
//! 关键设计：遮罩层和弹窗内容区是同级元素（非嵌套），
//! 避免 GPUI 点击事件冒泡导致弹窗误关闭。

use gpui::*;
use qingqi_ui::ui;
use qingqi_ui::text_input::TextInput;

pub struct ProfileFormInputs {
    pub name: Entity<TextInput>,
    pub host: Entity<TextInput>,
    pub port: Entity<TextInput>,
}

pub fn render_profile_editor(
    handle: Entity<super::SshView>,
    inputs: &ProfileFormInputs,
) -> impl IntoElement {
    let h = handle.clone();
    let h_close = handle.clone();
    let h_save = handle.clone();
    let name = inputs.name.clone();
    let host = inputs.host.clone();
    let port = inputs.port.clone();

    // 使用容器 div 包裹遮罩和弹窗（同级元素）
    div()
        .size_full().relative()
        // 遮罩层：点击关闭
        .child(
            div()
                .id("settings-overlay")
                .absolute().size_full().top_0().left_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.4))
                .on_click(move |_, _, cx| {
                    cx.update_entity(&h_close, |view, cx| view.toggle_settings(cx));
                }),
        )
        // 弹窗内容区：独立定位，无关闭事件
        .child(
            div()
                .absolute()
                .top(px(80.0))
                .left_0().right_0()
                .flex().justify_center()
                .child(
                    div()
                        .w(px(480.0)).rounded_lg().bg(ui::bg_surface()).shadow_lg()
                        .flex().flex_col()
                        .child(render_dialog_header(handle))
                        .child(render_dialog_body(name, host, port))
                        .child(render_dialog_footer(h_save, h)),
                ),
        )
}

fn render_dialog_header(handle: Entity<super::SshView>) -> impl IntoElement {
    div()
        .h(px(48.0)).flex().items_center().px_4().justify_between()
        .border_b_1().border_color(ui::border_light())
        .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("新建连接"))
        .child(
            div().id("settings-close").size(px(24.0)).flex().items_center().justify_center()
                .rounded_md().cursor_pointer().hover(|s| s.bg(ui::bg_hover()))
                .on_click(move |_, _, cx| { cx.update_entity(&handle, |v, cx| v.toggle_settings(cx)); })
                .child("✕"),
        )
}

fn render_dialog_body(
    name_input: Entity<TextInput>,
    host_input: Entity<TextInput>,
    port_input: Entity<TextInput>,
) -> impl IntoElement {
    div().flex_1().flex().flex_col().gap(px(12.0)).p_4()
        .child(render_field("名称", name_input))
        .child(render_field("主机", host_input))
        .child(render_field("端口", port_input))
}

fn render_field(label: &str, input: Entity<TextInput>) -> impl IntoElement {
    div().flex().flex_col().gap(px(4.0))
        .child(div().text_size(px(12.0)).text_color(ui::text_secondary()).child(label.to_string()))
        .child(input)
}

fn render_dialog_footer(
    save_handle: Entity<super::SshView>,
    cancel_handle: Entity<super::SshView>,
) -> impl IntoElement {
    div()
        .h(px(56.0)).flex().items_center().justify_end().gap(px(8.0)).px_4()
        .border_t_1().border_color(ui::border_light())
        .child(
            div().id("btn-save-profile")
                .px_4().py(px(6.0)).rounded_md().text_size(px(13.0))
                .bg(hsla(0.55, 0.7, 0.5, 1.0)).text_color(hsla(0.0, 0.0, 1.0, 1.0)).cursor_pointer()
                .hover(|s| s.bg(hsla(0.55, 0.7, 0.5, 0.8)))
                .on_click(move |_, _, cx| {
                    cx.update_entity(&save_handle, |v, cx| v.create_profile_from_form(cx));
                })
                .child("保存"),
        )
        .child(
            div().id("btn-cancel").px_4().py(px(6.0)).rounded_md().text_size(px(13.0)).cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()))
                .on_click(move |_, _, cx| {
                    cx.update_entity(&cancel_handle, |v, cx| v.toggle_settings(cx));
                })
                .child("取消"),
        )
}
