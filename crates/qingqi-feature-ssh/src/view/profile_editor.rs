//! Profile 新建/编辑弹窗

use gpui::prelude::*;
use gpui::{point, *};
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::components::overlay_host;
use qingqi_ui::ui::glass;

use crate::model::{ProtocolType, SshAuthMethod};

const ACCENT: PluginAccent = PluginAccent::Cyan;

pub struct ProfileFormInputs {
    pub name: Entity<TextInput>,
    pub host: Entity<TextInput>,
    pub port: Entity<TextInput>,
    pub username: Entity<TextInput>,
    pub password: Entity<TextInput>,
    pub remote_root: Entity<TextInput>,
    pub local_root: Entity<TextInput>,
    pub private_key_path: Entity<TextInput>,
    pub private_key_passphrase: Entity<TextInput>,
    pub note: Entity<TextInput>,
    pub connection_timeout: Entity<TextInput>,
    pub keepalive_interval: Entity<TextInput>,
}

pub fn render_profile_editor(
    handle: Entity<super::SshView>,
    inputs: &ProfileFormInputs,
    protocol: &ProtocolType,
    auth_method: &SshAuthMethod,
    advanced_expanded: bool,
    is_edit: bool,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
    let h = handle.clone();
    let dialog_handle = handle.clone();
    overlay_host(
        dark,
        "profile-editor-backdrop",
        move |_, _, cx| {
            h.update(cx, |v, cx| v.close_profile_editor(cx));
        },
        render_dialog_card(
            dialog_handle,
            inputs,
            protocol,
            auth_method,
            advanced_expanded,
            is_edit,
            dark,
        ),
    )
}

fn render_dialog_card(
    handle: Entity<super::SshView>,
    inputs: &ProfileFormInputs,
    protocol: &ProtocolType,
    auth_method: &SshAuthMethod,
    advanced_expanded: bool,
    is_edit: bool,
    dark: bool,
) -> impl IntoElement {
    let name = inputs.name.clone();
    let host = inputs.host.clone();
    let port = inputs.port.clone();
    let username = inputs.username.clone();
    let password = inputs.password.clone();
    let remote_root = inputs.remote_root.clone();
    let local_root = inputs.local_root.clone();
    let private_key_path = inputs.private_key_path.clone();
    let private_key_passphrase = inputs.private_key_passphrase.clone();
    let note = inputs.note.clone();
    let connection_timeout = inputs.connection_timeout.clone();
    let keepalive_interval = inputs.keepalive_interval.clone();
    let proto = protocol.clone();
    let auth = auth_method.clone();

    div()
        .id("profile-editor-card")
        .w(px(440.0))
        .h(px(540.0))
        .rounded(theme::radius_sheet())
        .bg(theme::semantic().bg_elevated)
        .border_1()
        .border_color(theme::semantic().border_default)
        .shadow(glass::shadow())
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(render_header(&handle, is_edit, dark))
        .child(
            div()
                .id("profile-editor-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .px(theme::space_5())
                .py(theme::space_4())
                .flex()
                .flex_col()
                .gap(theme::space_4())
                .child(section_block(
                    "协议",
                    div()
                        .p(theme::space_2())
                        .child(render_protocol_selector(&handle, &proto, dark)),
                ))
                .child(section_block(
                    "基本信息",
                    settings_group(dark)
                        .child(group_row(group_field("名称", &name, true), true))
                        .child(group_row(group_field("主机", &host, true), true))
                        .child(group_row(group_field("端口", &port, false), false)),
                ))
                .when(
                    matches!(proto, ProtocolType::Ssh),
                    |el| {
                        el.child(section_block(
                            "认证",
                            render_auth_selector(
                                &handle,
                                &auth,
                                &username,
                                &password,
                                &private_key_path,
                                &private_key_passphrase,
                                dark,
                            ),
                        ))
                    },
                )
                .when(
                    matches!(proto, ProtocolType::Ftp | ProtocolType::Ftps),
                    |el| {
                        el.child(section_block(
                            "FTP 认证",
                            settings_group(dark)
                                .child(group_row(group_field("用户名", &username, false), true))
                                .child(group_row(group_field("密码", &password, false), false)),
                        ))
                    },
                )
                .child(section_block(
                    "路径",
                    settings_group(dark)
                        .child(group_row(group_field("远程根目录", &remote_root, false), true))
                        .child(group_row(group_field("本地下载", &local_root, false), false)),
                ))
                .child(render_advanced_section(
                    &handle,
                    advanced_expanded,
                    &note,
                    &connection_timeout,
                    &keepalive_interval,
                    dark,
                )),
        )
        .child(render_footer(&handle, is_edit, dark))
}

fn render_header(handle: &Entity<super::SshView>, is_edit: bool, dark: bool) -> impl IntoElement {
    let h = handle.clone();
    div()
        .flex_shrink_0()
        .h(px(48.0))
        .flex()
        .items_center()
        .px(theme::space_5())
        .justify_between()
        .border_b_1()
        .border_color(glass::divider(dark))
        .child(
            div()
                .text_size(theme::font_size_heading())
                .font_weight(FontWeight::SEMIBOLD)
                .child(if is_edit { "编辑连接" } else { "新建连接" }),
        )
        .child(
            div()
                .id("profile-editor-close")
                .size(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .cursor_pointer()
                .text_size(px(14.0))
                .text_color(ui::text_tertiary())
                .hover(|s| s.bg(glass::hover_bg(dark)))
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h.update(cx, |v, cx| v.close_profile_editor(cx));
                })
                .child("×"),
        )
}

fn render_advanced_section(
    handle: &Entity<super::SshView>,
    expanded: bool,
    note: &Entity<TextInput>,
    timeout: &Entity<TextInput>,
    keepalive: &Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let h = handle.clone();
    div()
        .flex()
        .flex_col()
        .gap(theme::space_2())
        .child(
            div()
                .id("btn-toggle-advanced")
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .text_size(theme::font_size_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_tertiary())
                .hover(|s| s.text_color(ui::text_primary()))
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h.update(cx, |v, cx| v.toggle_form_advanced(cx));
                })
                .child("高级选项")
                .child(if expanded { "▾" } else { "▸" }),
        )
        .when(expanded, |el| {
            el.child(
                settings_group(dark)
                    .child(group_row(group_field("备注", note, false), true))
                    .child(group_row(group_field("连接超时", timeout, false), true))
                    .child(group_row(group_field("保活间隔", keepalive, false), false)),
            )
        })
}

fn section_block(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(theme::space_2())
        .child(
            div()
                .text_size(theme::font_size_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_tertiary())
                .child(title),
        )
        .child(content)
}

fn settings_group(dark: bool) -> Div {
    div()
        .rounded(theme::radius_lg())
        .border_1()
        .border_color(glass::border(dark))
        .bg(theme::semantic().bg_surface)
        .overflow_hidden()
        .flex()
        .flex_col()
}

fn group_row(content: impl IntoElement, show_divider: bool) -> impl IntoElement {
    let s = theme::semantic();
    div()
        .min_h(px(40.0))
        .px(theme::space_3())
        .py(theme::space_1p5())
        .when(show_divider, |el| el.border_b_1().border_color(s.border_default))
        .flex()
        .items_center()
        .gap(theme::space_3())
        .child(content)
}

fn group_field(label: &str, input: &Entity<TextInput>, required: bool) -> impl IntoElement {
    let label_text = if required {
        format!("{label} *")
    } else {
        label.to_string()
    };
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(theme::space_3())
        .child(
            div()
                .w(px(72.0))
                .flex_shrink_0()
                .text_size(theme::font_size_body())
                .child(label_text),
        )
        .child(div().flex_1().min_w_0().child(input.clone()))
}

fn segmented_control(dark: bool) -> Div {
    div()
        .p(px(2.0))
        .rounded(px(7.0))
        .bg(if dark {
            hsla(0.0, 0.0, 0.0, 0.22)
        } else {
            theme::rgba_with_alpha(theme::slate_200(), 0.85)
        })
        .flex()
        .gap(px(1.0))
}

fn segment_btn(
    label: &'static str,
    idx: u64,
    selected: bool,
    dark: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(("segment", idx))
        .flex_1()
        .h(px(24.0))
        .rounded(px(5.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::MEDIUM
        })
        .cursor_pointer()
        .bg(if selected {
            if dark {
                hsla(0.0, 0.0, 0.35, 0.85)
            } else {
                theme::white().into()
            }
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .text_color(if selected {
            ui::text_primary()
        } else {
            ui::text_secondary()
        })
        .shadow(if selected && !dark {
            vec![gpui::BoxShadow {
                color: theme::rgba_with_alpha(theme::semantic().shadow, 0.08),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(2.0),
                spread_radius: px(0.0),
            }]
        } else {
            vec![]
        })
        .on_click(on_click)
        .child(label)
}

fn render_protocol_selector(
    handle: &Entity<super::SshView>,
    current: &ProtocolType,
    dark: bool,
) -> impl IntoElement {
    segmented_control(dark)
        .child(proto_segment(handle, "SSH", 0, ProtocolType::Ssh, current, dark))
        .child(proto_segment(handle, "FTP", 1, ProtocolType::Ftp, current, dark))
        .child(proto_segment(handle, "FTPS", 2, ProtocolType::Ftps, current, dark))
}

fn proto_segment(
    handle: &Entity<super::SshView>,
    label: &'static str,
    idx: u64,
    proto: ProtocolType,
    current: &ProtocolType,
    dark: bool,
) -> impl IntoElement {
    let selected = std::mem::discriminant(&proto) == std::mem::discriminant(current);
    let h = handle.clone();
    segment_btn(label, idx, selected, dark, move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        h.update(cx, |v, cx| v.set_form_protocol(proto.clone(), cx));
    })
}

fn render_auth_selector(
    handle: &Entity<super::SshView>,
    current: &SshAuthMethod,
    username_input: &Entity<TextInput>,
    password_input: &Entity<TextInput>,
    key_path: &Entity<TextInput>,
    key_passphrase: &Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let is_password = matches!(current, SshAuthMethod::Password { .. });
    let is_key = matches!(current, SshAuthMethod::PrivateKey { .. });
    let is_agent = matches!(current, SshAuthMethod::Agent);

    settings_group(dark)
        .child(
            div()
                .p(theme::space_2())
                .child(
                    segmented_control(dark)
                        .child(auth_segment(
                            handle,
                            "密码",
                            0,
                            is_password,
                            SshAuthMethod::Password {
                                password: String::new(),
                            },
                            dark,
                        ))
                        .child(auth_segment(
                            handle,
                            "私钥",
                            1,
                            is_key,
                            SshAuthMethod::PrivateKey {
                                path: String::new(),
                                passphrase: String::new(),
                            },
                            dark,
                        ))
                        .child(auth_segment(
                            handle,
                            "Agent",
                            2,
                            is_agent,
                            SshAuthMethod::Agent,
                            dark,
                        )),
                ),
        )
        .when(is_password, |el| {
            el.child(group_row(group_field("用户名", username_input, false), true))
                .child(group_row(group_field("密码", password_input, false), false))
        })
        .when(is_key, |el| {
            el.child(group_row(group_field("私钥路径", key_path, false), true))
                .child(group_row(group_field("私钥密码", key_passphrase, false), false))
        })
        .when(is_agent, |el| {
            el.child(
                div()
                    .px(theme::space_3())
                    .py(theme::space_2())
                    .text_size(theme::font_size_caption())
                    .text_color(ui::text_tertiary())
                    .child("使用系统 SSH Agent，无需额外配置"),
            )
        })
}

fn auth_segment(
    handle: &Entity<super::SshView>,
    label: &'static str,
    idx: u64,
    selected: bool,
    method: SshAuthMethod,
    dark: bool,
) -> impl IntoElement {
    let h = handle.clone();
    segment_btn(label, idx, selected, dark, move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        h.update(cx, |v, cx| v.set_form_auth_method(method.clone(), cx));
    })
}

fn render_footer(handle: &Entity<super::SshView>, is_edit: bool, dark: bool) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .h(px(56.0))
        .flex()
        .items_center()
        .justify_between()
        .px(theme::space_5())
        .border_t_1()
        .border_color(glass::divider(dark))
        .bg(theme::semantic().bg_elevated)
        .child(if is_edit {
            let h = handle.clone();
            button("删除连接", ButtonVariant::Ghost, None, dark)
                .id("btn-delete-profile")
                .text_color(ui::danger())
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    h.update(cx, |v, cx| {
                        if let Some(pid) = v.editing_profile_id {
                            v.delete_profile(pid, cx);
                            v.close_profile_editor(cx);
                        }
                    });
                })
                .into_any_element()
        } else {
            div().into_any_element()
        })
        .child({
            let h_save = handle.clone();
            let h_cancel = handle.clone();
            div()
                .flex()
                .gap(theme::space_2())
                .child(
                    button("取消", ButtonVariant::Secondary, None, dark)
                        .id("btn-cancel")
                        .cursor_pointer()
                        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            h_cancel.update(cx, |v, cx| v.close_profile_editor(cx));
                        }),
                )
                .child(
                    button("保存", ButtonVariant::Primary, Some(ACCENT), dark)
                        .id("btn-save-profile")
                        .cursor_pointer()
                        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            h_save.update(cx, |v, cx| v.save_profile_from_form(cx));
                        }),
                )
        })
}
