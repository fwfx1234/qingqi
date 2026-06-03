use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

use gpui::{
    AnyWindowHandle, App, AppContext, Context, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions, div, img, prelude::FluentBuilder, px,
};

use qingqi_plugin::{
    command::Command,
    log_error,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};
use qingqi_ui::ui;

use super::manifest;

pub struct AntiPeepingPlugin {
    paths: AppPaths,
    image_path: Rc<RefCell<Option<String>>>,
    active: Rc<RefCell<bool>>,
    overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
}

impl AntiPeepingPlugin {
    pub fn new(paths: AppPaths) -> Self {
        let image_path = Rc::new(RefCell::new(Self::load_custom_image(&paths)));
        Self {
            paths,
            image_path,
            active: Rc::new(RefCell::new(false)),
            overlay_windows: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn load_custom_image(paths: &AppPaths) -> Option<String> {
        let config_path = paths.config("anti-peeping.json");
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("image_path")?.as_str().map(String::from))
    }

    fn save_custom_image(paths: &AppPaths, path: &str) {
        let config_path = paths.config("anti-peeping.json");
        if let Some(parent) = config_path.parent() {
            log_error!(
                std::fs::create_dir_all(parent),
                warn,
                "创建防窥配置目录失败"
            );
        }
        let value = serde_json::json!({"image_path": path});
        log_error!(
            std::fs::write(
                &config_path,
                serde_json::to_string_pretty(&value).unwrap_or_default(),
            ),
            warn,
            "保存防窥配置失败"
        );
    }

    fn open_overlays(
        cx: &mut App,
        image_path: Rc<RefCell<Option<String>>>,
        overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
        active: Rc<RefCell<bool>>,
    ) {
        Self::close_overlays(cx, &overlay_windows);

        let displays: Vec<_> = cx.displays();
        for display in &displays {
            let bounds = display.bounds();
            let ip = Rc::clone(&image_path);
            let active = Rc::clone(&active);
            let handles = Rc::clone(&overlay_windows);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Fullscreen(bounds)),
                display_id: Some(display.id()),
                kind: WindowKind::PopUp,
                focus: true,
                show: true,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                titlebar: None,
                window_background: WindowBackgroundAppearance::Opaque,
                ..Default::default()
            };
            match cx.open_window(options, move |window, cx| {
                cx.new(|cx| {
                    let overlay = AntiPeepingOverlay::new(ip, active, handles, cx);
                    window.focus(&overlay.focus_handle);
                    overlay
                })
            }) {
                Ok(handle) => overlay_windows.borrow_mut().push(handle.into()),
                Err(error) => tracing::warn!(error = %error, "open anti-peeping overlay failed"),
            }
        }
    }

    fn close_overlays(cx: &mut App, overlay_windows: &Rc<RefCell<Vec<AnyWindowHandle>>>) {
        for handle in overlay_windows.borrow_mut().drain(..) {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }
}

impl Plugin for AntiPeepingPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        let m = self.manifest();
        vec![Command::plugin_open(
            m.id.as_ref(),
            "打开防窥屏",
            "全屏遮盖所有屏幕内容",
            m.keywords.iter().map(|s| s.as_ref()),
            m.command_prefixes.iter().map(|s| s.as_ref()),
            m.icon.as_str(),
        )]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        if !*self.active.borrow() {
            *self.active.borrow_mut() = true;
            let image_path = Rc::clone(&self.image_path);
            let overlay_windows = Rc::clone(&self.overlay_windows);
            let active = Rc::clone(&self.active);
            Self::open_overlays(cx.app, image_path, overlay_windows, active);
        }
        Ok(PluginView::Window(Box::new(AntiPeepingView {
            active: Rc::clone(&self.active),
            image_path: Rc::clone(&self.image_path),
            paths: self.paths.clone(),
            draft_path: self.image_path.borrow().clone().unwrap_or_default(),
        })))
    }

    fn close_idle(&mut self) {
        *self.active.borrow_mut() = false;
    }
}

struct AntiPeepingView {
    active: Rc<RefCell<bool>>,
    image_path: Rc<RefCell<Option<String>>>,
    paths: AppPaths,
    draft_path: String,
}

impl WindowView for AntiPeepingView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "防窥屏".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        let active = *self.active.borrow();
        let ip = Rc::clone(&self.image_path);
        let paths = self.paths.clone();
        let draft = self.draft_path.clone();

        div()
            .flex()
            .flex_col()
            .p(px(20.0))
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("防窥屏"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(ui::text_secondary())
                    .child(if active {
                        "已开启 — 按 Esc 键退出"
                    } else {
                        "已关闭"
                    }),
            )
            .child(div().h(px(1.0)).bg(ui::border_light()))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("自定义图片"),
            )
            .child(div().flex().gap(px(8.0)).items_center().child({
                let label = if draft.is_empty() {
                    "（使用纯黑色）".to_string()
                } else {
                    draft.clone()
                };
                div()
                    .flex_1()
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(ui::border_light())
                    .bg(ui::bg_subtle())
                    .text_size(px(12.0))
                    .child(label)
            }))
            .child({
                let ip_clone = Rc::clone(&ip);
                let paths_clone = paths.clone();
                let draft_clone = draft.clone();
                div()
                    .id("save-image-config")
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(ui::success())
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(gpui::white())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("保存设置")
                    .hover(|style| style.cursor_pointer())
                    .on_click(move |_event, _window, _cx| {
                        *ip_clone.borrow_mut() = if draft_clone.is_empty() {
                            None
                        } else {
                            Some(draft_clone.clone())
                        };
                        AntiPeepingPlugin::save_custom_image(&paths_clone, &draft_clone);
                    })
            })
            .into_any_element()
    }

    fn on_close(&mut self) {
        *self.active.borrow_mut() = false;
    }
}

/// Fullscreen overlay view — renders black or custom image, closes on Escape.
struct AntiPeepingOverlay {
    image_path: Rc<RefCell<Option<String>>>,
    active: Rc<RefCell<bool>>,
    overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
    focus_handle: FocusHandle,
}

impl AntiPeepingOverlay {
    fn new(
        image_path: Rc<RefCell<Option<String>>>,
        active: Rc<RefCell<bool>>,
        overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            image_path,
            active,
            overlay_windows,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for AntiPeepingOverlay {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let ip = self.image_path.borrow();
        let image_path = ip.clone();
        let focus_handle = self.focus_handle.clone();
        let active = Rc::clone(&self.active);
        let overlay_windows = Rc::clone(&self.overlay_windows);

        div()
            .size_full()
            .bg(gpui::black())
            .track_focus(&focus_handle)
            .on_key_down(move |event, _window, cx| {
                if event.keystroke.key == "escape" {
                    *active.borrow_mut() = false;
                    cx.stop_propagation();
                    let overlay_windows = Rc::clone(&overlay_windows);
                    cx.defer(move |cx| AntiPeepingPlugin::close_overlays(cx, &overlay_windows));
                }
            })
            .when_some(image_path, |this, path| {
                this.child(img(PathBuf::from(path)).size_full())
            })
    }
}
