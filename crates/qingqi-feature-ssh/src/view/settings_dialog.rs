//! Profile 编辑弹窗 — 模态框（遮罩不可点击关闭）

use gpui::*;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::ui;
use qingqi_ui::text_input::TextInput;

pub struct ProfileFormInputs {
    pub name: Entity<TextInput>,
    pub host: Entity<TextInput>,
    pub port: Entity<TextInput>,
    pub username: Entity<TextInput>,
    pub password: Entity<TextInput>,
    pub remote_root: Entity<TextInput>,
    pub local_root: Entity<TextInput>,
}

pub fn render_profile_editor(
    handle: Entity<super::SshView>,
    inputs: &ProfileFormInputs,
) -> impl IntoElement {
    let h_save = handle.clone();
    let h_cancel = handle.clone();
    let name = inputs.name.clone();
    let host = inputs.host.clone();
    let port = inputs.port.clone();
    let username = inputs.username.clone();
    let password = inputs.password.clone();
    let remote_root = inputs.remote_root.clone();
    let local_root = inputs.local_root.clone();

    div()
        .size_full().relative()
        // 遮罩层：纯视觉，不可点击
        .child(
            div()
                .absolute().size_full().top_0().left_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.5)),
        )
        // 弹窗内容
        .child(
            div()
                .absolute().top(px(60.0)).left_0().right_0()
                .flex().justify_center()
                .child(
                    div()
                        .w(px(500.0)).rounded(px(12.0)).bg(ui::bg_surface()).shadow_2xl()
                        .flex().flex_col().overflow_hidden()
                        .child(render_header(handle))
                        .child(render_body(name, host, port, username, password, remote_root, local_root))
                        .child(render_footer(h_save, h_cancel)),
                ),
        )
}

fn render_header(handle: Entity<super::SshView>) -> impl IntoElement {
    let h = handle.clone();
    div()
        .h(px(48.0)).flex().items_center().px_4().justify_between()
        .border_b_1().border_color(ui::border_light())
        .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("新建连接"))
        .child(
            div().id("settings-close").size(px(24.0)).flex().items_center().justify_center()
                .rounded(px(6.0)).cursor_pointer().hover(|s| s.bg(ui::bg_hover()))
                .on_click(move |_, _, cx| { cx.update_entity(&h, |v, cx| v.toggle_settings(cx)); })
                .child("✕"),
        )
}

fn render_body(
    name: Entity<TextInput>,
    host: Entity<TextInput>,
    port: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    remote_root: Entity<TextInput>,
    local_root: Entity<TextInput>,
) -> impl IntoElement {
    div().flex_1().flex().flex_col().gap(px(10.0)).p_4().overflow_y_scrollbar()
        .child(render_field("名称 *", name))
        .child(render_field("主机 *", host))
        .child(render_field("端口", port))
        .child(div().h(px(1.0)).bg(ui::border_light()))
        .child(render_field("用户名", username))
        .child(render_field("密码", password))
        .child(div().h(px(1.0)).bg(ui::border_light()))
        .child(render_field("远程根目录", remote_root))
        .child(render_field("本地下载目录", local_root))
}

fn render_field(label: &str, input: Entity<TextInput>) -> impl IntoElement {
    div().flex().flex_col().gap(px(4.0))
        .child(div().text_size(px(12.0)).text_color(ui::text_secondary()).child(label.to_string()))
        .child(div().h(px(32.0)).flex().items_center().child(input))
}

fn render_footer(
    save_handle: Entity<super::SshView>,
    cancel_handle: Entity<super::SshView>,
) -> impl IntoElement {
    div()
        .h(px(52.0)).flex().items_center().justify_end().gap(px(8.0)).px_4()
        .border_t_1().border_color(ui::border_light())
        .child(
            div().id("btn-cancel").px_4().py(px(6.0)).rounded(px(6.0)).text_size(px(13.0)).cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()))
                .on_click(move |_, _, cx| {
                    cx.update_entity(&cancel_handle, |v, cx| v.toggle_settings(cx));
                })
                .child("取消"),
        )
        .child(
            div().id("btn-save-profile")
                .px_4().py(px(6.0)).rounded(px(6.0)).text_size(px(13.0))
                .bg(hsla(0.55, 0.7, 0.5, 1.0)).text_color(hsla(0.0, 0.0, 1.0, 1.0)).cursor_pointer()
                .hover(|s| s.bg(hsla(0.55, 0.7, 0.45, 0.9)))
                .on_click(move |_, _, cx| {
                    cx.update_entity(&save_handle, |v, cx| v.create_profile_from_form(cx));
                })
                .child("保存"),
        )
}
