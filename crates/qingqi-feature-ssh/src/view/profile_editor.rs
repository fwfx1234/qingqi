//! Profile 新建/编辑弹窗

use gpui::prelude::*;
use gpui::{point, *};
use gpui_component::scroll::ScrollableElement;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::components::button::{ButtonVariant, button};
use qingqi_ui::ui::glass;

use crate::model::{ProtocolType, SshAuthMethod};

const ACCENT: PluginAccent = PluginAccent::Cyan;
const FIELD_LABEL_WIDTH: f32 = 88.0;
const FIELD_ROW_MIN_HEIGHT: f32 = 36.0;
const FIELD_INPUT_HEIGHT: f32 = 32.0;
const FIELD_TEXTAREA_HEIGHT: f32 = 52.0;

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
    pub keepalive_max: Entity<TextInput>,
}

#[derive(Clone, Copy, Debug)]
pub struct ProfileAdvancedFlags {
    pub tcp_nodelay: bool,
    pub ftp_passive_mode: bool,
    pub ftp_passive_nat_workaround: bool,
}

pub fn render_profile_editor_panel(
    handle: Entity<super::SshView>,
    inputs: &ProfileFormInputs,
    protocol: &ProtocolType,
    auth_method: &SshAuthMethod,
    advanced_flags: &ProfileAdvancedFlags,
    advanced_expanded: bool,
    is_edit: bool,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
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
    let keepalive_max = inputs.keepalive_max.clone();
    let advanced = *advanced_flags;
    let proto = protocol.clone();
    let auth = auth_method.clone();

    div()
        .id("profile-editor-card")
        .size_full()
        .min_h(px(0.0))
        .bg(theme::semantic().bg_elevated)
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .flex_shrink_0()
                .px(theme::space_4())
                .pt(theme::space_3())
                .pb(theme::space_3())
                .border_b_1()
                .border_color(glass::divider(dark))
                .child(render_protocol_selector(&handle, &proto, dark)),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(
                    div()
                        .id("profile-editor-scroll")
                        .size_full()
                        .overflow_y_scrollbar()
                        .px(theme::space_4())
                        .py(theme::space_3())
                        .flex()
                        .flex_col()
                        .gap(theme::space_3())
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
                                        .child(group_row(
                                            group_field("用户名", &username, false),
                                            true,
                                        ))
                                        .child(group_row(
                                            group_field("密码", &password, false),
                                            false,
                                        )),
                                ))
                            },
                        )
                        .child(section_block(
                            "路径",
                            settings_group(dark)
                                .child(group_row(
                                    group_field("远程根目录", &remote_root, false),
                                    true,
                                ))
                                .child(group_row(
                                    group_field("本地下载", &local_root, false),
                                    false,
                                )),
                        ))
                        .child(render_advanced_section(
                            &handle,
                            &proto,
                            advanced_expanded,
                            advanced,
                            &note,
                            &connection_timeout,
                            &keepalive_interval,
                            &keepalive_max,
                            dark,
                        )),
                ),
        )
        .child(render_footer(&handle, is_edit, dark))
}

fn render_advanced_section(
    handle: &Entity<super::SshView>,
    protocol: &ProtocolType,
    expanded: bool,
    flags: ProfileAdvancedFlags,
    note: &Entity<TextInput>,
    timeout: &Entity<TextInput>,
    keepalive: &Entity<TextInput>,
    keepalive_max: &Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let h = handle.clone();
    let chevron = if expanded {
        "icons/chevron-up.svg"
    } else {
        "icons/chevron-down.svg"
    };
    let is_ssh = matches!(protocol, ProtocolType::Ssh);
    let is_ftp = matches!(protocol, ProtocolType::Ftp | ProtocolType::Ftps);

    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .id("btn-toggle-advanced")
                .h(px(34.0))
                .px(theme::space_3())
                .rounded(theme::radius_md())
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .text_size(theme::font_size_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_secondary())
                .hover(|s| {
                    s.bg(glass::hover_bg(dark))
                        .text_color(ui::text_primary())
                })
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    cx.stop_propagation();
                    h.update(cx, |v, cx| v.toggle_form_advanced(cx));
                })
                .child("高级选项")
                .child(ui::icon_element(chevron, ui::text_tertiary(), 18.0)),
        )
        .when(expanded, |el| {
            el.child(render_advanced_fields(
                handle,
                is_ssh,
                is_ftp,
                flags,
                note,
                timeout,
                keepalive,
                keepalive_max,
                dark,
            ))
        })
}

fn render_advanced_fields(
    handle: &Entity<super::SshView>,
    is_ssh: bool,
    is_ftp: bool,
    flags: ProfileAdvancedFlags,
    note: &Entity<TextInput>,
    timeout: &Entity<TextInput>,
    keepalive: &Entity<TextInput>,
    keepalive_max: &Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let mut group = settings_group(dark)
        .child(group_row(group_field_multiline("备注", note, false), true));

    if is_ssh {
        group = group
            .child(group_row(
                group_field("无活动超时 (秒，0=不限)", timeout, false),
                true,
            ))
            .child(group_row(
                group_field("保活间隔 (秒)", keepalive, false),
                true,
            ))
            .child(group_row(
                group_field("保活重试次数", keepalive_max, false),
                true,
            ))
            .child(group_row(
                bool_toggle_field(
                    handle,
                    "tcp-nodelay",
                    "TCP_NODELAY",
                    "降低终端交互延迟",
                    flags.tcp_nodelay,
                    dark,
                    super::SshView::set_form_tcp_nodelay,
                ),
                false,
            ));
    }

    if is_ftp {
        group = group
            .child(group_row(
                bool_toggle_field(
                    handle,
                    "ftp-passive-mode",
                    "被动模式",
                    "PASV 模式，适合大多数网络环境",
                    flags.ftp_passive_mode,
                    dark,
                    super::SshView::set_form_ftp_passive_mode,
                ),
                !is_ssh,
            ))
            .child(group_row(
                bool_toggle_field(
                    handle,
                    "ftp-nat-workaround",
                    "NAT 穿透修正",
                    "被动模式在 NAT 后连接失败时尝试开启",
                    flags.ftp_passive_nat_workaround,
                    dark,
                    super::SshView::set_form_ftp_passive_nat_workaround,
                ),
                false,
            ));
    }

    group
}

fn section_block(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_tertiary())
                .child(title),
        )
        .child(content)
}

fn settings_group(dark: bool) -> Div {
    div()
        .rounded(theme::radius_md())
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
        .min_h(px(FIELD_ROW_MIN_HEIGHT))
        .px(theme::space_3())
        .py(px(6.0))
        .when(show_divider, |el| el.border_b_1().border_color(s.border_default))
        .flex()
        .items_center()
        .gap(theme::space_3())
        .child(content)
}

fn input_slot(input: &Entity<TextInput>, height: f32) -> impl IntoElement {
    let focus_target = input.clone();
    div()
        .flex_1()
        .min_w_0()
        .h(px(height))
        .flex()
        .items_center()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            window.focus(&focus_target.read(cx).focus_handle(cx));
        })
        .child(input.clone())
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
        .h_full()
        .gap(theme::space_3())
        .child(field_label(&label_text))
        .child(input_slot(input, FIELD_INPUT_HEIGHT))
}

fn group_field_multiline(label: &str, input: &Entity<TextInput>, required: bool) -> impl IntoElement {
    let label_text = if required {
        format!("{label} *")
    } else {
        label.to_string()
    };
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(theme::space_3())
        .child(
            field_label(&label_text).pt(px(6.0)),
        )
        .child(input_slot(input, FIELD_TEXTAREA_HEIGHT))
}

type FormSetBoolFn = fn(&mut super::SshView, bool, &mut Context<super::SshView>);

fn bool_toggle_field(
    handle: &Entity<super::SshView>,
    id_prefix: &'static str,
    label: &str,
    hint: &str,
    enabled: bool,
    dark: bool,
    set_value: FormSetBoolFn,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(theme::space_3())
        .child(
            div()
                .w(px(FIELD_LABEL_WIDTH))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(theme::font_size_body())
                        .text_color(ui::text_secondary())
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(ui::text_tertiary())
                        .child(hint.to_string()),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .justify_end()
                .child(bool_toggle(handle, id_prefix, enabled, dark, set_value)),
        )
}

fn bool_toggle(
    handle: &Entity<super::SshView>,
    id_prefix: &'static str,
    enabled: bool,
    dark: bool,
    set_value: FormSetBoolFn,
) -> impl IntoElement {
    let h_on = handle.clone();
    let h_off = handle.clone();
    segmented_control(dark)
        .w(px(120.0))
        .flex_shrink_0()
        .child(segment_btn(id_prefix, "开启", 0, enabled, dark, move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            h_on.update(cx, |v, cx| set_value(v, true, cx));
        }))
        .child(segment_btn(id_prefix, "关闭", 1, !enabled, dark, move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            h_off.update(cx, |v, cx| set_value(v, false, cx));
        }))
}

fn field_label(text: &str) -> Div {
    div()
        .w(px(FIELD_LABEL_WIDTH))
        .flex_shrink_0()
        .text_size(theme::font_size_body())
        .text_color(ui::text_secondary())
        .child(text.to_string())
}

fn segmented_control(dark: bool) -> Div {
    div()
        .p(px(2.0))
        .rounded(theme::radius_md())
        .bg(if dark {
            hsla(0.0, 0.0, 0.0, 0.22)
        } else {
            theme::rgba_with_alpha(theme::slate_200(), 0.85)
        })
        .flex()
        .gap(px(1.0))
}

fn segment_btn(
    id_prefix: &'static str,
    label: &'static str,
    idx: u64,
    selected: bool,
    dark: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id((id_prefix, idx))
        .flex_1()
        .h(px(28.0))
        .rounded(theme::radius_sm())
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
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .child(label)
}

fn render_protocol_selector(
    handle: &Entity<super::SshView>,
    current: &ProtocolType,
    dark: bool,
) -> impl IntoElement {
    segmented_control(dark)
        .child(proto_segment(handle, "profile-protocol", "SSH", 0, ProtocolType::Ssh, current, dark))
        .child(proto_segment(handle, "profile-protocol", "FTP", 1, ProtocolType::Ftp, current, dark))
        .child(proto_segment(handle, "profile-protocol", "FTPS", 2, ProtocolType::Ftps, current, dark))
}

fn proto_segment(
    handle: &Entity<super::SshView>,
    id_prefix: &'static str,
    label: &'static str,
    idx: u64,
    proto: ProtocolType,
    current: &ProtocolType,
    dark: bool,
) -> impl IntoElement {
    let selected = std::mem::discriminant(&proto) == std::mem::discriminant(current);
    let h = handle.clone();
    segment_btn(id_prefix, label, idx, selected, dark, move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
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
                            "profile-auth",
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
                            "profile-auth",
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
                            "profile-auth",
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
            el.child(group_row(group_field("用户名", username_input, false), true))
                .child(group_row(group_field("私钥路径", key_path, false), true))
                .child(group_row(group_field("私钥密码", key_passphrase, false), false))
        })
        .when(is_agent, |el| {
            el.child(group_row(group_field("用户名", username_input, false), true))
                .child(
                    div()
                        .px(theme::space_3())
                        .py(theme::space_2())
                        .text_size(theme::font_size_caption())
                        .text_color(ui::text_tertiary())
                        .child("使用系统 SSH Agent 认证，无需配置密码或私钥"),
                )
        })
}

fn auth_segment(
    handle: &Entity<super::SshView>,
    id_prefix: &'static str,
    label: &'static str,
    idx: u64,
    selected: bool,
    method: SshAuthMethod,
    dark: bool,
) -> impl IntoElement {
    let h = handle.clone();
    segment_btn(id_prefix, label, idx, selected, dark, move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        h.update(cx, |v, cx| v.set_form_auth_method(method.clone(), cx));
    })
}

fn render_footer(handle: &Entity<super::SshView>, is_edit: bool, dark: bool) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .h(px(44.0))
        .flex()
        .items_center()
        .justify_between()
        .px(theme::space_4())
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
