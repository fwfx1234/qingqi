//! 新建连接 / 插件设置 — 独立子窗口

use gpui::prelude::*;
use gpui::*;
use gpui_component::Root;
use gpui_component::theme::Theme;

use super::SshView;
use super::app_settings;
use super::profile_editor;

const PROFILE_EDITOR_SIZE: (f32, f32) = (480.0, 560.0);
const APP_SETTINGS_SIZE: (f32, f32) = (380.0, 280.0);

pub fn dialog_window_options(
    title: impl Into<SharedString>,
    width: f32,
    height: f32,
    cx: &App,
) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(title.into()),
            ..Default::default()
        }),
        kind: WindowKind::Normal,
        is_resizable: true,
        is_minimizable: true,
        window_min_size: Some(size(px(width), px(height))),
        ..Default::default()
    }
}

pub fn close_window(handle: &mut Option<AnyWindowHandle>, cx: &mut App) {
    let Some(window_handle) = handle.take() else {
        return;
    };
    // 延迟到当前实体更新结束后再关窗，避免在子窗口点击回调里嵌套 update 失败。
    cx.defer(move |cx| {
        if let Err(error) = window_handle.update(cx, |_, window, _| window.remove_window()) {
            tracing::warn!(
                target: "qingqi_ssh",
                error = %error,
                "关闭子窗口失败"
            );
        }
    });
}

/// 延迟到当前 SshView 更新结束后再开窗，避免 render 时双重借用。
pub fn spawn_profile_editor_window(ssh_view: Entity<SshView>, is_edit: bool, cx: &mut App) {
    let options = profile_editor_window_options(is_edit, cx);
    let view_for_window = ssh_view.clone();
    match cx.open_window(options, move |window, cx| {
        let editor = cx.new(|cx| ProfileEditorWindow::new(view_for_window, window, cx));
        cx.new(|cx| Root::new(editor, window, cx))
    }) {
        Ok(handle) => {
            let _ = ssh_view.update(cx, |view, cx| {
                view.profile_editor_window = Some(handle.into());
                cx.notify();
            });
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
        Err(error) => {
            tracing::warn!(
                target: "qingqi_ssh",
                error = %error,
                "打开连接编辑窗口失败"
            );
        }
    }
}

pub fn spawn_app_settings_window(ssh_view: Entity<SshView>, cx: &mut App) {
    let options = app_settings_window_options(cx);
    let view_for_window = ssh_view.clone();
    match cx.open_window(options, move |window, cx| {
        let settings = cx.new(|cx| AppSettingsWindow::new(view_for_window, window, cx));
        cx.new(|cx| Root::new(settings, window, cx))
    }) {
        Ok(handle) => {
            let _ = ssh_view.update(cx, |view, cx| {
                view.app_settings_window = Some(handle.into());
                cx.notify();
            });
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
        Err(error) => {
            tracing::warn!(
                target: "qingqi_ssh",
                error = %error,
                "打开插件设置窗口失败"
            );
        }
    }
}

pub struct ProfileEditorWindow {
    ssh_view: Entity<SshView>,
    _observe: Subscription,
}

impl ProfileEditorWindow {
    pub fn new(ssh_view: Entity<SshView>, window: &Window, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&ssh_view, |_, _, cx| cx.notify());
        let view = ssh_view.clone();
        window.on_window_should_close(cx, move |window, cx| {
            let handle = window.window_handle();
            let _ = view.update(cx, |ssh, cx| {
                ssh.on_profile_editor_window_closed(handle, cx)
            });
            true
        });
        Self {
            ssh_view,
            _observe: observe,
        }
    }
}

impl Render for ProfileEditorWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.ssh_view.read(cx);
        let handle = self.ssh_view.clone();
        let inputs = profile_editor::ProfileFormInputs {
            name: view.form_name.clone(),
            host: view.form_host.clone(),
            port: view.form_port.clone(),
            username: view.form_username.clone(),
            password: view.form_password.clone(),
            remote_root: view.form_remote_root.clone(),
            local_root: view.form_local_root.clone(),
            private_key_path: view.form_private_key_path.clone(),
            private_key_passphrase: view.form_private_key_passphrase.clone(),
            note: view.form_note.clone(),
            connection_timeout: view.form_connection_timeout.clone(),
            keepalive_interval: view.form_keepalive_interval.clone(),
            keepalive_max: view.form_keepalive_max.clone(),
        };
        let advanced_flags = profile_editor::ProfileAdvancedFlags {
            tcp_nodelay: view.form_tcp_nodelay,
            ftp_passive_mode: view.form_ftp_passive_mode,
            ftp_passive_nat_workaround: view.form_ftp_passive_nat_workaround,
        };

        div()
            .size_full()
            .bg(theme_mode_bg(cx))
            .child(profile_editor::render_profile_editor_panel(
                handle,
                &inputs,
                &view.form_protocol,
                &view.form_auth_method,
                &advanced_flags,
                view.form_advanced_expanded,
                view.editing_profile_id.is_some(),
                cx,
            ))
    }
}

pub struct AppSettingsWindow {
    ssh_view: Entity<SshView>,
    _observe: Subscription,
}

impl AppSettingsWindow {
    pub fn new(ssh_view: Entity<SshView>, window: &Window, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&ssh_view, |_, _, cx| cx.notify());
        let view = ssh_view.clone();
        window.on_window_should_close(cx, move |window, cx| {
            let handle = window.window_handle();
            let _ = view.update(cx, |ssh, cx| ssh.on_app_settings_window_closed(handle, cx));
            true
        });
        Self {
            ssh_view,
            _observe: observe,
        }
    }
}

impl Render for AppSettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.ssh_view.read(cx);
        let handle = self.ssh_view.clone();
        let inputs = app_settings::AppSettingsInputs {
            terminal_font_size: view.form_terminal_font_size.clone(),
        };

        div()
            .size_full()
            .bg(theme_mode_bg(cx))
            .child(app_settings::render_app_settings_panel(
                handle,
                &inputs,
                view.terminal_font_size,
                cx,
            ))
    }
}

fn theme_mode_bg(cx: &App) -> Hsla {
    Theme::global(cx).list.into()
}

pub fn profile_editor_window_options(is_edit: bool, cx: &App) -> WindowOptions {
    let title = if is_edit {
        "编辑连接".to_string()
    } else {
        "新建连接".to_string()
    };
    dialog_window_options(title, PROFILE_EDITOR_SIZE.0, PROFILE_EDITOR_SIZE.1, cx)
}

pub fn app_settings_window_options(cx: &App) -> WindowOptions {
    dialog_window_options(
        "SSH 插件设置".to_string(),
        APP_SETTINGS_SIZE.0,
        APP_SETTINGS_SIZE.1,
        cx,
    )
}
