use std::{
    net::UdpSocket,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
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
}

/// 通过 HTTP 请求获取公网 IP。使用简单 TCP 连接避免引入 reqwest 依赖。
/// 使用 ip-api.com 免费的 JSON API（无需 API Key）。
fn fetch_public_ip_from_api() -> Option<String> {
    let request = "GET /json/ HTTP/1.1\r\nHost: ip-api.com\r\nConnection: close\r\n\r\n";
    let result = (|| -> std::io::Result<String> {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let mut stream = TcpStream::connect_timeout(
            &"ip-api.com:80"
                .parse()
                .unwrap_or_else(|_| ([93, 184, 216, 34], 80).into()),
            Duration::from_secs(5),
        )?;
        stream.write_all(request.as_bytes())?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        // 解析 JSON 响应: {"status":"success","query":"1.2.3.4"}
        if let Some(body_start) = response.find("\r\n\r\n") {
            let body = &response[body_start + 4..];
            if let Some(ip_start) = body.find("\"query\":\"") {
                let after = &body[ip_start + 9..];
                if let Some(ip_end) = after.find('\"') {
                    return Ok(after[..ip_end].to_string());
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "parse failed",
        ))
    })();
    result.ok()
}

fn detect_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}
