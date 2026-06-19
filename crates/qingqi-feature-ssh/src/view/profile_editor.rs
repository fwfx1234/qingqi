//! Profile 新建/编辑弹窗

use gpui::prelude::*;
use gpui::{point, *};
use gpui_component::scroll::ScrollableElement;
use gpui_component::theme::Theme;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::ui::glass;
use qingqi_ui::{theme, ui};

use crate::model::{ProtocolType, SshAuthMethod};

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
    cx: &App,
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
    let keepalive_max = inputs.keepalive_max.clone();
    let advanced = *advanced_flags;
    let proto = protocol.clone();
    let auth = auth_method.clone();

    div()
        .id("profile-editor-card")
        .size_full()
        .min_h(px(0.0))
        .bg(Theme::global(cx).popover)
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
                .border_color(glass::divider(cx))
                .child(render_protocol_selector(&handle, &proto, cx)),
        )
        .child(
            div().flex_1().min_h(px(0.0)).overflow_hidden().child(
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
                        settings_group(cx)
                            .child(group_row(group_field("名称", &name, true, cx), true, cx))
                            .child(group_row(group_field("主机", &host, true, cx), true, cx))
                            .child(group_row(group_field("端口", &port, false, cx), false, cx)),
                        cx,
                    ))
                    .when(matches!(proto, ProtocolType::Ssh), |el| {
                        el.child(section_block(
                            "认证",
                            render_auth_selector(
                                &handle,
                                &auth,
                                &username,
                                &password,
                                &private_key_path,
                                &private_key_passphrase,
                                cx,
                            ),
                            cx,
                        ))
                    })
                    .when(
                        matches!(proto, ProtocolType::Ftp | ProtocolType::Ftps),
                        |el| {
                            el.child(section_block(
                                "FTP 认证",
                                settings_group(cx)
                                    .child(group_row(
                                        group_field("用户名", &username, false, cx),
                                        true,
                                        cx,
                                    ))
                                    .child(group_row(
                                        group_field("密码", &password, false, cx),
                                        false,
                                        cx,
                                    )),
                                cx,
                            ))
                        },
                    )
                    .child(section_block(
                        "路径",
                        settings_group(cx)
                            .child(group_row(
                                group_field("远程根目录", &remote_root, false, cx),
                                true,
                                cx,
                            ))
                            .child(group_row(
                                group_field("本地下载", &local_root, false, cx),
                                false,
                                cx,
                            )),
                        cx,
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
                        cx,
                    )),
            ),
        )
        .child(render_footer(&handle, is_edit, cx))
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
    cx: &App,
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
                .text_color(ui::text_secondary(cx))
                .hover(|s| s.bg(glass::hover_bg(cx)).text_color(ui::text_primary(cx)))
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    cx.stop_propagation();
                    h.update(cx, |v, cx| v.toggle_form_advanced(cx));
                })
                .child("高级选项")
                .child(ui::icon_element(chevron, ui::text_tertiary(cx).into(), 18.0)),
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
                cx,
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
    cx: &App,
) -> impl IntoElement {
    let mut group = settings_group(cx).child(group_row(
        group_field_multiline("备注", note, false, cx),
        true,
        cx,
    ));

    if is_ssh {
        group = group
            .child(group_row(
                group_field("无活动超时 (秒，0=不限)", timeout, false, cx),
                true,
                cx,
            ))
            .child(group_row(
                group_field("保活间隔 (秒)", keepalive, false, cx),
                true,
                cx,
            ))
            .child(group_row(
                group_field("保活重试次数", keepalive_max, false, cx),
                true,
                cx,
            ))
            .child(group_row(
                bool_toggle_field(
                    handle,
                    "tcp-nodelay",
                    "TCP_NODELAY",
                    "降低终端交互延迟",
                    flags.tcp_nodelay,
                    cx,
                    super::SshView::set_form_tcp_nodelay,
                ),
                false,
                cx,
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
                    cx,
                    super::SshView::set_form_ftp_passive_mode,
                ),
                !is_ssh,
                cx,
            ))
            .child(group_row(
                bool_toggle_field(
                    handle,
                    "ftp-nat-workaround",
                    "NAT 穿透修正",
                    "被动模式在 NAT 后连接失败时尝试开启",
                    flags.ftp_passive_nat_workaround,
                    cx,
                    super::SshView::set_form_ftp_passive_nat_workaround,
                ),
                false,
                cx,
            ));
    }

    group
}

fn section_block(title: &'static str, content: impl IntoElement, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::text_tertiary(cx))
                .child(title),
        )
        .child(content)
}

fn settings_group(cx: &App) -> Div {
    div()
        .rounded(theme::radius_md())
        .border_1()
        .border_color(glass::border(cx))
        .bg(Theme::global(cx).list)
        .overflow_hidden()
        .flex()
        .flex_col()
}

fn group_row(content: impl IntoElement, show_divider: bool, cx: &App) -> impl IntoElement {
    div()
        .min_h(px(FIELD_ROW_MIN_HEIGHT))
        .px(theme::space_3())
        .py(px(6.0))
        .when(show_divider, |el| {
            el.border_b_1().border_color(Theme::global(cx).border)
        })
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

fn group_field(
    label: &str,
    input: &Entity<TextInput>,
    required: bool,
    cx: &App,
) -> impl IntoElement {
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
        .child(field_label(&label_text, cx))
        .child(input_slot(input, FIELD_INPUT_HEIGHT))
}

fn group_field_multiline(
    label: &str,
    input: &Entity<TextInput>,
    required: bool,
    cx: &App,
) -> impl IntoElement {
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
        .child(field_label(&label_text, cx).pt(px(6.0)))
        .child(input_slot(input, FIELD_TEXTAREA_HEIGHT))
}

type FormSetBoolFn = fn(&mut super::SshView, bool, &mut Context<super::SshView>);

fn bool_toggle_field(
    handle: &Entity<super::SshView>,
    id_prefix: &'static str,
    label: &str,
    hint: &str,
    enabled: bool,
    cx: &App,
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
                        .text_color(ui::text_secondary(cx))
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(ui::text_tertiary(cx))
                        .child(hint.to_string()),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .justify_end()
                .child(bool_toggle(handle, id_prefix, enabled, cx, set_value)),
        )
}

fn bool_toggle(
    handle: &Entity<super::SshView>,
    id_prefix: &'static str,
    enabled: bool,
    cx: &App,
    set_value: FormSetBoolFn,
) -> impl IntoElement {
    let h_on = handle.clone();
    let h_off = handle.clone();
    segmented_control(cx)
        .w(px(120.0))
        .flex_shrink_0()
        .child(segment_btn(
            id_prefix,
            "开启",
            0,
            enabled,
            cx,
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                h_on.update(cx, |v, cx| set_value(v, true, cx));
            },
        ))
        .child(segment_btn(
            id_prefix,
            "关闭",
            1,
            !enabled,
            cx,
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                h_off.update(cx, |v, cx| set_value(v, false, cx));
            },
        ))
}

fn field_label(text: &str, cx: &App) -> Div {
    div()
        .w(px(FIELD_LABEL_WIDTH))
        .flex_shrink_0()
        .text_size(theme::font_size_body())
        .text_color(ui::text_secondary(cx))
        .child(text.to_string())
}

fn segmented_control(cx: &App) -> Div {
    div()
        .p(px(2.0))
        .rounded(theme::radius_md())
        .bg(if Theme::global(cx).is_dark() {
            hsla(0.0, 0.0, 0.0, 0.22)
        } else {
            theme::rgba_with_alpha(Theme::global(cx).muted.into(), 0.85)
        })
        .flex()
        .gap(px(1.0))
}

fn segment_btn(
    id_prefix: &'static str,
    label: &'static str,
    idx: u64,
    selected: bool,
    cx: &App,
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
            if Theme::global(cx).is_dark() {
                hsla(0.0, 0.0, 0.35, 0.85)
            } else {
                gpui::hsla(0., 0., 1., 1.)
            }
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .text_color(if selected {
            ui::text_primary(cx)
        } else {
            ui::text_secondary(cx)
        })
        .shadow(if selected && !Theme::global(cx).is_dark() {
            vec![gpui::BoxShadow {
                color: theme::rgba_with_alpha(Theme::global(cx).border.into(), 0.08),
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
    cx: &App,
) -> impl IntoElement {
    segmented_control(cx)
        .child(proto_segment(
            handle,
            "profile-protocol",
            "SSH",
            0,
            ProtocolType::Ssh,
            current,
            cx,
        ))
        .child(proto_segment(
            handle,
            "profile-protocol",
            "FTP",
            1,
            ProtocolType::Ftp,
            current,
            cx,
        ))
        .child(proto_segment(
            handle,
            "profile-protocol",
            "FTPS",
            2,
            ProtocolType::Ftps,
            current,
            cx,
        ))
}

fn proto_segment(
    handle: &Entity<super::SshView>,
    id_prefix: &'static str,
    label: &'static str,
    idx: u64,
    proto: ProtocolType,
    current: &ProtocolType,
    cx: &App,
) -> impl IntoElement {
    let selected = std::mem::discriminant(&proto) == std::mem::discriminant(current);
    let h = handle.clone();
    segment_btn(
        id_prefix,
        label,
        idx,
        selected,
        cx,
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            h.update(cx, |v, cx| v.set_form_protocol(proto.clone(), cx));
        },
    )
}

fn render_auth_selector(
    handle: &Entity<super::SshView>,
    current: &SshAuthMethod,
    username_input: &Entity<TextInput>,
    password_input: &Entity<TextInput>,
    key_path: &Entity<TextInput>,
    key_passphrase: &Entity<TextInput>,
    cx: &App,
) -> impl IntoElement {
    let is_password = matches!(current, SshAuthMethod::Password { .. });
    let is_key = matches!(current, SshAuthMethod::PrivateKey { .. });
    let is_agent = matches!(current, SshAuthMethod::Agent);

    settings_group(cx)
        .child(
            div().p(theme::space_2()).child(
                segmented_control(cx)
                    .child(auth_segment(
                        handle,
                        "profile-auth",
                        "密码",
                        0,
                        is_password,
                        SshAuthMethod::Password {
                            password: String::new(),
                        },
                        cx,
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
                        cx,
                    ))
                    .child(auth_segment(
                        handle,
                        "profile-auth",
                        "Agent",
                        2,
                        is_agent,
                        SshAuthMethod::Agent,
                        cx,
                    )),
            ),
        )
        .when(is_password, |el| {
            el.child(group_row(
                group_field("用户名", username_input, false, cx),
                true,
                cx,
            ))
            .child(group_row(
                group_field("密码", password_input, false, cx),
                false,
                cx,
            ))
        })
        .when(is_key, |el| {
            el.child(group_row(
                group_field("用户名", username_input, false, cx),
                true,
                cx,
            ))
            .child(group_row(
                group_field("私钥路径", key_path, false, cx),
                true,
                cx,
            ))
            .child(group_row(
                group_field("私钥密码", key_passphrase, false, cx),
                false,
                cx,
            ))
        })
        .when(is_agent, |el| {
            el.child(group_row(
                group_field("用户名", username_input, false, cx),
                true,
                cx,
            ))
            .child(
                div()
                    .px(theme::space_3())
                    .py(theme::space_2())
                    .text_size(theme::font_size_caption())
                    .text_color(ui::text_tertiary(cx))
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
    cx: &App,
) -> impl IntoElement {
    let h = handle.clone();
    segment_btn(
        id_prefix,
        label,
        idx,
        selected,
        cx,
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            h.update(cx, |v, cx| v.set_form_auth_method(method.clone(), cx));
        },
    )
}

fn render_footer(handle: &Entity<super::SshView>, is_edit: bool, cx: &App) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .h(px(44.0))
        .flex()
        .items_center()
        .justify_between()
        .px(theme::space_4())
        .border_t_1()
        .border_color(glass::divider(cx))
        .bg(Theme::global(cx).popover)
        .child(if is_edit {
            let h = handle.clone();
            ui::ghost_btn("btn-delete-profile", "删除连接")
                .text_color(ui::danger(cx))
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
                    ui::secondary_btn("btn-cancel", "取消")
                        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            h_cancel.update(cx, |v, cx| v.close_profile_editor(cx));
                        }),
                )
                .child(
                    ui::primary_btn("btn-save-profile", "保存")
                        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            h_save.update(cx, |v, cx| v.save_profile_from_form(cx));
                        }),
                )
        })
}
