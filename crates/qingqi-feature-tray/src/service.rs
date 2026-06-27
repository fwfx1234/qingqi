use std::{
    net::{IpAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

use anyhow::Result;
use gpui::App;
use qingqi_platform::network::{NetworkSampler, NetworkSnapshot};
use qingqi_plugin::{
    plugin::PluginId,
    tray::{TrayHostRef, TrayItemId, TrayItemRect, TrayPopupOptions},
};

use crate::{
    model::{TRAY_ITEM_ID, tray_item_spec},
    settings::{
        NetworkSpeedDisplayMode, NetworkSpeedSettings, NetworkSpeedSettingsStore,
        NetworkSpeedTextMode,
    },
    view::NetworkSpeedPopupView,
};

pub struct NetworkSpeedService {
    plugin_id: PluginId,
    settings_store: NetworkSpeedSettingsStore,
    sampler: Mutex<NetworkSampler>,
    snapshot: Mutex<NetworkSnapshot>,
    public_ip: Arc<Mutex<Option<String>>>,
    local_ip: Arc<Mutex<Option<String>>>,
    update_subscribers: Arc<Mutex<Vec<Sender<()>>>>,
    tray_host: Mutex<Option<TrayHostRef>>,
    started: AtomicBool,
    ip_refreshing: Arc<AtomicBool>,
}

impl NetworkSpeedService {
    pub fn new(plugin_id: PluginId, settings_store: NetworkSpeedSettingsStore) -> Self {
        Self {
            plugin_id,
            settings_store,
            sampler: Mutex::new(NetworkSampler::new()),
            snapshot: Mutex::new(NetworkSnapshot::default()),
            public_ip: Arc::new(Mutex::new(None)),
            local_ip: Arc::new(Mutex::new(None)),
            update_subscribers: Arc::new(Mutex::new(Vec::new())),
            tray_host: Mutex::new(None),
            started: AtomicBool::new(false),
            ip_refreshing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn settings(&self) -> NetworkSpeedSettings {
        self.settings_store.settings()
    }

    pub fn snapshot(&self) -> NetworkSnapshot {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default()
    }

    pub fn public_ip(&self) -> Option<String> {
        self.public_ip.lock().ok().and_then(|ip| ip.clone())
    }

    pub fn local_ip(&self) -> Option<String> {
        self.local_ip.lock().ok().and_then(|ip| ip.clone())
    }

    pub fn subscribe_updates(&self) -> Receiver<()> {
        let (sender, receiver) = channel();
        if let Ok(mut subscribers) = self.update_subscribers.lock() {
            subscribers.push(sender);
        }
        receiver
    }

    pub fn start_background(self: &Arc<Self>, tray_host: TrayHostRef, cx: &mut App) -> Result<()> {
        self.attach_tray_host(tray_host.clone());
        let settings = self.settings();
        let snapshot = self.snapshot();
        tray_host.register_tray_item(&self.plugin_id, tray_item_spec(&settings, &snapshot))?;

        if !self.started.swap(true, Ordering::SeqCst) {
            self.refresh_ip_cache_background();
            Self::schedule_next_sample(
                Arc::clone(self),
                settings.network_speed_update_interval(),
                cx,
            );
        }
        Ok(())
    }

    pub fn open_popup(
        self: &Arc<Self>,
        item_id: &TrayItemId,
        rect: TrayItemRect,
        tray_host: TrayHostRef,
        cx: &mut App,
    ) -> Result<()> {
        self.attach_tray_host(tray_host.clone());
        let settings = self.settings();
        let snapshot = self.snapshot();
        let height = crate::model::popup_content_height(&settings, &snapshot);
        self.refresh_ip_cache_background();
        tray_host.open_tray_popup(
            &self.plugin_id,
            item_id,
            rect,
            TrayPopupOptions {
                width: settings.popup_width,
                height,
                close_on_deactivate: true,
            },
            Box::new(NetworkSpeedPopupView::new(Arc::clone(self))),
            cx,
        )
    }

    pub fn close_popup(
        &self,
        item_id: &TrayItemId,
        tray_host: TrayHostRef,
        cx: &mut App,
    ) -> Result<()> {
        tray_host.close_tray_popup(&self.plugin_id, item_id, cx)
    }

    pub fn set_network_speed_visible(&self, visible: bool) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| settings.network_speed_visible = visible)
    }

    pub fn set_network_speed_display_mode(
        &self,
        mode: NetworkSpeedDisplayMode,
    ) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| settings.network_speed_display_mode = mode)
    }

    pub fn set_network_speed_text_mode(
        &self,
        mode: NetworkSpeedTextMode,
    ) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| settings.network_speed_text_mode = mode)
    }

    pub fn set_network_speed_update_interval_ms(
        &self,
        interval_ms: u64,
    ) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| {
            settings.network_speed_update_interval_ms = interval_ms;
        })
    }

    pub fn set_popup_size(&self, width: u32, height: u32) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| {
            settings.popup_width = width;
            settings.popup_height = height;
        })
    }

    pub fn set_network_speed_show_totals(&self, show: bool) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| settings.network_speed_show_totals = show)
    }

    pub fn set_network_speed_show_interfaces(&self, show: bool) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| settings.network_speed_show_interfaces = show)
    }

    pub fn set_network_speed_max_interfaces(
        &self,
        max_interfaces: u8,
    ) -> Result<NetworkSpeedSettings> {
        self.save_and_refresh(|settings| settings.network_speed_max_interfaces = max_interfaces)
    }

    fn attach_tray_host(&self, tray_host: TrayHostRef) {
        if let Ok(mut current) = self.tray_host.lock() {
            *current = Some(tray_host);
        }
    }

    fn tray_host(&self) -> Option<TrayHostRef> {
        self.tray_host.lock().ok().and_then(|host| host.clone())
    }

    fn save_and_refresh(
        &self,
        apply: impl FnOnce(&mut NetworkSpeedSettings),
    ) -> Result<NetworkSpeedSettings> {
        let settings = self.settings_store.update(apply)?;
        self.refresh_tray_item(&settings, &self.snapshot());
        self.notify_updated();
        Ok(settings)
    }

    fn refresh_tray_item(&self, settings: &NetworkSpeedSettings, snapshot: &NetworkSnapshot) {
        let Some(tray_host) = self.tray_host() else {
            return;
        };
        if let Err(error) =
            tray_host.update_tray_item(&self.plugin_id, tray_item_spec(settings, snapshot))
        {
            tracing::warn!(error = %error, "network speed tray item update failed");
        }
    }

    fn refresh_ip_cache_background(self: &Arc<Self>) {
        if self.ip_refreshing.swap(true, Ordering::SeqCst) {
            return;
        }
        let public_ip = Arc::clone(&self.public_ip);
        let local_ip = Arc::clone(&self.local_ip);
        let update_subscribers = Arc::clone(&self.update_subscribers);
        let ip_refreshing = Arc::clone(&self.ip_refreshing);
        std::thread::spawn(move || {
            let next_local_ip = detect_local_ip();
            if let Ok(mut current) = local_ip.lock() {
                *current = next_local_ip;
            }

            let next_public_ip = fetch_public_ip_from_api();
            if let Ok(mut current) = public_ip.lock() {
                *current = next_public_ip;
            }
            notify_update_subscribers(&update_subscribers);
            ip_refreshing.store(false, Ordering::SeqCst);
        });
    }

    fn sample(&self) -> Result<(NetworkSpeedSettings, NetworkSnapshot)> {
        let snapshot = {
            let mut sampler = self
                .sampler
                .lock()
                .map_err(|_| anyhow::anyhow!("network sampler lock poisoned"))?;
            sampler.sample()
        };
        {
            let mut current = self
                .snapshot
                .lock()
                .map_err(|_| anyhow::anyhow!("network snapshot lock poisoned"))?;
            *current = snapshot.clone();
        }
        Ok((self.settings(), snapshot))
    }

    fn schedule_next_sample(service: Arc<Self>, interval: Duration, cx: &mut App) {
        cx.spawn(async move |async_cx| {
            async_cx.background_executor().timer(interval).await;

            let service_for_update = Arc::clone(&service);
            let _ = async_cx.update(move |cx| {
                let result = service_for_update.sample();
                match result {
                    Ok((settings, snapshot)) => {
                        service_for_update.refresh_tray_item(&settings, &snapshot);
                        service_for_update.notify_updated();
                        Self::schedule_next_sample(
                            Arc::clone(&service_for_update),
                            settings.network_speed_update_interval(),
                            cx,
                        );
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "network speed sample failed");
                        let interval = service_for_update
                            .settings()
                            .network_speed_update_interval();
                        Self::schedule_next_sample(Arc::clone(&service_for_update), interval, cx);
                    }
                }
            });
        })
        .detach();
    }

    pub fn tray_item_id() -> TrayItemId {
        TrayItemId::new(TRAY_ITEM_ID)
    }

    fn notify_updated(&self) {
        notify_update_subscribers(&self.update_subscribers);
    }
}

fn notify_update_subscribers(subscribers: &Arc<Mutex<Vec<Sender<()>>>>) {
    if let Ok(mut subscribers) = subscribers.lock() {
        subscribers.retain(|subscriber| subscriber.send(()).is_ok());
    }
}

/// 通过 api.ipify.org 获取公网 IP。
fn fetch_public_ip_from_api() -> Option<String> {
    let result = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent("qingqi/1.0")
        .build()
        .and_then(|client| client.get("https://api.ipify.org").send())
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text());

    match result {
        Ok(body) => parse_public_ip_body(&body),
        Err(error) => {
            tracing::warn!(error = %error, "public IP fetch failed");
            None
        }
    }
}

fn parse_public_ip_body(body: &str) -> Option<String> {
    let candidate = body.trim();
    candidate.parse::<IpAddr>().ok().map(|ip| ip.to_string())
}

fn detect_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}
