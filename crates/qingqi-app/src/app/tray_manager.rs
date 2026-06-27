use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Context, Global, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Subscription, Task, Window, WindowBackgroundAppearance,
    WindowBounds, WindowDecorations, WindowKind, WindowOptions, div, hsla, px, size,
};
use qingqi_core::{lock_or_recover, plugin::PluginManager};
use qingqi_platform::tray as platform_tray;
use qingqi_plugin::{
    plugin::PluginId,
    tray::{
        TrayHost, TrayItemIcon, TrayItemId, TrayItemRect, TrayItemSpec, TrayPopupOptions,
        TrayPopupView,
    },
};

#[derive(Clone)]
pub struct TrayManagerHandle(pub Arc<Mutex<TrayManager>>);

impl TrayManagerHandle {
    pub fn new(manager: TrayManager) -> Self {
        Self(Arc::new(Mutex::new(manager)))
    }
}

impl Global for TrayManagerHandle {}

pub struct TrayHostAdapter {
    manager: TrayManagerHandle,
}

impl TrayHostAdapter {
    pub fn new(manager: TrayManagerHandle) -> Self {
        Self { manager }
    }
}

impl TrayHost for TrayHostAdapter {
    fn register_tray_item(&self, plugin_id: &PluginId, spec: TrayItemSpec) -> anyhow::Result<()> {
        lock_or_recover(&self.manager.0, "tray-manager").register_item(plugin_id, spec)
    }

    fn update_tray_item(&self, plugin_id: &PluginId, spec: TrayItemSpec) -> anyhow::Result<()> {
        lock_or_recover(&self.manager.0, "tray-manager").update_item(plugin_id, spec)
    }

    fn remove_tray_item(&self, plugin_id: &PluginId, item_id: &TrayItemId) -> anyhow::Result<()> {
        lock_or_recover(&self.manager.0, "tray-manager").remove_item(plugin_id, item_id);
        Ok(())
    }

    fn open_tray_popup(
        &self,
        plugin_id: &PluginId,
        item_id: &TrayItemId,
        rect: TrayItemRect,
        options: TrayPopupOptions,
        view: Box<dyn TrayPopupView>,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        TrayManager::open_popup(
            self.manager.clone(),
            plugin_id.clone(),
            item_id.clone(),
            rect,
            options,
            view,
            cx,
        )
    }

    fn close_tray_popup(
        &self,
        plugin_id: &PluginId,
        item_id: &TrayItemId,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        TrayManager::close_popup(self.manager.clone(), plugin_id.as_ref(), item_id, cx)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RoutedTrayItemId {
    plugin_id: String,
    item_id: TrayItemId,
}

struct PendingTrayCallback {
    plugin_manager: Arc<Mutex<PluginManager>>,
    route: RoutedTrayItemId,
    event: TrayCallbackEvent,
}

enum TrayCallbackEvent {
    ItemClick(TrayItemRect),
    PopupClosed,
}

pub struct TrayManager {
    popup_windows: HashMap<platform_tray::TrayItemId, AnyWindowHandle>,
    item_routes: HashMap<platform_tray::TrayItemId, RoutedTrayItemId>,
    specs: HashMap<platform_tray::TrayItemId, TrayItemSpec>,
    plugin_manager: Option<Arc<Mutex<PluginManager>>>,
    last_click: Option<(platform_tray::TrayItemId, Instant)>,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            popup_windows: HashMap::new(),
            item_routes: HashMap::new(),
            specs: HashMap::new(),
            plugin_manager: None,
            last_click: None,
        }
    }

    pub fn set_plugin_manager(&mut self, plugin_manager: Arc<Mutex<PluginManager>>) {
        self.plugin_manager = Some(plugin_manager);
    }

    fn register_item(&mut self, plugin_id: &PluginId, spec: TrayItemSpec) -> anyhow::Result<()> {
        let platform_id = platform_id(plugin_id.as_ref(), &spec.id);
        let platform_spec = platform_spec(plugin_id.as_ref(), &spec);
        platform_tray::register_item(platform_spec)
            .map_err(|error| anyhow::anyhow!("tray item registration failed: {error}"))?;
        self.item_routes.insert(
            platform_id.clone(),
            RoutedTrayItemId {
                plugin_id: plugin_id.to_string(),
                item_id: spec.id.clone(),
            },
        );
        self.specs.insert(platform_id, spec);
        Ok(())
    }

    fn update_item(&mut self, plugin_id: &PluginId, spec: TrayItemSpec) -> anyhow::Result<()> {
        let platform_id = platform_id(plugin_id.as_ref(), &spec.id);
        let platform_spec = platform_spec(plugin_id.as_ref(), &spec);
        platform_tray::update_item(platform_spec)
            .map_err(|error| anyhow::anyhow!("tray item update failed: {error}"))?;
        self.item_routes.insert(
            platform_id.clone(),
            RoutedTrayItemId {
                plugin_id: plugin_id.to_string(),
                item_id: spec.id.clone(),
            },
        );
        self.specs.insert(platform_id, spec);
        Ok(())
    }

    fn remove_item(&mut self, plugin_id: &PluginId, item_id: &TrayItemId) {
        let platform_id = platform_id(plugin_id.as_ref(), item_id);
        platform_tray::remove_item(&platform_id);
        self.item_routes.remove(&platform_id);
        self.specs.remove(&platform_id);
        self.popup_windows.remove(&platform_id);
    }

    pub fn handle_item_click(
        handle: TrayManagerHandle,
        click: platform_tray::TrayItemClick,
        cx: &mut App,
    ) {
        let callback = {
            let mut manager = lock_or_recover(&handle.0, "tray-manager");
            if manager.is_duplicate_click(&click.id) {
                return;
            }
            let Some(route) = manager.item_routes.get(&click.id).cloned() else {
                tracing::warn!(item = click.id.as_str(), "tray item route not found");
                return;
            };
            let Some(plugin_manager) = manager.plugin_manager.clone() else {
                tracing::warn!("tray item click skipped: plugin manager not attached");
                return;
            };
            Some(PendingTrayCallback {
                plugin_manager,
                route,
                event: TrayCallbackEvent::ItemClick(TrayItemRect {
                    x: click.rect.x,
                    y: click.rect.y,
                    width: click.rect.width,
                    height: click.rect.height,
                }),
            })
        };
        if let Some(callback) = callback {
            cx.defer(move |cx| dispatch_tray_callback(callback, cx));
        }
    }

    fn is_duplicate_click(&mut self, id: &platform_tray::TrayItemId) -> bool {
        const DUPLICATE_CLICK_WINDOW: Duration = Duration::from_millis(90);
        let now = Instant::now();
        let duplicate = self.last_click.as_ref().is_some_and(|(last_id, last_at)| {
            last_id == id && now.saturating_duration_since(*last_at) <= DUPLICATE_CLICK_WINDOW
        });
        self.last_click = Some((id.clone(), now));
        duplicate
    }

    fn open_popup(
        handle: TrayManagerHandle,
        plugin_id: PluginId,
        item_id: TrayItemId,
        rect: TrayItemRect,
        options: TrayPopupOptions,
        view: Box<dyn TrayPopupView>,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        let platform_id = platform_id(plugin_id.as_ref(), &item_id);
        if let Some(existing) = {
            lock_or_recover(&handle.0, "tray-manager")
                .popup_windows
                .get(&platform_id)
                .copied()
        } {
            let _ = close_popup_window(existing, cx);
            schedule_popup_closed_callback(&handle, &platform_id, cx);
            return Ok(());
        }

        let (display, bounds) = popup_bounds(cx, rect, options.width, options.height);
        let title = view.title().to_string();
        let window_options = WindowOptions {
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
        let handle_for_window = handle.clone();
        let platform_id_for_window = platform_id.clone();
        let window_handle = cx.open_window(window_options, move |window, cx| {
            window.set_window_title(&title);
            cx.new(|cx| {
                TrayPopupWindow::new(
                    platform_id_for_window,
                    handle_for_window,
                    view,
                    options.close_on_deactivate,
                    window,
                    cx,
                )
            })
        })?;
        let _ = window_handle.update(cx, |_, window, cx| {
            cx.activate(true);
            window.activate_window();
        });
        lock_or_recover(&handle.0, "tray-manager")
            .popup_windows
            .insert(platform_id, window_handle.into());
        Ok(())
    }

    fn close_popup(
        handle: TrayManagerHandle,
        plugin_id: &str,
        item_id: &TrayItemId,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        let platform_id = platform_id(plugin_id, item_id);
        let Some(window_handle) = lock_or_recover(&handle.0, "tray-manager")
            .popup_windows
            .remove(&platform_id)
        else {
            return Ok(());
        };
        close_popup_window(window_handle, cx)
    }

    fn clear_popup(&mut self, id: &platform_tray::TrayItemId) -> bool {
        self.popup_windows.remove(id).is_some()
    }
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}

fn platform_id(plugin_id: &str, item_id: &TrayItemId) -> platform_tray::TrayItemId {
    platform_tray::TrayItemId::new(format!("{plugin_id}/{}", item_id.as_str()))
}

fn platform_spec(plugin_id: &str, spec: &TrayItemSpec) -> platform_tray::TrayItemSpec {
    platform_tray::TrayItemSpec {
        id: platform_id(plugin_id, &spec.id),
        icon: match spec.icon {
            TrayItemIcon::None => platform_tray::TrayItemIcon::None,
            TrayItemIcon::Default => platform_tray::TrayItemIcon::Default,
        },
        title: spec.title.clone(),
        tooltip: spec.tooltip.clone(),
        menu: Vec::new(),
        priority: spec.priority,
        visible: spec.visible,
    }
}

struct TrayPopupWindow {
    platform_id: platform_tray::TrayItemId,
    manager: TrayManagerHandle,
    view: Box<dyn TrayPopupView>,
    activation_subscription: Option<Subscription>,
    _update_task: Option<Task<()>>,
}

impl TrayPopupWindow {
    #[allow(clippy::too_many_arguments)]
    fn new(
        platform_id: platform_tray::TrayItemId,
        manager: TrayManagerHandle,
        mut view: Box<dyn TrayPopupView>,
        close_on_deactivate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let activation_subscription = if close_on_deactivate {
            let platform_id_for_activation = platform_id.clone();
            let manager_for_activation = manager.clone();
            Some(cx.observe_window_activation(window, move |_, window, cx| {
                if window.is_window_active() {
                    return;
                }
                let platform_id = platform_id_for_activation.clone();
                let manager = manager_for_activation.clone();
                window.defer(cx, move |window, cx| {
                    window.remove_window();
                    schedule_popup_closed_callback(&manager, &platform_id, cx);
                });
            }))
        } else {
            None
        };

        let update_task = view.subscribe_updates().map(|receiver| {
            cx.spawn(async move |this, async_cx| {
                let receiver = Arc::new(Mutex::new(receiver));
                loop {
                    let receiver = Arc::clone(&receiver);
                    let event = async_cx
                        .background_executor()
                        .spawn(async move { recv_popup_update(receiver) })
                        .await;
                    if event.is_none() {
                        break;
                    }
                    if this.update(async_cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
        });

        Self {
            platform_id,
            manager,
            view,
            activation_subscription,
            _update_task: update_task,
        }
    }
}

fn recv_popup_update(receiver: Arc<Mutex<Receiver<()>>>) -> Option<()> {
    receiver.lock().ok()?.recv().ok()
}

impl Drop for TrayPopupWindow {
    fn drop(&mut self) {
        self.view.on_close();
    }
}

impl Render for TrayPopupWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let platform_id = self.platform_id.clone();
        let manager = self.manager.clone();
        let _keep_subscription_alive = &self.activation_subscription;
        let content = self.view.render(window, cx);
        div()
            .size_full()
            .bg(hsla(0.0, 0.0, 1.0, 0.04))
            .on_mouse_down_out(cx.listener(move |_, _, window, cx| {
                let manager = manager.clone();
                let platform_id = platform_id.clone();
                window.defer(cx, move |window, cx| {
                    window.remove_window();
                    schedule_popup_closed_callback(&manager, &platform_id, cx);
                });
            }))
            .child(content)
    }
}

fn schedule_popup_closed_callback(
    manager: &TrayManagerHandle,
    platform_id: &platform_tray::TrayItemId,
    cx: &mut App,
) {
    let callback = {
        let mut manager_guard = lock_or_recover(&manager.0, "tray-manager");
        if !manager_guard.clear_popup(platform_id) {
            return;
        }
        let Some(route) = manager_guard.item_routes.get(platform_id).cloned() else {
            return;
        };
        let Some(plugin_manager) = manager_guard.plugin_manager.clone() else {
            return;
        };
        PendingTrayCallback {
            plugin_manager,
            route,
            event: TrayCallbackEvent::PopupClosed,
        }
    };
    cx.defer(move |cx| dispatch_tray_callback(callback, cx));
}

fn dispatch_tray_callback(callback: PendingTrayCallback, cx: &mut App) {
    let PendingTrayCallback {
        plugin_manager,
        route,
        event,
    } = callback;
    let result = match event {
        TrayCallbackEvent::ItemClick(rect) => lock_or_recover(&plugin_manager, "plugin-manager")
            .dispatch_tray_item_click(&route.plugin_id, &route.item_id, rect, cx),
        TrayCallbackEvent::PopupClosed => lock_or_recover(&plugin_manager, "plugin-manager")
            .dispatch_tray_popup_closed(&route.plugin_id, &route.item_id, cx),
    };
    if let Err(error) = result {
        tracing::warn!(
            plugin_id = %route.plugin_id,
            item = route.item_id.as_str(),
            error = %error,
            "tray callback failed"
        );
    }
}

fn popup_bounds(
    cx: &App,
    tray_rect: TrayItemRect,
    popup_width: u32,
    popup_height: u32,
) -> (
    Option<std::rc::Rc<dyn gpui::PlatformDisplay>>,
    Bounds<gpui::Pixels>,
) {
    let popup_width = popup_width.clamp(280, 520) as f32;
    let popup_height = popup_height.clamp(220, 640) as f32;
    let window_size = size(px(popup_width), px(popup_height));

    if let Some((display, origin)) = mouse_popup_origin(cx, window_size) {
        return (Some(display), Bounds::new(origin, window_size));
    }

    if tray_rect.width > 0.0
        && tray_rect.height > 0.0
        && let Some((display, origin)) = best_popup_origin_for_tray_rect(cx, tray_rect, window_size)
    {
        return (Some(display), Bounds::new(origin, window_size));
    }

    if let Some(display) = qingqi_platform::display::active_display(cx) {
        let bounds = display.bounds();
        let margin = px(10.0);
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

fn mouse_popup_origin(
    cx: &App,
    window_size: gpui::Size<gpui::Pixels>,
) -> Option<(
    std::rc::Rc<dyn gpui::PlatformDisplay>,
    gpui::Point<gpui::Pixels>,
)> {
    let (mouse, frame) = qingqi_platform::display::mouse_display_frame()?;
    let display = cx
        .displays()
        .into_iter()
        .find(|display| u32::from(display.id()) == frame.id)
        .or_else(|| qingqi_platform::display::active_display(cx))?;
    let local_x = mouse.x - frame.x;
    let local_y = mouse.y - frame.y;
    let bounds = display.bounds();
    let anchor = TrayAnchor {
        center_x: local_x,
        top: local_y,
        bottom: local_y + 22.0,
    };
    Some((
        display,
        popup_origin_for_anchor(anchor, bounds, window_size),
    ))
}

fn best_popup_origin_for_tray_rect(
    cx: &App,
    tray_rect: TrayItemRect,
    window_size: gpui::Size<gpui::Pixels>,
) -> Option<(
    std::rc::Rc<dyn gpui::PlatformDisplay>,
    gpui::Point<gpui::Pixels>,
)> {
    let mut best: Option<(
        f64,
        std::rc::Rc<dyn gpui::PlatformDisplay>,
        gpui::Point<gpui::Pixels>,
    )> = None;

    for display in cx.displays() {
        let bounds = display.bounds();
        let scale = display_scale_for_tray_rect(tray_rect, bounds);
        for anchor in candidate_anchors_for_display(tray_rect, scale, bounds) {
            let score = tray_anchor_score(anchor, bounds);
            let origin = popup_origin_for_anchor(anchor, bounds, window_size);
            if best
                .as_ref()
                .is_none_or(|(best_score, _, _)| score < *best_score)
            {
                best = Some((score, display.clone(), origin));
            }
        }
    }

    best.map(|(_, display, origin)| (display, origin))
}

#[derive(Clone, Copy, Debug)]
struct TrayAnchor {
    center_x: f64,
    top: f64,
    bottom: f64,
}

fn tray_anchor_for_rect(tray_rect: TrayItemRect) -> TrayAnchor {
    TrayAnchor {
        center_x: tray_rect.x + tray_rect.width / 2.0,
        top: tray_rect.y,
        bottom: tray_rect.y + tray_rect.height,
    }
}

fn display_scale_for_tray_rect(tray_rect: TrayItemRect, bounds: Bounds<gpui::Pixels>) -> f64 {
    let status_item_scale = tray_rect.height / 22.0;
    if (1.0..=3.0).contains(&status_item_scale) {
        return status_item_scale;
    }

    let logical_width: f64 = bounds.size.width.into();
    let physical_x = tray_rect.x + tray_rect.width / 2.0;
    if physical_x > logical_width * 2.25 {
        2.0
    } else if physical_x > logical_width * 1.25 {
        (physical_x / logical_width).clamp(1.0, 2.0)
    } else {
        1.0
    }
}

fn candidate_anchors_for_display(
    tray_rect: TrayItemRect,
    scale: f64,
    bounds: Bounds<gpui::Pixels>,
) -> [TrayAnchor; 2] {
    let raw = scaled_anchor(tray_rect, scale);
    let display_top: f64 = bounds.origin.y.into();
    let display_bottom: f64 = (bounds.origin.y + bounds.size.height).into();
    let flipped_top = display_top + (display_bottom - raw.bottom);
    let flipped_bottom = display_top + (display_bottom - raw.top);
    let flipped = TrayAnchor {
        center_x: raw.center_x,
        top: flipped_top.min(flipped_bottom),
        bottom: flipped_top.max(flipped_bottom),
    };
    [raw, flipped]
}

fn scaled_anchor(tray_rect: TrayItemRect, scale: f64) -> TrayAnchor {
    let scale = scale.max(1.0);
    let x = tray_rect.x / scale;
    let y = tray_rect.y / scale;
    let width = tray_rect.width / scale;
    let height = tray_rect.height / scale;
    TrayAnchor {
        center_x: x + width / 2.0,
        top: y,
        bottom: y + height,
    }
}

fn tray_anchor_overlaps_display(anchor: TrayAnchor, bounds: Bounds<gpui::Pixels>) -> bool {
    let left: f64 = bounds.origin.x.into();
    let top: f64 = bounds.origin.y.into();
    let right: f64 = (bounds.origin.x + bounds.size.width).into();
    let bottom: f64 = (bounds.origin.y + bounds.size.height).into();

    anchor.center_x >= left
        && anchor.center_x <= right
        && anchor.bottom >= top
        && anchor.top <= bottom
}

fn tray_anchor_distance(anchor: TrayAnchor, bounds: Bounds<gpui::Pixels>) -> f64 {
    let left: f64 = bounds.origin.x.into();
    let top: f64 = bounds.origin.y.into();
    let right: f64 = (bounds.origin.x + bounds.size.width).into();
    let bottom: f64 = (bounds.origin.y + bounds.size.height).into();
    let center_y = (anchor.top + anchor.bottom) / 2.0;

    let dx = if anchor.center_x < left {
        left - anchor.center_x
    } else if anchor.center_x > right {
        anchor.center_x - right
    } else {
        0.0
    };
    let dy = if center_y < top {
        top - center_y
    } else if center_y > bottom {
        center_y - bottom
    } else {
        0.0
    };
    dx * dx + dy * dy
}

fn tray_anchor_score(anchor: TrayAnchor, bounds: Bounds<gpui::Pixels>) -> f64 {
    let top: f64 = bounds.origin.y.into();
    let bottom: f64 = (bounds.origin.y + bounds.size.height).into();
    let height = (anchor.bottom - anchor.top).abs();
    let top_distance = (anchor.top - top).abs().min((anchor.bottom - top).abs());
    let bottom_distance = (anchor.top - bottom)
        .abs()
        .min((anchor.bottom - bottom).abs());
    let edge_distance = top_distance.min(bottom_distance);
    let menu_bar_height_penalty = if height > 80.0 { height - 80.0 } else { 0.0 };
    tray_anchor_distance(anchor, bounds) * 4.0 + edge_distance + menu_bar_height_penalty
}

fn popup_origin_for_anchor(
    anchor: TrayAnchor,
    bounds: Bounds<gpui::Pixels>,
    window_size: gpui::Size<gpui::Pixels>,
) -> gpui::Point<gpui::Pixels> {
    let margin = px(10.0);
    let center_x = px(anchor.center_x as f32);
    let menu_bottom = px(anchor.bottom.max(anchor.top) as f32);
    let x = (center_x - window_size.width / 2.0)
        .max(bounds.origin.x + margin)
        .min(bounds.origin.x + bounds.size.width - window_size.width - margin);
    let y = (menu_bottom + px(8.0))
        .max(bounds.origin.y + margin)
        .min(bounds.origin.y + bounds.size.height - window_size.height - margin);
    gpui::point(x, y)
}

fn close_popup_window(window_handle: AnyWindowHandle, cx: &mut App) -> Result<(), anyhow::Error> {
    if let Some(handle) = window_handle.downcast::<TrayPopupWindow>() {
        handle.update(cx, |_, window, _cx| {
            window.remove_window();
        })?;
        return Ok(());
    }
    anyhow::bail!("unexpected tray popup window root")
}

#[cfg(test)]
mod tests {
    use super::{
        TrayAnchor, popup_origin_for_anchor, tray_anchor_distance, tray_anchor_for_rect,
        tray_anchor_overlaps_display,
    };
    use gpui::{Bounds, point, px, size};
    use qingqi_plugin::tray::TrayItemRect;

    #[test]
    fn tray_anchor_uses_platform_rect_without_scaling_or_flipping() {
        let anchor = tray_anchor_for_rect(TrayItemRect {
            x: 1500.0,
            y: 0.0,
            width: 60.0,
            height: 24.0,
        });

        assert_eq!(anchor.center_x, 1530.0);
        assert_eq!(anchor.top, 0.0);
        assert_eq!(anchor.bottom, 24.0);
    }

    #[test]
    fn popup_origin_clamps_inside_secondary_display() {
        let display_bounds = Bounds::new(point(px(1440.0), px(0.0)), size(px(1280.0), px(900.0)));
        let window_size = size(px(340.0), px(360.0));
        let origin = popup_origin_for_anchor(
            TrayAnchor {
                center_x: 2650.0,
                top: 0.0,
                bottom: 24.0,
            },
            display_bounds,
            window_size,
        );

        assert_eq!(origin.x, px(2370.0));
        assert_eq!(origin.y, px(32.0));
    }

    #[test]
    fn tray_anchor_overlap_and_distance_select_secondary_display() {
        let primary = Bounds::new(point(px(0.0), px(0.0)), size(px(1440.0), px(900.0)));
        let secondary = Bounds::new(point(px(1440.0), px(0.0)), size(px(1280.0), px(900.0)));
        let anchor = TrayAnchor {
            center_x: 1700.0,
            top: 0.0,
            bottom: 24.0,
        };

        assert!(!tray_anchor_overlaps_display(anchor, primary));
        assert!(tray_anchor_overlaps_display(anchor, secondary));
        assert!(tray_anchor_distance(anchor, secondary) < tray_anchor_distance(anchor, primary));
    }
}
