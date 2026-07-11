use std::{
    net::{IpAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
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

pub type FetchPublicIpFn = Arc<dyn Fn() -> Option<String> + Send + Sync>;

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
    ip_generation: Arc<AtomicU64>,
    fetch_public_ip: FetchPublicIpFn,
}

impl NetworkSpeedService {
    pub fn new(plugin_id: PluginId, settings_store: NetworkSpeedSettingsStore) -> Self {
        Self::with_fetch(
            plugin_id,
            settings_store,
            Arc::new(fetch_public_ip_from_api),
        )
    }

    pub fn with_fetch(
        plugin_id: PluginId,
        settings_store: NetworkSpeedSettingsStore,
        fetch_public_ip: FetchPublicIpFn,
    ) -> Self {
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
            ip_generation: Arc::new(AtomicU64::new(0)),
            fetch_public_ip,
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
            if settings.public_ip_enabled {
                self.refresh_ip_cache_background();
            }
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
        if settings.public_ip_enabled {
            self.refresh_ip_cache_background();
        }
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
        let result = tray_host.close_tray_popup(&self.plugin_id, item_id, cx);
        let settings = self.settings();
        if !settings.public_ip_enabled {
            if let Ok(mut cached) = self.public_ip.lock() {
                *cached = None;
            }
        }
        result
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

    pub fn set_public_ip_enabled(&self, enabled: bool) -> Result<NetworkSpeedSettings> {
        let settings = self.save_and_refresh(|settings| settings.public_ip_enabled = enabled)?;
        self.ip_generation.fetch_add(1, Ordering::SeqCst);
        if !enabled {
            if let Ok(mut cached) = self.public_ip.lock() {
                *cached = None;
            }
        }
        Ok(settings)
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
        let public_ip_enabled = self.settings().public_ip_enabled;
        let generation = self.ip_generation.load(Ordering::SeqCst);
        let public_ip = Arc::clone(&self.public_ip);
        let local_ip = Arc::clone(&self.local_ip);
        let update_subscribers = Arc::clone(&self.update_subscribers);
        let ip_refreshing = Arc::clone(&self.ip_refreshing);
        let ip_generation = Arc::clone(&self.ip_generation);
        let fetch_public_ip = Arc::clone(&self.fetch_public_ip);
        std::thread::spawn(move || {
            let next_local_ip = detect_local_ip();
            if let Ok(mut current) = local_ip.lock() {
                *current = next_local_ip;
            }

            if public_ip_enabled && ip_generation.load(Ordering::SeqCst) == generation {
                let next_public_ip = fetch_public_ip();
                if ip_generation.load(Ordering::SeqCst) == generation {
                    if let Ok(mut current) = public_ip.lock() {
                        *current = next_public_ip;
                    }
                }
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use qingqi_plugin::{database::DatabaseService, dict_store::PluginDictStore};

    use crate::settings::NetworkSpeedSettingsStore;

    use super::*;

    fn temp_store() -> NetworkSpeedSettingsStore {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("qingqi-tray-test-{nanos}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let paths = qingqi_plugin::storage::AppPaths::for_test(dir);
        let database = DatabaseService::new(paths);
        let dict = PluginDictStore::for_database(database, "test.db");
        NetworkSpeedSettingsStore::new(dict)
    }

    fn counting_fetch(counter: &Arc<AtomicUsize>, result: Option<String>) -> FetchPublicIpFn {
        let counter = Arc::clone(counter);
        Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            result.clone()
        })
    }

    fn make_service(
        store: NetworkSpeedSettingsStore,
        fetch: FetchPublicIpFn,
    ) -> Arc<NetworkSpeedService> {
        let plugin_id: qingqi_plugin::plugin::PluginId = "test".into();
        Arc::new(NetworkSpeedService::with_fetch(plugin_id, store, fetch))
    }

    #[test]
    fn default_and_old_config_disable_public_ip() {
        let store = temp_store();

        let default_settings = crate::settings::NetworkSpeedSettings::default();
        assert!(!default_settings.public_ip_enabled);

        let loaded = store.load().expect("load settings");
        assert!(!loaded.public_ip_enabled);
    }

    #[test]
    fn public_ip_disabled_does_not_call_fetch() {
        let store = temp_store();
        let counter = Arc::new(AtomicUsize::new(0));
        let fetch = counting_fetch(&counter, Some("1.2.3.4".to_string()));
        let service = make_service(store, fetch);

        service.refresh_ip_cache_background();
        std::thread::sleep(Duration::from_millis(200));

        assert_eq!(counter.load(Ordering::SeqCst), 0);
        assert!(service.public_ip().is_none());
    }

    #[test]
    fn public_ip_enabled_triggers_fetch_once_and_respects_ip_refreshing() {
        let store = temp_store();
        store.set_public_ip_enabled(true).expect("enable public ip");

        let counter = Arc::new(AtomicUsize::new(0));
        let fetch = counting_fetch(&counter, Some("1.2.3.4".to_string()));
        let service = make_service(store, fetch);

        service.refresh_ip_cache_background();
        service.refresh_ip_cache_background();
        service.refresh_ip_cache_background();
        std::thread::sleep(Duration::from_millis(200));

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(service.public_ip(), Some("1.2.3.4".to_string()));
    }

    #[test]
    fn toggling_public_ip_enabled_clears_cache() {
        let store = temp_store();
        store.set_public_ip_enabled(true).expect("enable public ip");

        let counter = Arc::new(AtomicUsize::new(0));
        let fetch = counting_fetch(&counter, Some("5.6.7.8".to_string()));
        let service = make_service(store, fetch);

        service.refresh_ip_cache_background();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(service.public_ip(), Some("5.6.7.8".to_string()));

        service
            .set_public_ip_enabled(false)
            .expect("disable public ip");
        assert!(service.public_ip().is_none());
    }

    #[test]
    fn disabling_public_ip_discards_in_flight_result() {
        let store = temp_store();
        store.set_public_ip_enabled(true).expect("enable public ip");

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let fetch: FetchPublicIpFn = Arc::new(move || {
            started_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
            Some("9.8.7.6".to_string())
        });
        let service = make_service(store, fetch);

        service.refresh_ip_cache_background();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        service
            .set_public_ip_enabled(false)
            .expect("disable public ip");
        release_tx.send(()).unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while service.ip_refreshing.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(service.public_ip().is_none());
    }
}
