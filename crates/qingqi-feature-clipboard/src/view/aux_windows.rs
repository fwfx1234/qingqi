//! 插件设置 — 独立子窗口

use gpui::prelude::*;
use gpui::*;
use gpui_component::scroll::ScrollableElement;
use gpui_component::Root;

use super::ClipboardView;
use super::settings;

const APP_SETTINGS_SIZE: (f32, f32) = (520.0, 560.0);

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
        window_background: WindowBackgroundAppearance::Opaque,
        window_min_size: Some(size(px(width), px(height))),
        window_decorations: Some(WindowDecorations::Server),
        ..Default::default()
    }
}

pub fn close_window(handle: &mut Option<AnyWindowHandle>, cx: &mut App) {
    let Some(window_handle) = handle.take() else {
        return;
    };
    cx.defer(move |cx| {
        if let Err(error) = window_handle.update(cx, |_, window, _| window.remove_window()) {
            tracing::warn!(
                target: "qingqi_clipboard",
                error = %error,
                "关闭设置窗口失败"
            );
        }
    });
}

pub fn spawn_app_settings_window(clipboard_view: Entity<ClipboardView>, cx: &mut App) {
    let options = app_settings_window_options(cx);
    let view_for_window = clipboard_view.clone();
    match cx.open_window(options, move |window, cx| {
        let settings = cx.new(|cx| AppSettingsWindow::new(view_for_window, window, cx));
        cx.new(|cx| Root::new(settings, window, cx))
    }) {
        Ok(handle) => {
            clipboard_view.update(cx, |view, cx| {
                view.app_settings_window = Some(handle.into());
                cx.notify();
            });
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
        Err(error) => {
            tracing::warn!(
                target: "qingqi_clipboard",
                error = %error,
                "打开剪贴板设置窗口失败"
            );
        }
    }
}

pub struct AppSettingsWindow {
    clipboard_view: Entity<ClipboardView>,
    _observe: Subscription,
}

impl AppSettingsWindow {
    pub fn new(
        clipboard_view: Entity<ClipboardView>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&clipboard_view, |_, _, cx| cx.notify());
        let view = clipboard_view.clone();
        window.on_window_should_close(cx, move |window, cx| {
            let handle = window.window_handle();
            view.update(cx, |clipboard, cx| {
                clipboard.on_app_settings_window_closed(handle, cx)
            });
            true
        });
        Self {
            clipboard_view,
            _observe: observe,
        }
    }
}

impl Render for AppSettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.clipboard_view.read(cx);
        let handle = self.clipboard_view.clone();
        let config = view.settings_snapshot();
        let dark = qingqi_ui::theme_mode::is_dark();

        let ignore_input = view.ignore_patterns_input.clone();
        let max_chars_input = view.max_text_chars_input.clone();
        let hotkey_input = view.hotkey_input.clone();

        let (Some(ignore_patterns_input), Some(max_text_chars_input), Some(hotkey_input)) =
            (ignore_input, max_chars_input, hotkey_input)
        else {
            return div()
                .size_full()
                .bg(qingqi_ui::theme::semantic().bg_elevated)
                .flex()
                .items_center()
                .justify_center()
                .text_color(qingqi_ui::theme::semantic().text_placeholder)
                .child("剪贴板设置加载中...")
                .into_any_element();
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(qingqi_ui::theme::semantic().bg_elevated)
            .child(settings_titlebar("剪贴板设置"))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .child(settings::settings_panel(
                        handle,
                        config,
                        ignore_patterns_input,
                        max_text_chars_input,
                        hotkey_input,
                        dark,
                    )),
            )
            .into_any_element()
    }
}

fn settings_titlebar(title: impl Into<SharedString>) -> impl IntoElement {
    let title = title.into();
    let s = qingqi_ui::theme::semantic();
    let is_macos = cfg!(target_os = "macos");

    div()
        .h(px(38.0))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .bg(qingqi_ui::theme::rgba_with_alpha(s.bg_surface, 0.72))
        .border_b_1()
        .border_color(qingqi_ui::ui::border_light())
        .child(qingqi_ui::ui::traffic_light::macos_traffic_lights())
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(s.text_primary)
                        .child(title),
                ),
        )
        .child(if is_macos {
            div()
                .w(px(86.0))
                .h_full()
                .flex_none()
                .into_any_element()
        } else {
            div()
                .w(px(40.0))
                .h_full()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(qingqi_ui::ui::window_close_button())
                .into_any_element()
        })
}

pub fn app_settings_window_options(cx: &App) -> WindowOptions {
    let window_size = size(px(APP_SETTINGS_SIZE.0), px(APP_SETTINGS_SIZE.1));
    let (display, bounds) =
        qingqi_platform::display::centered_on_active_display(cx, window_size);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display.map(|display| display.id()),
        titlebar: Some(TitlebarOptions {
            title: Some("剪贴板设置".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        kind: WindowKind::PopUp,
        show: false,
        is_resizable: true,
        window_background: WindowBackgroundAppearance::Opaque,
        window_min_size: Some(window_size),
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    }
}
