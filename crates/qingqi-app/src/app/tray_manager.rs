use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Context, Global, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowKind, WindowOptions, div, px, size,
};
use qingqi_core::lock_or_recover;
use qingqi_platform::{
    network::{NetworkSampler, NetworkSnapshot, format_bytes, format_rate},
    tray::{TrayItemIcon, TrayItemId, TrayItemSpec},
};
use qingqi_ui::ui;

#[derive(Clone)]
pub struct TrayManagerHandle(pub Arc<Mutex<TrayManager>>);

impl TrayManagerHandle {
    pub fn new(manager: TrayManager) -> Self {
        Self(Arc::new(Mutex::new(manager)))
    }
}

#[derive(Clone, Debug)]
pub struct TrayPopupViewModel {
    pub title: String,
    pub subtitle: String,
    pub rows: Vec<TrayPopupRow>,
}

#[derive(Clone, Debug)]
pub struct TrayPopupRow {
    pub label: String,
    pub value: String,
}

pub trait TrayItemProvider: Send {
    fn spec(&self) -> TrayItemSpec;
    fn popup(&self) -> TrayPopupViewModel;
}

pub struct TrayManager {
    providers: HashMap<TrayItemId, Box<dyn TrayItemProvider>>,
    popup_windows: HashMap<TrayItemId, AnyWindowHandle>,
}

impl Global for TrayManagerHandle {}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            popup_windows: HashMap::new(),
        }
    }

    pub fn register_provider(&mut self, provider: Box<dyn TrayItemProvider>) {
        let spec = provider.spec();
        if let Err(error) = qingqi_platform::tray::register_item(spec.clone()) {
            tracing::warn!(item = spec.id.as_str(), error, "tray item registration failed");
        }
        self.providers.insert(spec.id.clone(), provider);
    }

    pub fn update_provider(&mut self, provider: Box<dyn TrayItemProvider>) {
        let spec = provider.spec();
        if let Err(error) = qingqi_platform::tray::update_item(spec.clone()) {
            tracing::warn!(item = spec.id.as_str(), error, "tray item update failed");
        }
        self.providers.insert(spec.id.clone(), provider);
    }

    pub fn handle_item_click(handle: TrayManagerHandle, id: TrayItemId, cx: &mut App) {
        let existing = {
            lock_or_recover(&handle.0, "tray-manager")
                .popup_windows
                .get(&id)
                .copied()
        };
        if let Some(window_handle) = existing {
            if close_popup_window(window_handle, cx).is_ok() {
                lock_or_recover(&handle.0, "tray-manager")
                    .popup_windows
                    .remove(&id);
                return;
            }
            lock_or_recover(&handle.0, "tray-manager")
                .popup_windows
                .remove(&id);
        }

        let popup = {
            let manager = lock_or_recover(&handle.0, "tray-manager");
            let Some(provider) = manager.providers.get(&id) else {
                tracing::warn!(item = id.as_str(), "tray item provider not found");
                return;
            };
            provider.popup()
        };

        let bounds = popup_bounds(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(popup.title.clone().into()),
                appears_transparent: true,
                ..Default::default()
            }),
            kind: WindowKind::PopUp,
            is_movable: true,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Blurred,
            window_decorations: Some(WindowDecorations::Client),
            window_min_size: Some(bounds.size),
            ..Default::default()
        };
        let id_for_window = id.clone();
        let handle_for_window = handle.clone();
        match cx.open_window(options, move |window, cx| {
            window.set_window_title(&popup.title);
            cx.new(|_| TrayPopupWindow {
                id: id_for_window,
                manager: handle_for_window,
                popup,
            })
        }) {
            Ok(window_handle) => {
                let _ = window_handle.update(cx, |_, window, cx| {
                    cx.activate(true);
                    window.activate_window();
                });
                lock_or_recover(&handle.0, "tray-manager")
                    .popup_windows
                    .insert(id, window_handle.into());
            }
            Err(error) => tracing::warn!(error = %error, "open tray popup failed"),
        }
    }

    fn clear_popup(&mut self, id: &TrayItemId) {
        self.popup_windows.remove(id);
    }
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NetworkSpeedProvider {
    snapshot: NetworkSnapshot,
    updated_at_unix: u64,
}

impl NetworkSpeedProvider {
    pub const ID: &'static str = "network-speed";

    pub fn new(snapshot: NetworkSnapshot) -> Self {
        Self {
            snapshot,
            updated_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default(),
        }
    }

    pub fn sampling_interval() -> Duration {
        Duration::from_secs(1)
    }

    pub fn sample(sampler: &mut NetworkSampler) -> Self {
        Self::new(sampler.sample())
    }

    fn title(&self) -> String {
        if !self.snapshot.ready {
            return String::from("Net ...");
        }
        format!(
            "↓{} ↑{}",
            format_compact_rate(self.snapshot.received_per_sec),
            format_compact_rate(self.snapshot.transmitted_per_sec)
        )
    }

    fn tooltip(&self) -> String {
        if !self.snapshot.ready {
            return String::from("网速采集中...");
        }
        format!(
            "下载: {}\n上传: {}\n总接收: {}\n总发送: {}",
            format_rate(self.snapshot.received_per_sec),
            format_rate(self.snapshot.transmitted_per_sec),
            format_bytes(self.snapshot.total_received, ""),
            format_bytes(self.snapshot.total_transmitted, "")
        )
    }
}

impl TrayItemProvider for NetworkSpeedProvider {
    fn spec(&self) -> TrayItemSpec {
        TrayItemSpec {
            id: TrayItemId::new(Self::ID),
            icon: TrayItemIcon::Default,
            title: self.title(),
            tooltip: self.tooltip(),
            menu: Vec::new(),
            priority: 10,
            visible: true,
        }
    }

    fn popup(&self) -> TrayPopupViewModel {
        let mut rows = vec![
            TrayPopupRow {
                label: String::from("下载速度"),
                value: format_rate(self.snapshot.received_per_sec),
            },
            TrayPopupRow {
                label: String::from("上传速度"),
                value: format_rate(self.snapshot.transmitted_per_sec),
            },
            TrayPopupRow {
                label: String::from("总接收"),
                value: format_bytes(self.snapshot.total_received, ""),
            },
            TrayPopupRow {
                label: String::from("总发送"),
                value: format_bytes(self.snapshot.total_transmitted, ""),
            },
            TrayPopupRow {
                label: String::from("更新时间"),
                value: format!("{}s", self.updated_at_unix),
            },
        ];

        for interface in self.snapshot.interfaces.iter().take(5) {
            rows.push(TrayPopupRow {
                label: interface.name.clone(),
                value: format!(
                    "↓{}  ↑{}",
                    format_rate(interface.received_per_sec),
                    format_rate(interface.transmitted_per_sec)
                ),
            });
        }

        TrayPopupViewModel {
            title: String::from("网速"),
            subtitle: if self.snapshot.ready {
                self.title()
            } else {
                String::from("采集中...")
            },
            rows,
        }
    }
}

struct TrayPopupWindow {
    id: TrayItemId,
    manager: TrayManagerHandle,
    popup: TrayPopupViewModel,
}

impl Render for TrayPopupWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.id.clone();
        let manager = self.manager.clone();
        div()
            .size_full()
            .bg(ui::bg_surface(cx))
            .text_color(ui::text_primary(cx))
            .on_mouse_down_out(cx.listener(move |_, _, window, cx| {
                window.defer(cx, |window, _cx| window.remove_window());
                lock_or_recover(&manager.0, "tray-manager").clear_popup(&id);
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child(self.popup.title.clone()))
                            .child(
                                ui::window_close_button(cx),
                            ),
                    )
                    .child(div().text_sm().text_color(ui::text_secondary(cx)).child(self.popup.subtitle.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(self.popup.rows.iter().map(|row| {
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .py_1()
                                    .border_b_1()
                                    .border_color(ui::border_light(cx))
                                    .child(div().text_sm().text_color(ui::text_secondary(cx)).child(row.label.clone()))
                                    .child(div().text_sm().font_family("monospace").child(row.value.clone()))
                            })),
                    ),
            )
    }
}

fn popup_bounds(cx: &App) -> Bounds<gpui::Pixels> {
    let window_size = size(px(340.0), px(360.0));
    if let Some(display) = qingqi_platform::display::active_display(cx) {
        let bounds = display.bounds();
        let margin = px(18.0);
        let origin = gpui::point(
            bounds.origin.x + bounds.size.width - window_size.width - margin,
            bounds.origin.y + margin,
        );
        Bounds::new(origin, window_size)
    } else {
        Bounds::centered_at(gpui::point(px(500.0), px(320.0)), window_size)
    }
}

fn close_popup_window(
    window_handle: AnyWindowHandle,
    cx: &mut App,
) -> Result<(), anyhow::Error> {
    if let Some(handle) = window_handle.downcast::<TrayPopupWindow>() {
        handle.update(cx, |_, window, cx| {
            window.defer(cx, |window, _cx| window.remove_window());
        })?;
        return Ok(());
    }
    anyhow::bail!("unexpected tray popup window root")
}

fn format_compact_rate(bytes_per_sec: u64) -> String {
    let formatted = format_rate(bytes_per_sec);
    formatted.strip_suffix("/s").unwrap_or(&formatted).to_string()
}
