//! Profile 编辑弹窗 (Overlay)

use gpui::*;
use qingqi_ui::ui;
use qingqi_ui::text_input::TextInput;

pub struct ProfileFormInputs {
    pub name: Entity<TextInput>,
    pub host: Entity<TextInput>,
    pub port: Entity<TextInput>,
    pub username: Entity<TextInput>,
}

pub fn render_profile_editor(
    handle: Entity<super::SshView>,
    inputs: &ProfileFormInputs,
) -> impl IntoElement {
    let h = handle.clone();
    div()
        .id("settings-overlay")
        .absolute().size_full().top_0().left_0()
        .bg(hsla(0.0, 0.0, 0.0, 0.4))
        .flex().items_center().justify_center()
        .on_click(move |_, _, cx| {
            cx.update_entity(&h, |view, cx| view.toggle_settings(cx));
        })
        .child(
            div()
                .id("settings-dialog")
                .w(px(520.0)).rounded_lg().bg(ui::bg_surface()).shadow_lg()
                .flex().flex_col()
                .on_click(|_, _, _| {})
                .child(render_dialog_header(handle.clone()))
                .child(render_dialog_body(inputs))
                .child(render_dialog_footer(handle)),
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

fn render_dialog_body(inputs: &ProfileFormInputs) -> impl IntoElement {
    div().flex_1().flex().flex_col().gap(px(12.0)).p_4()
        .child(render_text_input("名称", inputs.name.clone()))
        .child(render_text_input("主机", inputs.host.clone()))
        .child(render_text_input("端口", inputs.port.clone()))
        .child(render_text_input("用户名", inputs.username.clone()))
        .child(render_static_field("认证方式", "密码"))
        .child(render_static_field("远程根目录", "~"))
        .child(render_static_field("本地下载目录", "~/Downloads"))
}

fn render_text_input(label: &str, input: Entity<TextInput>) -> impl IntoElement {
    div().flex().flex_col().gap(px(4.0))
        .child(div().text_size(px(12.0)).text_color(ui::text_secondary()).child(label.to_string()))
        .child(div().h(px(32.0)).flex().items_center().px_2().rounded_md()
            .border_1().border_color(ui::border_light())
            .child(input))
}

fn render_static_field(label: &str, value: &str) -> impl IntoElement {
    div().flex().flex_col().gap(px(4.0))
        .child(div().text_size(px(12.0)).text_color(ui::text_secondary()).child(label.to_string()))
        .child(div().h(px(32.0)).flex().items_center().px_3().rounded_md()
            .border_1().border_color(ui::border_light())
            .text_size(px(13.0)).child(value.to_string()))
}

fn render_dialog_footer(handle: Entity<super::SshView>) -> impl IntoElement {
    let h = handle.clone();
    div()
        .h(px(56.0)).flex().items_center().justify_end().gap(px(8.0)).px_4()
        .border_t_1().border_color(ui::border_light())
        .child(div().px_4().py_2().rounded_md().text_size(px(13.0)).cursor_pointer()
            .hover(|s| s.bg(ui::bg_hover())).child("测试连接"))
        .child(
            div().id("btn-save-profile")
                .px_4().py_2().rounded_md().text_size(px(13.0))
                .bg(hsla(0.55, 0.7, 0.5, 1.0)).text_color(hsla(0.0, 0.0, 1.0, 1.0)).cursor_pointer()
                .hover(|s| s.bg(hsla(0.55, 0.7, 0.5, 0.8)))
                .on_click(move |_, _, cx| { cx.update_entity(&h, |v, cx| v.create_profile_from_form(cx)); })
                .child("保存"),
        )
        .child(div().id("btn-cancel")
            .px_4().py_2().rounded_md().text_size(px(13.0)).cursor_pointer()
            .hover(|s| s.bg(ui::bg_hover()))
            .on_click(move |_, _, cx| { cx.update_entity(&handle, |v, cx| v.toggle_settings(cx)); })
            .child("取消"))
}
