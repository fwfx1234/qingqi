use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, ClipboardItem, Context, Global, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, Styled, Subscription, Window,
    WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowKind, WindowOptions, div,
    hsla, px, size,
};
use qingqi_core::lock_or_recover;
use qingqi_platform::{
    network::{NetworkSampler, NetworkSnapshot, format_bytes, format_rate},
    tray::{TrayItemClick, TrayItemIcon, TrayItemId, TrayItemRect, TrayItemSpec},
    tray_settings::{NetworkSpeedTextMode, TraySettings, load_tray_settings},
};
use qingqi_ui::ui::glass;

#[derive(Clone)]
pub struct TrayManagerHandle(pub Arc<Mutex<TrayManager>>);

impl TrayManagerHandle {
    pub fn new(manager: TrayManager) -> Self {
        Self(Arc::new(Mutex::new(manager)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayPopupViewModel {
    pub title: String,
    pub subtitle: String,
    pub upload_rate: String,
    pub download_rate: String,
    pub rows: Vec<TrayPopupRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayPopupRow {
    pub label: String,
    pub value: String,
    pub copy_value: Option<String>,
}

pub trait TrayItemProvider: Send {
    fn spec(&self) -> TrayItemSpec;
    fn popup(&self) -> TrayPopupViewModel;
}

pub struct TrayManager {
    providers: HashMap<TrayItemId, Box<dyn TrayItemProvider>>,
    popup_windows: HashMap<TrayItemId, AnyWindowHandle>,
    last_specs: HashMap<TrayItemId, TrayItemSpec>,
    last_popups: HashMap<TrayItemId, TrayPopupViewModel>,
}

impl Global for TrayManagerHandle {}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            popup_windows: HashMap::new(),
            last_specs: HashMap::new(),
            last_popups: HashMap::new(),
        }
    }

    pub fn register_provider(&mut self, provider: Box<dyn TrayItemProvider>) {
        let spec = provider.spec();
        if let Err(error) = qingqi_platform::tray::register_item(spec.clone()) {
            tracing::warn!(
                item = spec.id.as_str(),
                error,
                "tray item registration failed"
            );
        }
        self.last_specs.insert(spec.id.clone(), spec.clone());
        self.last_popups.insert(spec.id.clone(), provider.popup());
        self.providers.insert(spec.id.clone(), provider);
    }

    pub fn update_provider(&mut self, provider: Box<dyn TrayItemProvider>, cx: &mut App) {
        let spec = provider.spec();
        let spec_changed = self.last_specs.get(&spec.id) != Some(&spec);
        if spec_changed {
            if let Err(error) = qingqi_platform::tray::update_item(spec.clone()) {
                tracing::warn!(item = spec.id.as_str(), error, "tray item update failed");
            }
            self.last_specs.insert(spec.id.clone(), spec.clone());
        }

        let popup = provider.popup();
        let popup_changed = self.last_popups.get(&spec.id) != Some(&popup);
        if popup_changed {
            self.last_popups.insert(spec.id.clone(), popup.clone());
        }

        if let Some(window_handle) = self.popup_windows.get(&spec.id).copied() {
            if popup_changed && let Some(handle) = window_handle.downcast::<TrayPopupWindow>() {
                if let Err(error) = handle.update(cx, |window_view, _, cx| {
                    window_view.popup = popup;
                    cx.notify();
                }) {
                    tracing::debug!(
                        item = spec.id.as_str(),
                        error = %error,
                        "tray popup live update skipped"
                    );
                }
            }
        }
        self.providers.insert(spec.id.clone(), provider);
    }

    pub fn handle_item_click(handle: TrayManagerHandle, click: TrayItemClick, cx: &mut App) {
        let id = click.id.clone();
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

        let popup_settings = qingqi_plugin::storage::AppPaths::resolve()
            .ok()
            .map(|paths| load_current_tray_settings(&paths))
            .unwrap_or_default();
        let (display, bounds) = popup_bounds(cx, click.rect, &popup, popup_settings.popup_width);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: display.map(|display| display.id()),
            titlebar: None,
            kind: WindowKind::PopUp,
            focus: true,
            is_movable: false,
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
            cx.new(|cx| TrayPopupWindow::new(id_for_window, handle_for_window, popup, window, cx))
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
    settings: TraySettings,
    public_ip: Option<String>,
}

impl NetworkSpeedProvider {
    pub const ID: &'static str = "network-speed";

    pub fn new(snapshot: NetworkSnapshot, settings: TraySettings) -> Self {
        Self::new_with_public_ip(snapshot, settings, None)
    }

    pub fn new_with_public_ip(
        snapshot: NetworkSnapshot,
        settings: TraySettings,
        public_ip: Option<String>,
    ) -> Self {
        Self {
            snapshot,
            settings,
            public_ip,
        }
    }

    pub fn sampling_interval() -> Duration {
        TraySettings::default().network_speed_update_interval()
    }

    pub fn sample(sampler: &mut NetworkSampler, settings: TraySettings) -> Self {
        Self::new(sampler.sample(), settings)
    }

    pub fn sample_with_public_ip(
        sampler: &mut NetworkSampler,
        settings: TraySettings,
        public_ip: Option<String>,
    ) -> Self {
        Self::new_with_public_ip(sampler.sample(), settings, public_ip)
    }

    fn title(&self) -> String {
        if !self.settings.effective_network_speed_show_text() {
            return String::new();
        }
        if !self.snapshot.ready {
            return String::from("采集中");
        }
        let down = format_menu_bar_rate(self.snapshot.received_per_sec);
        let up = format_menu_bar_rate(self.snapshot.transmitted_per_sec);
        match self.settings.network_speed_text_mode {
            NetworkSpeedTextMode::Both => fixed_width_menu_bar_rates(&up, &down),
            NetworkSpeedTextMode::DownloadOnly => format!("↓{down}"),
            NetworkSpeedTextMode::UploadOnly => format!("↑{up}"),
            NetworkSpeedTextMode::Dominant => {
                if self.snapshot.transmitted_per_sec > self.snapshot.received_per_sec {
                    format!("↑{up}")
                } else {
                    format!("↓{down}")
                }
            }
        }
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
            icon: if matches!(
                self.settings.network_speed_display_mode,
                qingqi_platform::tray_settings::NetworkSpeedDisplayMode::IconOnly
            ) {
                TrayItemIcon::Default
            } else {
                TrayItemIcon::None
            },
            title: self.title(),
            tooltip: self.tooltip(),
            menu: Vec::new(),
            priority: 10,
            visible: self.settings.network_speed_visible,
        }
    }

    fn popup(&self) -> TrayPopupViewModel {
        let mut rows = vec![TrayPopupRow {
            label: String::from("公网 IP"),
            value: self
                .public_ip
                .clone()
                .unwrap_or_else(|| String::from("获取中...")),
            copy_value: self.public_ip.clone(),
        }];

        if let Some(local_ip) = detect_local_ip() {
            rows.push(TrayPopupRow {
                label: String::from("内网 IP"),
                value: local_ip.clone(),
                copy_value: Some(local_ip),
            });
        }

        if self.settings.network_speed_show_totals {
            rows.push(TrayPopupRow {
                label: String::from("总接收"),
                value: format_bytes(self.snapshot.total_received, ""),
                copy_value: None,
            });
            rows.push(TrayPopupRow {
                label: String::from("总发送"),
                value: format_bytes(self.snapshot.total_transmitted, ""),
                copy_value: None,
            });
        }

        if self.settings.network_speed_show_interfaces {
            for interface in self
                .snapshot
                .interfaces
                .iter()
                .take(self.settings.network_speed_max_interfaces as usize)
            {
                rows.push(TrayPopupRow {
                    label: interface.name.clone(),
                    value: format!(
                        "↓{}  ↑{}",
                        format_rate(interface.received_per_sec),
                        format_rate(interface.transmitted_per_sec)
                    ),
                    copy_value: None,
                });
            }
        }

        TrayPopupViewModel {
            title: String::from("网速"),
            subtitle: if self.snapshot.ready {
                String::from("实时网络")
            } else {
                String::from("采集中...")
            },
            upload_rate: format_rate(self.snapshot.transmitted_per_sec),
            download_rate: format_rate(self.snapshot.received_per_sec),
            rows,
        }
    }
}

struct TrayPopupWindow {
    id: TrayItemId,
    manager: TrayManagerHandle,
    popup: TrayPopupViewModel,
    copied_row: Option<String>,
    activation_subscription: Option<Subscription>,
}

impl TrayPopupWindow {
    fn new(
        id: TrayItemId,
        manager: TrayManagerHandle,
        popup: TrayPopupViewModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let id_for_activation = id.clone();
        let manager_for_activation = manager.clone();
        let activation_subscription =
            Some(cx.observe_window_activation(window, move |_, window, cx| {
                if window.is_window_active() {
                    return;
                }
                let id = id_for_activation.clone();
                let manager = manager_for_activation.clone();
                window.defer(cx, move |window, _cx| {
                    window.remove_window();
                    lock_or_recover(&manager.0, "tray-manager").clear_popup(&id);
                });
            }));

        Self {
            id,
            manager,
            popup,
            copied_row: None,
            activation_subscription,
        }
    }
}

impl Render for TrayPopupWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.id.clone();
        let manager = self.manager.clone();
        let _keep_subscription_alive = &self.activation_subscription;
        let down = self.popup.download_rate.clone();
        let up = self.popup.upload_rate.clone();
        let window_fill = gpui::linear_gradient(
            145.0,
            gpui::linear_color_stop(hsla(0.0, 0.0, 1.0, 0.16), 0.0),
            gpui::linear_color_stop(hsla(210.0 / 360.0, 0.20, 0.96, 0.12), 1.0),
        );
        let glass_bg = gpui::linear_gradient(
            145.0,
            gpui::linear_color_stop(hsla(0.0, 0.0, 1.0, 0.62), 0.0),
            gpui::linear_color_stop(hsla(210.0 / 360.0, 0.18, 0.96, 0.50), 1.0),
        );
        let primary_text = hsla(215.0 / 360.0, 0.24, 0.14, 0.99);
        let secondary_text = hsla(215.0 / 360.0, 0.12, 0.34, 0.82);
        let muted_text = hsla(215.0 / 360.0, 0.10, 0.46, 0.68);
        let warm_border = hsla(0.0, 0.0, 1.0, 0.58);
        let warm_hairline = hsla(215.0 / 360.0, 0.12, 0.50, 0.09);
        let quiet_panel = hsla(0.0, 0.0, 1.0, 0.52);
        let live_bg = hsla(0.0, 0.0, 1.0, 0.48);
        div()
            .size_full()
            .bg(window_fill)
            .text_color(primary_text)
            .on_mouse_down_out(cx.listener(move |_, _, window, cx| {
                window.defer(cx, |window, _cx| window.remove_window());
                lock_or_recover(&manager.0, "tray-manager").clear_popup(&id);
            }))
            .child(
                div()
                    .size_full()
                    .rounded(px(18.0))
                    .bg(glass_bg)
                    .border_1()
                    .border_color(warm_border)
                    .shadow(glass::shadow())
                    .overflow_hidden()
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
                                    .gap_3()
                                    .mb_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .size(px(9.0))
                                                    .rounded(px(999.0))
                                                    .bg(hsla(156.0 / 360.0, 0.70, 0.52, 0.95))
                                                    .shadow(vec![gpui::BoxShadow {
                                                        color: hsla(
                                                            156.0 / 360.0,
                                                            0.70,
                                                            0.52,
                                                            0.24,
                                                        ),
                                                        offset: gpui::point(px(0.0), px(0.0)),
                                                        blur_radius: px(12.0),
                                                        spread_radius: px(2.0),
                                                    }]),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .text_color(muted_text)
                                                            .child(self.popup.title.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(15.0))
                                                            .font_weight(gpui::FontWeight::NORMAL)
                                                            .line_height(px(16.0))
                                                            .child(self.popup.subtitle.clone()),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h(px(26.0))
                                            .px_3()
                                            .rounded(px(999.0))
                                            .bg(live_bg)
                                            .border_1()
                                            .border_color(hsla(36.0 / 360.0, 0.82, 0.98, 0.28))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .font_family("monospace")
                                            .text_size(px(11.0))
                                            .text_color(muted_text)
                                            .child(div().size(px(6.0)).rounded(px(999.0)).bg(hsla(
                                                148.0 / 360.0,
                                                0.74,
                                                0.56,
                                                0.92,
                                            )))
                                            .child("LIVE"),
                                    ),
                            )
                            .child(
                                div()
                                    .grid()
                                    .grid_cols(2)
                                    .gap_2()
                                    .child(rate_card("上传", "↑", up))
                                    .child(rate_card("下载", "↓", down)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0()
                                    .rounded(px(18.0))
                                    .bg(quiet_panel)
                                    .border_1()
                                    .border_color(hsla(36.0 / 360.0, 0.82, 0.99, 0.30))
                                    .overflow_hidden()
                                    .children(self.popup.rows.iter().map(|row| {
                                        let copy_value = row.copy_value.clone();
                                        let row_label = row.label.clone();
                                        let is_copied =
                                            self.copied_row.as_deref() == Some(row.label.as_str());
                                        let display_value = if is_copied {
                                            String::from("已复制")
                                        } else {
                                            row.value.clone()
                                        };
                                        let mut row_el = div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .px_3()
                                            .py_2()
                                            .border_b_1()
                                            .border_color(warm_hairline)
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(secondary_text)
                                                    .child(row.label.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_family("monospace")
                                                    .font_weight(gpui::FontWeight::NORMAL)
                                                    .text_color(if is_copied {
                                                        hsla(156.0 / 360.0, 0.62, 0.36, 0.98)
                                                    } else {
                                                        primary_text
                                                    })
                                                    .child(display_value),
                                            );
                                        if let Some(copy_value) = copy_value {
                                            row_el = row_el
                                                .hover(|style| {
                                                    style.bg(hsla(36.0 / 360.0, 0.90, 0.99, 0.34))
                                                })
                                                .cursor_pointer()
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _window, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                copy_value.clone(),
                                                            ),
                                                        );
                                                        this.copied_row = Some(row_label.clone());
                                                        cx.notify();
                                                        let row_label = row_label.clone();
                                                        cx.spawn(async move |this, async_cx| {
                                                            async_cx
                                                                .background_executor()
                                                                .timer(Duration::from_millis(1200))
                                                                .await;
                                                            this.update(async_cx, |this, cx| {
                                                                if this.copied_row.as_deref()
                                                                    == Some(row_label.as_str())
                                                                {
                                                                    this.copied_row = None;
                                                                    cx.notify();
                                                                }
                                                            })
                                                            .ok();
                                                        })
                                                        .detach();
                                                    }),
                                                );
                                        }
                                        row_el
                                    })),
                            ),
                    ),
            )
    }
}

fn rate_card(label: &'static str, icon: &'static str, value: String) -> impl IntoElement {
    let primary_text = hsla(215.0 / 360.0, 0.24, 0.14, 0.98);
    let secondary_text = hsla(215.0 / 360.0, 0.12, 0.34, 0.78);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .rounded(px(18.0))
        .bg(gpui::linear_gradient(
            135.0,
            gpui::linear_color_stop(hsla(0.0, 0.0, 1.0, 0.58), 0.0),
            gpui::linear_color_stop(hsla(210.0 / 360.0, 0.14, 0.96, 0.44), 1.0),
        ))
        .border_1()
        .border_color(hsla(0.0, 0.0, 1.0, 0.56))
        .px_3()
        .py_2()
        .child(
            div().flex().items_center().justify_between().gap_2().child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(22.0))
                            .rounded(px(999.0))
                            .bg(hsla(0.0, 0.0, 1.0, 0.76))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::NORMAL)
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(secondary_text)
                            .child(label),
                    ),
            ),
        )
        .child(
            div()
                .font_family("monospace")
                .text_size(px(16.0))
                .line_height(px(17.0))
                .font_weight(gpui::FontWeight::NORMAL)
                .text_color(primary_text)
                .child(value),
        )
}

fn popup_bounds(
    cx: &App,
    tray_rect: TrayItemRect,
    popup: &TrayPopupViewModel,
    popup_width: u32,
) -> (
    Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    Bounds<gpui::Pixels>,
) {
    let popup_width = popup_width.clamp(280, 520) as f32;
    let popup_height = adaptive_popup_height(popup);
    let window_size = size(px(popup_width), px(popup_height));
    if tray_rect.width > 0.0 && tray_rect.height > 0.0 {
        let (display, anchor) = logical_anchor_for_tray_rect(cx, tray_rect);
        let (display, origin) = anchored_popup_origin(cx, display, anchor, window_size);
        return (display, Bounds::new(origin, window_size));
    }

    if let Some(display) = qingqi_platform::display::active_display(cx) {
        let bounds = display.bounds();
        let margin = px(18.0);
        let origin = gpui::point(
            bounds.origin.x + bounds.size.width - window_size.width - margin,
            bounds.origin.y + margin,
        );
        (Some(display), Bounds::new(origin, window_size))
    } else {
        (
            None,
            Bounds::centered_at(gpui::point(px(500.0), px(320.0)), window_size),
        )
    }
}

fn adaptive_popup_height(popup: &TrayPopupViewModel) -> f32 {
    let row_count = popup.rows.len().max(1) as f32;
    let content_height = 30.0 + 92.0 + row_count * 42.0 + 44.0;
    content_height.clamp(220.0, 520.0)
}

#[derive(Clone, Copy, Debug)]
struct TrayPopupAnchor {
    center_x: f64,
    top: f64,
    bottom: f64,
}

fn logical_anchor_for_tray_rect(
    cx: &App,
    tray_rect: TrayItemRect,
) -> (
    Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    TrayPopupAnchor,
) {
    let mut best_display = None;
    let mut best_anchor = scaled_anchor(tray_rect, 1.0);
    let mut best_score = f64::MAX;

    for display in cx.displays() {
        let bounds = display.bounds();
        for scale in [1.0, 2.0, 3.0] {
            for anchor in candidate_anchors_for_display(tray_rect, scale, bounds) {
                let score = menu_bar_anchor_score(anchor, bounds);
                if score < best_score {
                    best_score = score;
                    best_display = Some(display.clone());
                    best_anchor = anchor;
                }
            }
        }
    }

    if best_score.is_finite() {
        (best_display, best_anchor)
    } else {
        (None, best_anchor)
    }
}

fn candidate_anchors_for_display(
    tray_rect: TrayItemRect,
    scale: f64,
    bounds: Bounds<gpui::Pixels>,
) -> [TrayPopupAnchor; 2] {
    let raw = scaled_anchor(tray_rect, scale);
    let display_top: f64 = bounds.origin.y.into();
    let display_bottom: f64 = (bounds.origin.y + bounds.size.height).into();
    let flipped_top = display_top + (display_bottom - raw.bottom);
    let flipped_bottom = display_top + (display_bottom - raw.top);
    let flipped = TrayPopupAnchor {
        center_x: raw.center_x,
        top: flipped_top.min(flipped_bottom),
        bottom: flipped_top.max(flipped_bottom),
    };
    [raw, flipped]
}

fn scaled_anchor(tray_rect: TrayItemRect, scale: f64) -> TrayPopupAnchor {
    let x = tray_rect.x / scale;
    let y = tray_rect.y / scale;
    let width = tray_rect.width / scale;
    let height = tray_rect.height / scale;
    TrayPopupAnchor {
        center_x: x + width / 2.0,
        top: y,
        bottom: y + height,
    }
}

fn menu_bar_anchor_score(anchor: TrayPopupAnchor, bounds: Bounds<gpui::Pixels>) -> f64 {
    let left: f64 = bounds.origin.x.into();
    let top: f64 = bounds.origin.y.into();
    let right: f64 = (bounds.origin.x + bounds.size.width).into();
    let bottom: f64 = (bounds.origin.y + bounds.size.height).into();
    let height = (anchor.bottom - anchor.top).abs();
    let dx = if anchor.center_x < left {
        left - anchor.center_x
    } else if anchor.center_x > right {
        anchor.center_x - right
    } else {
        0.0
    };
    let dy = if anchor.bottom < top {
        top - anchor.bottom
    } else if anchor.top > bottom {
        anchor.top - bottom
    } else {
        0.0
    };
    let top_distance = (anchor.top - top).abs().min((anchor.bottom - top).abs());
    let menu_height_penalty = if height > 80.0 { height - 80.0 } else { 0.0 };
    dx * 4.0 + dy * 8.0 + top_distance + menu_height_penalty
}

fn anchored_popup_origin(
    cx: &App,
    display: Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    anchor: TrayPopupAnchor,
    window_size: gpui::Size<gpui::Pixels>,
) -> (
    Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    gpui::Point<gpui::Pixels>,
) {
    let display = display
        .or_else(|| display_for_anchor(cx, anchor))
        .or_else(|| qingqi_platform::display::active_display(cx));
    let Some(display) = display else {
        let window_width: f64 = window_size.width.into();
        return (
            None,
            gpui::point(
                px((anchor.center_x - window_width / 2.0) as f32),
                px(anchor.bottom as f32 + 8.0),
            ),
        );
    };

    let bounds = display.bounds();
    let margin = px(10.0);
    let center_x = px(anchor.center_x as f32);
    let menu_bottom = px(anchor.bottom.max(anchor.top) as f32);
    let mut y = menu_bottom + px(8.0);

    let x = (center_x - window_size.width / 2.0)
        .max(bounds.origin.x + margin)
        .min(bounds.origin.x + bounds.size.width - window_size.width - margin);
    y = y
        .max(bounds.origin.y + margin)
        .min(bounds.origin.y + bounds.size.height - window_size.height - margin);
    (Some(display), gpui::point(x, y))
}

fn display_for_anchor(
    cx: &App,
    anchor: TrayPopupAnchor,
) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    let anchor_x = px(anchor.center_x as f32);
    let anchor_y = px(anchor.bottom as f32);
    cx.displays().into_iter().find(|display| {
        let bounds = display.bounds();
        anchor_x >= bounds.origin.x
            && anchor_x <= bounds.origin.x + bounds.size.width
            && anchor_y >= bounds.origin.y
            && anchor_y <= bounds.origin.y + bounds.size.height
    })
}

fn close_popup_window(window_handle: AnyWindowHandle, cx: &mut App) -> Result<(), anyhow::Error> {
    if let Some(handle) = window_handle.downcast::<TrayPopupWindow>() {
        handle.update(cx, |_, window, cx| {
            window.defer(cx, |window, _cx| window.remove_window());
        })?;
        return Ok(());
    }
    anyhow::bail!("unexpected tray popup window root")
}

fn format_menu_bar_rate(bytes_per_sec: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes = bytes_per_sec as f64;
    if bytes >= GB {
        format!("{:.0}G", bytes / GB)
    } else if bytes >= 100.0 * MB {
        format!("{:.0}M", bytes / MB)
    } else if bytes >= 10.0 * MB {
        format!("{:.1}M", bytes / MB)
    } else if bytes >= MB {
        format!("{:.1}M", bytes / MB)
    } else if bytes >= 100.0 * KB {
        format!("{:.0}K", bytes / KB)
    } else if bytes >= KB {
        format!("{:.1}K", bytes / KB)
    } else {
        format!("{bytes:.0}B")
    }
}

fn fixed_width_menu_bar_rates(up: &str, down: &str) -> String {
    let width = up.chars().count().max(down.chars().count());
    let up = pad_menu_bar_rate(up, width);
    let down = pad_menu_bar_rate(down, width);
    let gap = "\u{202f}";
    format!("↑{gap}{up}\n↓{gap}{down}")
}

fn pad_menu_bar_rate(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(value.chars().count());
    format!("{}{}", "\u{00a0}".repeat(padding), value)
}

fn detect_local_ip() -> Option<String> {
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).ok()?;
    socket.connect(SocketAddr::from(([8, 8, 8, 8], 80))).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

pub fn load_current_tray_settings(paths: &qingqi_plugin::storage::AppPaths) -> TraySettings {
    load_tray_settings(&paths.config("tray.json"))
        .unwrap_or_default()
        .sanitized()
}
