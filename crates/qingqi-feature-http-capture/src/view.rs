use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    breakpoint::{BreakpointPhase, BreakpointRule},
    certificate::CaManager,
    composer::{ComposedResponse, ComposedRequest},
    engine::CaptureEngine,
    manifest,
    mock_store::MockStore,
    model::{
        BodyDisplay, CaptureEndpoint, CaptureSetupInfo, CapturedExchange, CertificateStatus,
        DetailTab, FilterState,
    },
    performance,
    rewrite::{RewriteRule, RewriteTarget},
    session_tree::{self, FlatRow},
    store::CaptureStore,
    throttle::ThrottlePreset,
    video_sniff,
};
use gpui::{
    App, AppContext, ClipboardItem, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
    Window, div, px,
};
use qingqi_ui::components::button::{Button, ButtonVariant, ButtonVariants};
use qingqi_ui::components::styled::Disableable;
use qingqi_ui::components::styled::Sizable;
use qingqi_ui::components::styled::Size;

use qingqi_plugin::{
    events::{AppEventBus, AppEventKind},
    plugin_spec::PluginAccent,
};
use qingqi_ui::components::divider::Divider;
use qingqi_ui::components::input::{Input, InputState};
use qingqi_ui::components::scroll::ScrollableElement;
use qingqi_ui::components::scroll::ScrollbarExt;
use qingqi_ui::components::theme::Theme;
use qingqi_ui::token::tokens;
use qingqi_ui::{
    theme,
    ui::{self, components},
};

const PAGE_SIZE: i64 = 50;
const DEFAULT_PROXY_PORT: u16 = 8899;

pub struct CaptureView {
    store: Arc<Mutex<CaptureStore>>,
    engine: Arc<CaptureEngine>,
    ca_manager: Arc<Mutex<CaManager>>,
    search_input: Option<Entity<InputState>>,
    host_input: Option<Entity<InputState>>,
    filter: FilterState,
    exchanges: Vec<CapturedExchange>,
    total: i64,
    selected_id: Option<i64>,
    selected_detail: Option<CapturedExchange>,
    detail_tab: DetailTab,
    offset: i64,
    engine_running: bool,
    engine_port: u16,
    setup_info: CaptureSetupInfo,
    notice: Option<String>,
    loading: bool,
    load_generation: u64,
    reload_task: Option<Task<()>>,
    detail_task: Option<Task<()>>,
    event_task: Option<Task<()>>,
    subscriptions: Vec<Subscription>,
    // Composer state
    composer_visible: bool,
    composer_method: String,
    composer_url: Option<Entity<InputState>>,
    composer_headers: Option<Entity<InputState>>,
    composer_body: Option<Entity<InputState>>,
    composer_response: Option<ComposedResponse>,
    composer_sending: bool,
    // Tree view state
    tree_view_mode: bool,
    tree_rows: Vec<FlatRow>,
    tree_expanded: std::collections::HashSet<String>,
    tree_load_generation: u64,
    tree_task: Option<Task<()>>,
    // Performance panel state
    performance_visible: bool,
    performance_stats: Option<performance::PerformanceStats>,
    performance_task: Option<Task<()>>,
    performance_load_generation: u64,
    // Breakpoint panel state
    breakpoint_visible: bool,
    breakpoint_rules: Vec<BreakpointRule>,
    breakpoint_url_input: Option<Entity<InputState>>,
    breakpoint_method_input: Option<Entity<InputState>>,
    // Throttle panel state
    throttle_visible: bool,
    throttle_custom_input: Option<Entity<InputState>>,
    // Rewrite panel state
    rewrite_visible: bool,
    rewrite_rules: Vec<RewriteRule>,
    rewrite_name_input: Option<Entity<InputState>>,
    rewrite_url_input: Option<Entity<InputState>>,
}

impl CaptureView {
    pub fn new(
        store: Arc<Mutex<CaptureStore>>,
        engine: Arc<CaptureEngine>,
        _mock_store: Arc<Mutex<MockStore>>,
        ca_manager: Arc<Mutex<CaManager>>,
        events: AppEventBus,
        cx: &mut Context<Self>,
    ) -> Self {
        let setup_info = build_setup_info(&engine, &ca_manager, DEFAULT_PROXY_PORT);
        let mut this = Self {
            store,
            engine,
            ca_manager,
            search_input: None,
            host_input: None,
            filter: FilterState::default(),
            exchanges: Vec::new(),
            total: 0,
            selected_id: None,
            selected_detail: None,
            detail_tab: DetailTab::Overview,
            offset: 0,
            engine_running: setup_info.is_running(),
            engine_port: setup_info.port(),
            setup_info,
            notice: None,
            loading: false,
            load_generation: 0,
            reload_task: None,
            detail_task: None,
            event_task: None,
            subscriptions: Vec::new(),
            composer_visible: false,
            composer_method: "GET".to_string(),
            composer_url: None,
            composer_headers: None,
            composer_body: None,
            composer_response: None,
            composer_sending: false,
            tree_view_mode: false,
            tree_rows: Vec::new(),
            tree_expanded: std::collections::HashSet::new(),
            tree_load_generation: 0,
            tree_task: None,
            performance_visible: false,
            performance_stats: None,
            performance_task: None,
            performance_load_generation: 0,
            breakpoint_visible: false,
            breakpoint_rules: Vec::new(),
            breakpoint_url_input: None,
            breakpoint_method_input: None,
            throttle_visible: false,
            throttle_custom_input: None,
            rewrite_visible: false,
            rewrite_rules: Vec::new(),
            rewrite_name_input: None,
            rewrite_url_input: None,
        };
        this.start_event_watch(events, cx);
        this.refresh_from_store(cx);
        this
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_none() {
            self.search_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("搜索 URL 关键词")));
        }
        if self.host_input.is_none() {
            self.host_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Host 过滤")));
        }
        self.observe_inputs(cx);
    }

    fn ensure_composer_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer_url.is_none() {
            self.composer_url =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("https://example.com/api")));
        }
        if self.composer_headers.is_none() {
            self.composer_headers = Some(
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true)
                        .rows(4)
                        .placeholder("Content-Type: application/json\nAuthorization: Bearer token")
                }),
            );
        }
        if self.composer_body.is_none() {
            self.composer_body = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .rows(6)
                    .placeholder("{\"key\": \"value\"}")
            }));
        }
    }

    fn observe_inputs(&mut self, cx: &mut Context<Self>) {
        if !self.subscriptions.is_empty() {
            return;
        }

        let search = self.search_input.clone().expect("search input initialized");
        let sub = cx.observe(&search, |panel, search, cx| {
            panel.filter.search = search.read(cx).value().to_string();
            panel.offset = 0;
            panel.refresh_from_store(cx);
        });
        self.subscriptions.push(sub);

        let host = self.host_input.clone().expect("host input initialized");
        let sub = cx.observe(&host, |panel, host, cx| {
            panel.filter.host = host.read(cx).value().to_string();
            panel.offset = 0;
            panel.refresh_from_store(cx);
        });
        self.subscriptions.push(sub);
    }

    fn start_event_watch(&mut self, events: AppEventBus, cx: &mut Context<Self>) {
        if self.event_task.is_some() {
            return;
        }

        self.event_task = Some(cx.spawn(async move |panel, async_cx| {
            let receiver = Arc::new(Mutex::new(events.subscribe()));
            loop {
                let rx = Arc::clone(&receiver);
                let events = async_cx
                    .background_executor()
                    .spawn(async move {
                        let mut events = Vec::new();
                        let receiver = rx.lock().ok()?;
                        let first = receiver.recv().ok()?;
                        events.push(first);
                        let drain_until = Instant::now() + Duration::from_millis(80);
                        while events.len() < 128 {
                            let remaining = drain_until.saturating_duration_since(Instant::now());
                            if remaining.is_zero() {
                                break;
                            }
                            match receiver.recv_timeout(remaining) {
                                Ok(event) => events.push(event),
                                Err(_) => break,
                            }
                        }
                        Some(events)
                    })
                    .await;
                let Some(events) = events else {
                    break;
                };
                let should_refresh = events.iter().any(|event| {
                    event.kind == AppEventKind::FeatureChanged
                        && event.source.as_ref() == manifest::PLUGIN_ID
                });
                if should_refresh {
                    if panel
                        .update(async_cx, |panel, cx| {
                            panel.sync_engine_state();
                            panel.refresh_from_store(cx);
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }));
    }

    fn sync_engine_state(&mut self) {
        self.setup_info = build_setup_info(&self.engine, &self.ca_manager, self.engine_port);
        self.engine_running = self.setup_info.is_running();
        self.engine_port = self.setup_info.port();
    }

    fn start_proxy(&mut self, cx: &mut Context<Self>) {
        let port = if self.engine_port == 0 {
            DEFAULT_PROXY_PORT
        } else {
            self.engine_port
        };

        match self.engine.start(port) {
            Ok(()) => {
                self.notice = Some(format!(
                    "代理已启动: {}",
                    CaptureEndpoint {
                        ip: "127.0.0.1".to_string(),
                        port
                    }
                    .http_proxy_url()
                ));
            }
            Err(error) => {
                self.notice = Some(format!("启动代理失败: {error}"));
            }
        }
        self.sync_engine_state();
        cx.notify();
    }

    fn stop_proxy(&mut self, cx: &mut Context<Self>) {
        self.engine.stop();
        self.notice = Some(String::from("代理已停止"));
        self.sync_engine_state();
        cx.notify();
    }

    fn refresh_certificate_status(&mut self, cx: &mut Context<Self>) {
        match self.ca_manager.lock() {
            Ok(mut ca) => {
                ca.refresh_status();
                self.notice = Some(format!("证书状态: {}", ca.status().label()));
            }
            Err(error) => {
                self.notice = Some(format!("刷新证书状态失败: {error}"));
            }
        }
        self.sync_engine_state();
        cx.notify();
    }

    fn copy_text(&mut self, text: String, message: impl Into<String>, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.notice = Some(message.into());
        cx.notify();
    }

    fn copy_lan_proxy(&mut self, cx: &mut Context<Self>) {
        let proxy = self.setup_info.lan_endpoint.http_proxy_url();
        self.copy_text(proxy, "已复制移动端代理地址", cx);
    }

    fn copy_local_proxy(&mut self, cx: &mut Context<Self>) {
        let proxy = self.setup_info.local_endpoint.http_proxy_url();
        self.copy_text(proxy, "已复制本机代理地址", cx);
    }

    fn copy_cert_path(&mut self, cx: &mut Context<Self>) {
        self.copy_text(
            self.setup_info.mobile_cert_path.clone(),
            "已复制移动端证书路径",
            cx,
        );
    }

    fn copy_cert_download_url(&mut self, cx: &mut Context<Self>) {
        self.copy_text(
            self.setup_info.cert_download_url.clone(),
            "已复制手机证书下载地址",
            cx,
        );
    }

    fn copy_install_command(&mut self, cx: &mut Context<Self>) {
        if let Some(command) = self.setup_info.install_command.clone() {
            self.copy_text(command, "已复制系统信任安装命令", cx);
        } else {
            self.notice = Some(String::from("当前平台暂无自动安装命令，请手动导入证书"));
            cx.notify();
        }
    }

    fn open_certificate_dir(&mut self, cx: &mut Context<Self>) {
        let path = std::path::Path::new(&self.setup_info.ca_dir);
        match qingqi_platform::shell::open_directory(path) {
            Ok(()) => self.notice = Some(String::from("已打开证书目录")),
            Err(error) => self.notice = Some(format!("打开证书目录失败: {error}")),
        }
        cx.notify();
    }

    fn refresh_from_store(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.notice = None;
        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;
        let store = Arc::clone(&self.store);
        let filter = self.filter.clone();
        let offset = self.offset;

        self.reload_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let store = store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("capture store lock poisoned"))?;
                    let rows = store.query(&filter, offset, PAGE_SIZE)?;
                    let exchanges = if filter.hide_static {
                        rows.into_iter().filter(|ex| filter.matches(ex)).collect()
                    } else {
                        rows
                    };
                    let total = store.count(&filter)?;
                    anyhow::Ok((exchanges, total))
                })
                .await;

            let _ = panel.update(async_cx, |panel, cx| {
                if panel.load_generation != generation {
                    return;
                }
                panel.loading = false;
                panel.selected_id = None;
                panel.selected_detail = None;
                match result {
                    Ok((rows, total)) => {
                        panel.exchanges = rows;
                        panel.total = total;
                    }
                    Err(error) => {
                        panel.exchanges.clear();
                        panel.total = 0;
                        panel.notice = Some(format!("查询失败: {error}"));
                    }
                }
                cx.notify();
            });
        }));
    }

    fn select_exchange(&mut self, id: i64, cx: &mut Context<Self>) {
        self.selected_id = Some(id);
        self.selected_detail = None;
        let store = Arc::clone(&self.store);
        self.detail_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let store = store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("capture store lock poisoned"))?;
                    store.get_by_id(id)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                if panel.selected_id != Some(id) {
                    return;
                }
                match result {
                    Ok(detail) => panel.selected_detail = detail,
                    Err(error) => panel.notice = Some(format!("读取详情失败: {error}")),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.notice = Some(String::from("正在清空抓包记录..."));
        let store = Arc::clone(&self.store);
        self.reload_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let store = store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("capture store lock poisoned"))?;
                    store.clear()
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.loading = false;
                panel.notice = Some(match result {
                    Ok(_) => String::from("已清空所有抓包记录"),
                    Err(error) => format!("清空失败: {error}"),
                });
                panel.refresh_from_store(cx);
            });
        }));
        cx.notify();
    }

    fn export_har(&mut self, cx: &mut Context<Self>) {
        let result = match self.store.lock() {
            Ok(store) => store.export_har(),
            Err(error) => Err(anyhow::anyhow!("获取存储锁失败: {error}")),
        };
        match result {
            Ok(json) => {
                self.copy_text(json, "HAR 已复制到剪贴板", cx);
            }
            Err(error) => {
                self.notice = Some(format!("导出 HAR 失败: {error}"));
                cx.notify();
            }
        }
    }

    fn copy_as_curl(&mut self, exchange: &CapturedExchange, cx: &mut Context<Self>) {
        let mut cmd = format!("curl -X {} '{}'", exchange.method, exchange.url);
        let headers: Vec<(String, String)> =
            serde_json::from_str(&exchange.request_headers_json).unwrap_or_default();
        for (key, value) in &headers {
            if key.eq_ignore_ascii_case("host") {
                continue;
            }
            cmd.push_str(&format!(" -H '{key}: {value}'"));
        }
        if !exchange.request_body.is_empty() {
            cmd.push_str(&format!(" -d '{}'", exchange.request_body.replace('\'', r"'\''")));
        }
        self.copy_text(cmd, "cURL 命令已复制到剪贴板", cx);
    }

    fn toggle_composer(&mut self, cx: &mut Context<Self>) {
        self.composer_visible = !self.composer_visible;
        if self.composer_visible {
            self.composer_response = None;
        }
        cx.notify();
    }

    fn set_composer_method(&mut self, method: String, cx: &mut Context<Self>) {
        self.composer_method = method;
        cx.notify();
    }

    fn toggle_tree_view(&mut self, cx: &mut Context<Self>) {
        self.tree_view_mode = !self.tree_view_mode;
        if self.tree_view_mode {
            self.load_tree_data(cx);
        }
        cx.notify();
    }

    fn load_tree_data(&mut self, cx: &mut Context<Self>) {
        self.tree_load_generation = self.tree_load_generation.wrapping_add(1);
        let generation = self.tree_load_generation;
        let store = Arc::clone(&self.store);
        self.tree_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let store = store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("capture store lock poisoned"))?;
                    let exchanges = store.get_all_exchanges()?;
                    let domains = session_tree::build_session_tree(&exchanges);
                    let rows = session_tree::flatten_tree(&domains);
                    anyhow::Ok(rows)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                if panel.tree_load_generation != generation {
                    return;
                }
                match result {
                    Ok(rows) => {
                        panel.tree_rows = rows;
                        panel.tree_expanded = panel
                            .tree_rows
                            .iter()
                            .filter(|r| r.is_domain)
                            .map(|r| r.node_id.clone())
                            .collect();
                    }
                    Err(_) => panel.tree_rows.clear(),
                }
                cx.notify();
            });
        }));
    }

    fn toggle_tree_node(&mut self, node_id: String, cx: &mut Context<Self>) {
        if self.tree_expanded.contains(&node_id) {
            self.tree_expanded.remove(&node_id);
        } else {
            self.tree_expanded.insert(node_id);
        }
        cx.notify();
    }

    fn toggle_performance(&mut self, cx: &mut Context<Self>) {
        self.performance_visible = !self.performance_visible;
        if self.performance_visible && self.performance_stats.is_none() {
            self.load_performance_data(cx);
        }
        cx.notify();
    }

    fn load_performance_data(&mut self, cx: &mut Context<Self>) {
        self.performance_load_generation = self.performance_load_generation.wrapping_add(1);
        let generation = self.performance_load_generation;
        let store = Arc::clone(&self.store);
        self.performance_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let store = store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("capture store lock poisoned"))?;
                    let exchanges = store.get_all_exchanges()?;
                    let stats = performance::calculate_stats(&exchanges);
                    anyhow::Ok(stats)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                if panel.performance_load_generation != generation {
                    return;
                }
                match result {
                    Ok(stats) => panel.performance_stats = Some(stats),
                    Err(_) => panel.performance_stats = None,
                }
                cx.notify();
            });
        }));
    }

    fn toggle_breakpoint_panel(&mut self, cx: &mut Context<Self>) {
        self.breakpoint_visible = !self.breakpoint_visible;
        if self.breakpoint_visible {
            self.sync_breakpoint_rules(cx);
        }
        cx.notify();
    }

    fn sync_breakpoint_rules(&mut self, _cx: &mut Context<Self>) {
        if let Ok(mgr) = self.engine.breakpoint_manager().lock() {
            self.breakpoint_rules = mgr.list_rules().to_vec();
        }
    }

    fn add_breakpoint_rule(&mut self, cx: &mut Context<Self>) {
        let id = format!(
            "bp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rule = BreakpointRule::new(id, BreakpointPhase::BeforeRequest, "*");
        if let Ok(mut mgr) = self.engine.breakpoint_manager().lock() {
            mgr.add_rule(rule.clone());
        }
        self.breakpoint_rules.push(rule);
        cx.notify();
    }

    fn remove_breakpoint_rule(&mut self, id: String, cx: &mut Context<Self>) {
        if let Ok(mut mgr) = self.engine.breakpoint_manager().lock() {
            mgr.remove_rule(&id);
        }
        self.breakpoint_rules.retain(|r| r.id != id);
        cx.notify();
    }

    fn toggle_breakpoint_rule_enabled(&mut self, id: String, cx: &mut Context<Self>) {
        if let Ok(mut mgr) = self.engine.breakpoint_manager().lock() {
            mgr.update_rule(&id, |r| r.enabled = !r.enabled);
        }
        if let Some(rule) = self.breakpoint_rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = !rule.enabled;
        }
        cx.notify();
    }

    fn update_breakpoint_rule_pattern(&mut self, id: String, pattern: String, cx: &mut Context<Self>) {
        if let Ok(mut mgr) = self.engine.breakpoint_manager().lock() {
            mgr.update_rule(&id, |r| r.url_pattern.clone_from(&pattern));
        }
        if let Some(rule) = self.breakpoint_rules.iter_mut().find(|r| r.id == id) {
            rule.url_pattern = pattern;
        }
        cx.notify();
    }

    fn update_breakpoint_rule_method(&mut self, id: String, method: String, cx: &mut Context<Self>) {
        if let Ok(mut mgr) = self.engine.breakpoint_manager().lock() {
            mgr.update_rule(&id, |r| r.method.clone_from(&method));
        }
        if let Some(rule) = self.breakpoint_rules.iter_mut().find(|r| r.id == id) {
            rule.method = method;
        }
        cx.notify();
    }

    fn update_breakpoint_rule_phase(&mut self, id: String, phase: BreakpointPhase, cx: &mut Context<Self>) {
        if let Ok(mut mgr) = self.engine.breakpoint_manager().lock() {
            mgr.update_rule(&id, |r| r.phase = phase);
        }
        if let Some(rule) = self.breakpoint_rules.iter_mut().find(|r| r.id == id) {
            rule.phase = phase;
        }
        cx.notify();
    }

    fn toggle_throttle_panel(&mut self, cx: &mut Context<Self>) {
        self.throttle_visible = !self.throttle_visible;
        cx.notify();
    }

    fn set_throttle_preset(&mut self, preset: ThrottlePreset, cx: &mut Context<Self>) {
        self.engine.throttle_manager().set_preset(preset);
        cx.notify();
    }

    fn set_custom_kbps_value(&mut self, kbps: u32, cx: &mut Context<Self>) {
        self.engine.throttle_manager().set_custom_kbps(kbps);
        self.engine.throttle_manager().set_preset(ThrottlePreset::Custom);
        cx.notify();
    }

    fn toggle_rewrite_panel(&mut self, cx: &mut Context<Self>) {
        self.rewrite_visible = !self.rewrite_visible;
        if self.rewrite_visible {
            self.sync_rewrite_rules();
        }
        cx.notify();
    }

    fn sync_rewrite_rules(&mut self) {
        // Rewrite rules are stored locally in the view since the engine
        // does not currently expose a rewrite_engine accessor.
    }

    fn add_rewrite_rule(&mut self, cx: &mut Context<Self>) {
        let id = format!(
            "rw-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let rule = RewriteRule {
            id: id.clone(),
            enabled: true,
            name: "新规则".to_string(),
            condition: crate::rewrite::RewriteCondition {
                url_pattern: "*".to_string(),
                method: String::new(),
            },
            actions: vec![crate::rewrite::RewriteAction {
                target: RewriteTarget::ResponseHeader,
                header_name: None,
                match_pattern: String::new(),
                replace_value: String::new(),
                is_regex: false,
            }],
        };
        self.rewrite_rules.push(rule);
        cx.notify();
    }

    fn remove_rewrite_rule(&mut self, id: String, cx: &mut Context<Self>) {
        self.rewrite_rules.retain(|r| r.id != id);
        cx.notify();
    }

    fn send_composer_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_composer_inputs(window, cx);
        let url = self
            .composer_url
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default();
        if url.is_empty() {
            self.notice = Some(String::from("请输入请求 URL"));
            cx.notify();
            return;
        }
        let headers_raw = self
            .composer_headers
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default();
        let mut headers = Vec::new();
        for line in headers_raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                headers.push((key.trim().to_string(), value.trim().to_string()));
            }
        }
        let body = self
            .composer_body
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default();
        let request = ComposedRequest {
            method: self.composer_method.clone(),
            url,
            headers,
            body,
        };
        let engine = Arc::clone(&self.engine);
        self.composer_sending = true;
        self.notice = Some(String::from("正在发送请求..."));
        cx.notify();
        cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    engine
                        .composer()
                        .lock()
                        .map_err(|_| anyhow::anyhow!("composer lock poisoned"))?
                        .send_request(&request)
                        .map_err(|e| e.context("发送请求失败"))
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.composer_sending = false;
                match result {
                    Ok(response) => {
                        panel.composer_response = Some(response);
                        panel.notice = Some(String::from("请求已完成"));
                    }
                    Err(error) => {
                        panel.composer_response = None;
                        panel.notice = Some(format!("请求失败: {error}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_method_filter(&mut self, method: &str, cx: &mut Context<Self>) {
        if self.filter.method == method {
            self.filter.method.clear();
        } else {
            self.filter.method = method.to_string();
        }
        self.offset = 0;
        self.refresh_from_store(cx);
    }

    fn toggle_error_only(&mut self, cx: &mut Context<Self>) {
        self.filter.error_only = !self.filter.error_only;
        self.offset = 0;
        self.refresh_from_store(cx);
    }

    fn toggle_https_only(&mut self, cx: &mut Context<Self>) {
        self.filter.https_only = !self.filter.https_only;
        self.offset = 0;
        self.refresh_from_store(cx);
    }

    fn toggle_hide_static(&mut self, cx: &mut Context<Self>) {
        self.filter.hide_static = !self.filter.hide_static;
        self.offset = 0;
        self.refresh_from_store(cx);
    }

    fn set_detail_tab(&mut self, tab: DetailTab, cx: &mut Context<Self>) {
        self.detail_tab = tab;
        cx.notify();
    }

    fn reset_filters(&mut self, cx: &mut Context<Self>) {
        self.filter = FilterState::default();
        if let Some(search_input) = self.search_input.as_ref() {
            search_input.update(cx, |input, cx| input.reset_value("", cx));
        }
        if let Some(host_input) = self.host_input.as_ref() {
            host_input.update(cx, |input, cx| input.reset_value("", cx));
        }
        self.offset = 0;
        self.refresh_from_store(cx);
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if self.offset + PAGE_SIZE < self.total {
            self.offset += PAGE_SIZE;
            self.refresh_from_store(cx);
        }
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.offset > 0 {
            self.offset = (self.offset - PAGE_SIZE).max(0);
            self.refresh_from_store(cx);
        }
    }

    fn render_tree_view(
        &self,
        selected_id: Option<i64>,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let rows = self.tree_rows.clone();
        let expanded = self.tree_expanded.clone();

        if rows.is_empty() {
            return div()
                .flex_1()
                .min_h(px(0.0))
                .flex()
                .items_center()
                .justify_center()
                .child(components::empty_state(
                    "icons/capture.svg",
                    "暂无树形数据",
                    "树形视图下暂无可显示的请求",
                    cx,
                ))
                .into_any_element();
        }

        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .children(rows.iter().filter_map(|row| {
                        if row.is_domain {
                            let is_expanded = expanded.contains(&row.node_id);
                            let node_id = row.node_id.clone();
                            let display = row.display.clone();
                            Some(
                                div()
                                    .id(SharedString::from(format!("tree-domain-{}", &node_id)))
                                    .h(px(28.0))
                                    .px_3()
                                    .pl(px(8.0))
                                    .bg(ui::bg_subtle(cx))
                                    .hover(|s| s.bg(ui::bg_hover(cx)).cursor_pointer())
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_size(px(11.0))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.toggle_tree_node(node_id.clone(), cx);
                                    }))
                                    .child(
                                        div()
                                            .w(px(12.0))
                                            .text_size(px(9.0))
                                            .text_color(ui::text_secondary(cx))
                                            .child(if is_expanded { "▼" } else { "▶" }),
                                    )
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(ui::text_primary(cx))
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(display),
                                    ),
                            )
                        } else {
                            let domain_idx = rows.iter().position(|r| r.node_id == row.node_id && !r.is_domain);
                            let domain = domain_idx.and_then(|idx| {
                                rows[..idx].iter().rev().find(|r| r.is_domain)
                            })?;
                            let domain_id = &domain.node_id;
                            if !expanded.contains(domain_id) {
                                return None;
                            }
                            let ex_id = row.exchange_id?;
                            let is_selected = selected_id == Some(ex_id);
                            let display = row.display.clone();
                            let method_color = if display.starts_with("GET") {
                                theme::http_method_color("GET", dark)
                            } else if display.starts_with("POST") {
                                theme::http_method_color("POST", dark)
                            } else if display.starts_with("PUT") {
                                theme::http_method_color("PUT", dark)
                            } else if display.starts_with("DELETE") {
                                theme::http_method_color("DELETE", dark)
                            } else {
                                theme::http_method_color("GET", dark)
                            };
                            Some(
                                div()
                                    .id(("tree-req", ex_id as u64))
                                    .h(px(26.0))
                                    .px_3()
                                    .pl(px(28.0))
                                    .bg(if is_selected {
                                        tokens(cx).primary
                                    } else {
                                        ui::bg_surface(cx)
                                    })
                                    .hover(|s| s.bg(ui::bg_hover(cx)).cursor_pointer())
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(11.0))
                                    .font_family("SF Mono")
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.select_exchange(ex_id, cx);
                                    }))
                                    .child(
                                        div()
                                            .text_color(method_color)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(display),
                                    ),
                            )
                        }
                    })),
            )
            .into_any_element()
    }

    fn render_performance_panel(&self, cx: &App) -> gpui::AnyElement {
        let stats = match &self.performance_stats {
            Some(s) => s.clone(),
            None => {
                return div()
                    .rounded(theme::radius_lg())
                    .bg(ui::bg_surface(cx))
                    .border_1()
                    .border_color(ui::border_light(cx))
                    .p_4()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(ui::text_tertiary(cx))
                    .child("正在加载性能数据...")
                    .into_any_element();
            }
        };

        let total_requests = stats.total_requests;
        let error_rate_pct = stats.error_rate * 100.0;

        div()
            .rounded(theme::radius_lg())
            .bg(ui::bg_surface(cx))
            .border_1()
            .border_color(ui::border_light(cx))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .max_h(px(400.0))
            .overflow_y_scrollbar()
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("性能分析"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary(cx))
                            .child(format!("共 {} 条请求", total_requests)),
                    ),
            )
            .child(Divider::horizontal().color(ui::border_light(cx)))
            // Key metrics row
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(perf_metric_card(
                        "总请求数",
                        &format!("{total_requests}"),
                        PluginAccent::Blue,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "总流量",
                        &crate::model::format_bytes(stats.total_bytes),
                        PluginAccent::Cyan,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "平均响应",
                        &format!("{:.0}ms", stats.avg_response_time_ms),
                        PluginAccent::Green,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "错误率",
                        &format!("{:.1}%", error_rate_pct),
                        if error_rate_pct > 10.0 {
                            PluginAccent::Rose
                        } else {
                            PluginAccent::Amber
                        },
                        cx,
                    )),
            )
            // Response time row
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(perf_metric_card(
                        "最大响应",
                        &format!("{}ms", stats.max_response_time_ms),
                        PluginAccent::Rose,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "最小响应",
                        &format!("{}ms", stats.min_response_time_ms),
                        PluginAccent::Green,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "请求/秒",
                        &format!("{:.1}", stats.requests_per_second),
                        PluginAccent::Purple,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "",
                        "",
                        PluginAccent::Blue,
                        cx,
                    )),
            )
            .child(Divider::horizontal().color(ui::border_light(cx)))
            // Status distribution
            .child(section_label("状态码分布", cx))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(perf_metric_card(
                        "2xx 成功",
                        &format!("{}", stats.status_code_distribution.ok_2xx),
                        PluginAccent::Green,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "3xx 重定向",
                        &format!("{}", stats.status_code_distribution.redirect_3xx),
                        PluginAccent::Blue,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "4xx 客户端错误",
                        &format!("{}", stats.status_code_distribution.client_error_4xx),
                        PluginAccent::Amber,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "5xx 服务端错误",
                        &format!("{}", stats.status_code_distribution.server_error_5xx),
                        PluginAccent::Rose,
                        cx,
                    )),
            )
            .child(Divider::horizontal().color(ui::border_light(cx)))
            // Content type distribution
            .child(section_label("内容类型分布", cx))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(perf_metric_card(
                        "HTML",
                        &format!("{}", stats.content_type_distribution.html),
                        PluginAccent::Blue,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "JSON",
                        &format!("{}", stats.content_type_distribution.json),
                        PluginAccent::Green,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "图片",
                        &format!("{}", stats.content_type_distribution.image),
                        PluginAccent::Purple,
                        cx,
                    ))
                    .child(perf_metric_card(
                        "其他",
                        &format!(
                            "{}",
                            stats.content_type_distribution.css
                                + stats.content_type_distribution.javascript
                                + stats.content_type_distribution.other
                        ),
                        PluginAccent::Slate,
                        cx,
                    )),
            )
            .child(Divider::horizontal().color(ui::border_light(cx)))
            // Slowest endpoints
            .child(section_label("最慢的 Top 10 端点", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_px()
                    .children(
                        stats
                            .slowest_endpoints
                            .iter()
                            .take(10)
                            .map(|ep| {
                                let method_color = theme::http_method_color(&ep.method, tokens(cx).is_dark());
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .p_1()
                                    .rounded(theme::radius_sm())
                                    .hover(|s| s.bg(ui::bg_subtle(cx)))
                                    .child(
                                        div()
                                            .w(px(54.0))
                                            .text_size(px(11.0))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(method_color)
                                            .child(ep.method.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(11.0))
                                            .font_family(ui::font_mono())
                                            .text_color(ui::text_primary(cx))
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(ep.url.clone()),
                                    )
                                    .child(
                                        div()
                                            .w(px(60.0))
                                            .text_align(gpui::TextAlign::Right)
                                            .text_size(px(11.0))
                                            .text_color(ui::text_secondary(cx))
                                            .child(format!("{:.0}ms", ep.avg_duration_ms)),
                                    )
                                    .child(
                                        div()
                                            .w(px(40.0))
                                            .text_align(gpui::TextAlign::Right)
                                            .text_size(px(10.0))
                                            .text_color(ui::text_tertiary(cx))
                                            .child(format!("×{}", ep.count)),
                                    )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn ensure_breakpoint_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.breakpoint_url_input.is_none() {
            self.breakpoint_url_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("*")));
        }
        if self.breakpoint_method_input.is_none() {
            self.breakpoint_method_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("ALL")));
        }
    }

    fn ensure_rewrite_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rewrite_name_input.is_none() {
            self.rewrite_name_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("规则名称")));
        }
        if self.rewrite_url_input.is_none() {
            self.rewrite_url_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("*")));
        }
    }

    fn render_breakpoint_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let rules = self.breakpoint_rules.clone();
        div()
            .rounded(theme::radius_lg())
            .bg(ui::bg_surface(cx))
            .border_1()
            .border_color(ui::border_light(cx))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .max_h(px(400.0))
            .overflow_y_scrollbar()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("断点规则"),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("add-breakpoint-rule-btn")
                            .label("添加规则")
                            .with_size(Size::XSmall)
                            .with_variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.add_breakpoint_rule(cx);
                            })),
                    ),
            )
            .child(Divider::horizontal().color(ui::border_light(cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(rules.iter().map(|rule| {
                        let url_pattern = rule.url_pattern.clone();
                        let method = rule.method.clone();
                        let phase = rule.phase;
                        let enabled = rule.enabled;
                        let id_enabled = rule.id.clone();
                        let id_delete = rule.id.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .p_2()
                            .rounded(theme::radius_sm())
                            .bg(ui::bg_subtle(cx))
                            .child(
                                div()
                                    .id(SharedString::from(format!("bp-enabled-{}", rule.id)))
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded(theme::radius_sm())
                                    .bg(if enabled { ui::success(cx) } else { ui::bg_subtle(cx) })
                                    .border_1()
                                    .border_color(if enabled { ui::success(cx) } else { ui::border_light(cx) })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.toggle_breakpoint_rule_enabled(id_enabled.clone(), cx);
                                    }))
                                    .child(if enabled { "✓" } else { "" }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(24.0))
                                    .rounded(theme::radius_sm())
                                    .bg(ui::bg_canvas(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .text_size(px(11.0))
                                    .text_color(ui::text_primary(cx))
                                    .child(url_pattern),
                            )
                            .child(
                                div()
                                    .w(px(70.0))
                                    .h(px(24.0))
                                    .rounded(theme::radius_sm())
                                    .bg(ui::bg_canvas(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .px_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .text_color(if method.is_empty() || method == "ALL" {
                                        ui::text_secondary(cx)
                                    } else {
                                        ui::text_primary(cx)
                                    })
                                    .child(if method.is_empty() { "ALL".to_string() } else { method.clone() }),
                            )
                            .child(
                                div()
                                    .w(px(60.0))
                                    .h(px(24.0))
                                    .rounded(theme::radius_sm())
                                    .bg(ui::bg_canvas(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .px_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .text_color(ui::text_primary(cx))
                                    .child(phase.label()),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("bp-del-{}", rule.id)))
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded(theme::radius_sm())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.0))
                                    .text_color(ui::danger(cx))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::rgba_with_alpha(ui::danger(cx).into(), 0.1)))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.remove_breakpoint_rule(id_delete.clone(), cx);
                                    }))
                                    .child("✕"),
                            )
                    })),
            )
            .into_any_element()
    }

    fn render_throttle_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let current_config = self.engine.throttle_manager().config();
        let presets = [
            ThrottlePreset::Off,
            ThrottlePreset::ThreeG,
            ThrottlePreset::FourG,
            ThrottlePreset::Custom,
        ];
        let custom_kbps = current_config.custom_kbps;
        div()
            .rounded(theme::radius_lg())
            .bg(ui::bg_surface(cx))
            .border_1()
            .border_color(ui::border_light(cx))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("限速控制"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary(cx))
                            .child(format!("当前: {}", current_config.preset.label())),
                    ),
            )
            .child(Divider::horizontal().color(ui::border_light(cx)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children(presets.iter().map(|&preset| {
                        let is_active = current_config.preset == preset;
                        let color = if is_active { ui::success(cx) } else { ui::text_secondary(cx) };
                        div()
                            .id(SharedString::from(format!("throttle-preset-{:?}", preset)))
                            .px_3()
                            .h(px(28.0))
                            .rounded(theme::radius_sm())
                            .bg(if is_active {
                                theme::rgba_with_alpha(color.into(), 0.12)
                            } else {
                                ui::bg_subtle(cx)
                            })
                            .border_1()
                            .border_color(if is_active { color } else { ui::border_light(cx) })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.0))
                            .text_color(if is_active { color } else { ui::text_secondary(cx) })
                            .font_weight(if is_active { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL })
                            .cursor_pointer()
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                panel.set_throttle_preset(preset, cx);
                            }))
                            .child(preset.label())
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary(cx))
                            .child("自定义 Kbps:"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .h(px(24.0))
                            .rounded(theme::radius_sm())
                            .bg(ui::bg_canvas(cx))
                            .border_1()
                            .border_color(ui::border_light(cx))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_size(px(11.0))
                            .text_color(ui::text_primary(cx))
                            .child(format!("{custom_kbps}")),
                    )
                    .child(
                        Button::new("throttle-custom-apply-btn")
                            .label("应用")
                            .with_size(Size::XSmall)
                            .with_variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                panel.set_custom_kbps_value(custom_kbps, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_rewrite_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let rules = self.rewrite_rules.clone();
        div()
            .rounded(theme::radius_lg())
            .bg(ui::bg_surface(cx))
            .border_1()
            .border_color(ui::border_light(cx))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .max_h(px(400.0))
            .overflow_y_scrollbar()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("改写规则"),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("add-rewrite-rule-btn")
                            .label("添加规则")
                            .with_size(Size::XSmall)
                            .with_variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.add_rewrite_rule(cx);
                            })),
                    ),
            )
            .child(Divider::horizontal().color(ui::border_light(cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(rules.iter().map(|rule| {
                        let id = rule.id.clone();
                        let name = rule.name.clone();
                        let url_pattern = rule.condition.url_pattern.clone();
                        let action_target = rule.actions.first().map(|a| a.target).unwrap_or(RewriteTarget::ResponseHeader);
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .p_2()
                            .rounded(theme::radius_sm())
                            .bg(ui::bg_subtle(cx))
                            .child(
                                div()
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded(theme::radius_sm())
                                    .bg(if rule.enabled { ui::success(cx) } else { ui::bg_subtle(cx) })
                                    .border_1()
                                    .border_color(if rule.enabled { ui::success(cx) } else { ui::border_light(cx) })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .child(if rule.enabled { "✓" } else { "" }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(11.0))
                                    .text_color(ui::text_primary(cx))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .w(px(60.0))
                                    .text_size(px(10.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child(url_pattern),
                            )
                            .child(
                                div()
                                    .w(px(70.0))
                                    .text_size(px(10.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child(action_target.label()),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("rw-del-{}", rule.id)))
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded(theme::radius_sm())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.0))
                                    .text_color(ui::danger(cx))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::rgba_with_alpha(ui::danger(cx).into(), 0.1)))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.remove_rewrite_rule(id.clone(), cx);
                                    }))
                                    .child("✕"),
                            )
                    })),
            )
            .into_any_element()
    }

    fn status_text(&self) -> String {
        if let Some(ref notice) = self.notice {
            return notice.clone();
        }
        if self.loading {
            return String::from("正在加载抓包记录...");
        }
        if self.exchanges.is_empty() {
            return String::from("捕获引擎未接入 — 当前仅展示已持久化的抓包数据");
        }
        let start = self.offset + 1;
        let visible_end = self.offset + self.exchanges.len() as i64;
        if self.filter.hide_static {
            format!(
                "第 {start}–{visible_end} 条（静态文件已隐藏），总计约 {} 条",
                self.total
            )
        } else {
            let end = (self.offset + PAGE_SIZE).min(self.total);
            format!("第 {start}–{end} 条，共 {} 条", self.total)
        }
    }

    fn status_color(status: i64, cx: &App) -> gpui::Rgba {
        if status >= 500 {
            tokens(cx).danger.into()
        } else if status >= 400 {
            tokens(cx).warning.into()
        } else if status >= 300 {
            tokens(cx).info.into()
        } else if status >= 200 {
            tokens(cx).success.into()
        } else {
            tokens(cx).muted_foreground.into()
        }
    }

}

fn extract_content_type(ex: &CapturedExchange) -> String {
    let headers: Vec<(String, String)> =
        serde_json::from_str(&ex.response_headers_json).unwrap_or_default();
    for (key, value) in &headers {
        if key.eq_ignore_ascii_case("content-type") {
            return value.clone();
        }
    }
    String::new()
}

fn is_video_exchange(ex: &CapturedExchange) -> bool {
    let content_type = extract_content_type(ex);
    video_sniff::is_video_stream(&ex.url, &content_type, ex.response_size)
}



fn build_setup_info(
    engine: &Arc<CaptureEngine>,
    ca_manager: &Arc<Mutex<CaManager>>,
    fallback_port: u16,
) -> CaptureSetupInfo {
    let proxy_state = engine.proxy_state();
    let port = proxy_state.port().unwrap_or(fallback_port);
    let lan_ip = detect_lan_ip().unwrap_or_else(|| "127.0.0.1".to_string());

    let (certificate_status, cert_path, mobile_cert_path, ca_dir, install_command) =
        match ca_manager.lock() {
            Ok(mut ca) => {
                ca.refresh_status();
                (
                    ca.status(),
                    ca.cert_file_path().display().to_string(),
                    ca.mobile_cert_file_path().display().to_string(),
                    ca.ca_dir().display().to_string(),
                    ca.install_command(),
                )
            }
            Err(_) => (
                CertificateStatus::NotGenerated,
                String::new(),
                String::new(),
                String::new(),
                None,
            ),
        };

    CaptureSetupInfo {
        proxy_state,
        certificate_status,
        local_endpoint: CaptureEndpoint {
            ip: "127.0.0.1".to_string(),
            port,
        },
        lan_endpoint: CaptureEndpoint { ip: lan_ip, port },
        cert_path,
        mobile_cert_path,
        cert_download_url: "http://qingqi.cert/qingqi-ca-cert.crt".to_string(),
        ca_dir,
        install_command,
    }
}

fn detect_lan_ip() -> Option<String> {
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).ok()?;
    socket.connect(SocketAddr::from(([8, 8, 8, 8], 80))).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

fn status_badge(label: &str, color: gpui::Rgba) -> gpui::AnyElement {
    div()
        .h(px(24.0))
        .px_2()
        .rounded(theme::radius_sm())
        .bg(theme::rgba_with_alpha(color, 0.12))
        .border_1()
        .border_color(theme::rgba_with_alpha(color, 0.35))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label.to_string())
        .into_any_element()
}

fn section_label(label: &str, cx: &App) -> gpui::AnyElement {
    div()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(ui::text_secondary(cx))
        .child(label.to_string())
        .into_any_element()
}

fn proxy_value_row(
    label: &str,
    value: String,
    action: impl IntoElement,
    cx: &App,
) -> gpui::AnyElement {
    div()
        .rounded(theme::radius_md())
        .bg(ui::bg_subtle(cx))
        .border_1()
        .border_color(ui::border_light(cx))
        .p_2()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(44.0))
                .text_size(px(11.0))
                .text_color(ui::text_secondary(cx))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .font_family(ui::font_mono())
                .text_size(px(11.0))
                .text_color(ui::text_primary(cx))
                .overflow_hidden()
                .text_ellipsis()
                .child(value),
        )
        .child(action)
        .into_any_element()
}

fn perf_metric_card(
    label: &str,
    value: &str,
    accent: PluginAccent,
    cx: &App,
) -> gpui::AnyElement {
    let color = ui::accent_color(accent);
    div()
        .flex_1()
        .rounded(theme::radius_md())
        .bg(theme::rgba_with_alpha(color.into(), 0.06))
        .border_1()
        .border_color(theme::rgba_with_alpha(color.into(), 0.15))
        .p_2()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_secondary(cx))
                .child(label.to_string()),
        )
                .child(
            div()
                .text_size(px(14.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(gpui::Rgba::from(color))
                .font_family(ui::font_mono())
                .child(value.to_string()),
        )
        .into_any_element()
}

fn small_action(id: &'static str, label: &str, _cx: &App) -> Button {
    Button::new(id)
        .label(label.to_string())
        .with_size(Size::Small)
        .with_size(Size::XSmall)
}

fn capture_input(state: Entity<InputState>) -> Input {
    Input::new(&state)
        .appearance(false)
        .bordered(false)
        .focus_bordered(false)
        .h(px(32.0))
        .text_size(px(12.0))
}

fn guide_step(index: &str, text: &str, cx: &App) -> gpui::AnyElement {
    div()
        .flex()
        .items_start()
        .gap_2()
        .child(
            div()
                .w(px(18.0))
                .h(px(18.0))
                .rounded(theme::radius_sm())
                .bg(Theme::global(cx).primary_hover)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(Theme::global(cx).primary_active)
                .child(index.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(ui::text_secondary(cx))
                .child(text.to_string()),
        )
        .into_any_element()
}

fn value_line(label: &str, value: String, value_color: gpui::Rgba, cx: &App) -> gpui::AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(11.0))
        .child(
            div()
                .w(px(56.0))
                .text_color(ui::text_secondary(cx))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .font_family(ui::font_mono())
                .text_color(value_color)
                .overflow_hidden()
                .text_ellipsis()
                .child(value),
        )
        .into_any_element()
}

fn short_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::from("-");
    }
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!(".../{name}"))
        .unwrap_or_else(|| path.to_string())
}

impl Render for CaptureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(window, cx);
        self.ensure_composer_inputs(window, cx);
        self.ensure_breakpoint_inputs(window, cx);
        self.ensure_rewrite_inputs(window, cx);
        let dark = tokens(cx).is_dark();
        let exchanges = self.exchanges.clone();
        let total = self.total;
        let selected_id = self.selected_id;
        let selected_detail = self.selected_detail.clone();
        let offset = self.offset;
        let engine_running = self.engine_running;
        let search_input = self.search_input.clone().expect("search input initialized");
        let host_input = self.host_input.clone().expect("host input initialized");
        let composer_visible = self.composer_visible;
        let composer_method = self.composer_method.clone();
        let composer_url = self.composer_url.clone().expect("composer url initialized");
        let composer_headers = self.composer_headers.clone().expect("composer headers initialized");
        let composer_body = self.composer_body.clone().expect("composer body initialized");
        let composer_response = self.composer_response.clone();
        let composer_sending = self.composer_sending;
        let filter_method = self.filter.method.clone();
        let filter_error_only = self.filter.error_only;
        let filter_https_only = self.filter.https_only;
        let filter_hide_static = self.filter.hide_static;
        let detail_tab = self.detail_tab;
        let notice = self.notice.clone();
        let setup_info = self.setup_info.clone();
        let certificate_status = setup_info.certificate_status;
        let local_proxy = setup_info.local_endpoint.http_proxy_url();
        let lan_proxy = setup_info.lan_endpoint.http_proxy_url();
        let mobile_cert_path = setup_info.mobile_cert_path.clone();
        let cert_download_url = setup_info.cert_download_url.clone();
        let has_active_filter = !self.filter.search.trim().is_empty()
            || !self.filter.host.trim().is_empty()
            || !self.filter.method.is_empty()
            || self.filter.error_only
            || self.filter.https_only
            || self.filter.hide_static;
        let page_count = exchanges.len();
        let has_prev = offset > 0;
        // When hide_static is on, the SQL count includes static files that are
        // filtered in-memory, so we use the actual page result count to decide
        // whether a next page might exist.
        let has_next = if filter_hide_static {
            page_count == PAGE_SIZE as usize
        } else {
            offset + PAGE_SIZE < total
        };

        div()
            .size_full()
            .bg(ui::bg_canvas(cx))
            .text_color(ui::text_primary(cx))
            .font_family(ui::font_ui())
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            // ── Header ──
            .child(
                div()
                    .h(px(54.0))
                    .rounded(theme::radius_lg())
                    .bg(ui::bg_surface(cx))
                    .border_1()
                    .border_color(ui::border_light(cx))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(34.0))
                            .h(px(34.0))
                            .rounded(theme::radius_md())
                            .bg(if engine_running {
                                ui::success(cx)
                            } else {
                                ui::bg_subtle(cx)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(ui::icon_element(
                                "icons/antenna.svg",
                                if engine_running {
                                    ui::bg_canvas(cx).into()
                                } else {
                                    ui::text_secondary(cx).into()
                                },
                                18.0,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("抓包代理"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child(if engine_running {
                                        "HTTP/HTTPS MITM 代理运行中，可接入桌面或移动端"
                                    } else {
                                        "启动代理后，将系统或手机代理指向下方地址"
                                    }),
                            ),
                    )
                    .child(div().flex_1())
                    .child(status_badge(
                        if engine_running {
                            "运行中"
                        } else {
                            "已停止"
                        },
                        if engine_running {
                            ui::success(cx).into()
                        } else {
                            ui::text_secondary(cx).into()
                        },
                    ))
                    .child(status_badge(
                        certificate_status.label(),
                        if certificate_status == CertificateStatus::Installed {
                            ui::success(cx).into()
                        } else if certificate_status.ready_for_https() {
                            ui::warning(cx).into()
                        } else {
                            ui::danger(cx).into()
                        },
                    ))
                    .child(if engine_running {
                        Button::new("stop-proxy-btn").label("停止代理").with_size(Size::Small).with_variant(ButtonVariant::Danger)
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.stop_proxy(cx);
                            }))
                    } else {
                        Button::new("start-proxy-btn").label("启动代理").with_size(Size::Small).with_variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.start_proxy(cx);
                            }))
                    })
                    .child({
                        Button::new("export-har-btn").label("导出 HAR").with_size(Size::Small).with_variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.export_har(cx);
                            }))
                    })
                    .child({
                        Button::new("toggle-composer-btn").label("请求构造器").with_size(Size::Small).with_variant(if self.composer_visible {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.toggle_composer(cx);
                            }))
                    })
                    .child({
                        Button::new("toggle-tree-view-btn").label(if self.tree_view_mode { "列表视图" } else { "树形视图" }).with_size(Size::Small).with_variant(if self.tree_view_mode {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.toggle_tree_view(cx);
                            }))
                    })
                    .child({
                        Button::new("toggle-performance-btn").label("性能分析").with_size(Size::Small).with_variant(if self.performance_visible {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.toggle_performance(cx);
                            }))
                    })
                    .child({
                        Button::new("toggle-breakpoint-btn").label("断点规则").with_size(Size::Small).with_variant(if self.breakpoint_visible {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.toggle_breakpoint_panel(cx);
                            }))
                    })
                    .child({
                        Button::new("toggle-throttle-btn").label("限速控制").with_size(Size::Small).with_variant(if self.throttle_visible {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.toggle_throttle_panel(cx);
                            }))
                    })
                    .child({
                        Button::new("toggle-rewrite-btn").label("改写规则").with_size(Size::Small).with_variant(if self.rewrite_visible {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.toggle_rewrite_panel(cx);
                            }))
                    })
                    .child({
                        Button::new("reset-filter-btn").label("重置过滤").with_size(Size::Small).with_variant(ButtonVariant::Ghost)
                            .disabled(!has_active_filter)
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.reset_filters(cx);
                            }))
                    })
                    .child({
                        Button::new("clear-btn").label("清空记录").with_size(Size::Small).with_variant(ButtonVariant::Danger)
                            .disabled(total == 0)
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.clear_all(cx);
                            }))
                    }),
            )
            // ── Main content: filter + list + detail ──
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap_3()
                    .min_h(px(0.0))
                    .min_w(px(0.0))
                    // ── Left filter panel ──
                    .child(
                        div()
                            .w(px(260.0))
                            .rounded(theme::radius_lg())
                            .bg(ui::bg_surface(cx))
                            .border_1()
                            .border_color(ui::border_light(cx))
                            .p_3()
                            .flex()
                            .flex_col()
                            .min_h(px(0.0))
                            .overflow_y_scrollbar()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(section_label("连接向导", cx))
                                    .child(proxy_value_row(
                                        "本机",
                                        local_proxy.clone(),
                                        small_action("copy-local-proxy", "复制", cx).on_click(
                                            cx.listener(|panel, _, _, cx| {
                                                panel.copy_local_proxy(cx);
                                            }),
                                        ),
                                        cx,
                                    ))
                                    .child(proxy_value_row(
                                        "移动端",
                                        lan_proxy.clone(),
                                        small_action("copy-lan-proxy", "复制", cx).on_click(
                                            cx.listener(|panel, _, _, cx| {
                                                panel.copy_lan_proxy(cx);
                                            }),
                                        ),
                                        cx,
                                    ))
                                    .child(
                                        div()
                                            .rounded(theme::radius_md())
                                            .bg(ui::bg_subtle(cx))
                                            .border_1()
                                            .border_color(ui::border_light(cx))
                                            .p_2()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(guide_step(
                                                "1",
                                                if engine_running {
                                                    "代理已运行，保持此窗口打开"
                                                } else {
                                                    "先点击右上角启动代理"
                                                },
                                                cx,
                                            ))
                                            .child(guide_step(
                                                "2",
                                                "桌面应用填本机地址，手机需同一局域网并填移动端地址",
                                                cx,
                                            ))
                                            .child(guide_step(
                                                "3",
                                                "手机设置代理后访问下载地址，安装并信任 Qingqi CA",
                                                cx,
                                            ))
                                            .child(guide_step(
                                                "4",
                                                "遇到证书固定的 App 时，该请求可能无法解密",
                                                cx,
                                            )),
                                    )
                                    .child(section_label("HTTPS 证书", cx))
                                    .child(
                                        div()
                                            .rounded(theme::radius_md())
                                            .bg(ui::bg_subtle(cx))
                                            .border_1()
                                            .border_color(ui::border_light(cx))
                                            .p_2()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(value_line(
                                                "状态",
                                                certificate_status.label().to_string(),
                                                if certificate_status
                                                    == CertificateStatus::Installed
                                                {
                                                    ui::success(cx).into()
                                                } else {
                                                    ui::warning(cx).into()
                                                },
                                                cx,
                                            ))
                                            .child(value_line(
                                                "手机访问",
                                                cert_download_url.clone(),
                                                tokens(cx).primary.into(),
                                                cx,
                                            ))
                                            .child(value_line(
                                                "移动证书",
                                                short_path(&mobile_cert_path),
                                                ui::text_primary(cx).into(),
                                                cx,
                                            ))
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_1()
                                                    .child(
                                                        small_action(
                                                            "copy-cert-download-url",
                                                            "复制下载地址",
                                                            cx,
                                                        )
                                                        .on_click(cx.listener(|panel, _, _, cx| {
                                                            panel.copy_cert_download_url(cx);
                                                        })),
                                                    )
                                                    .child(
                                                        small_action(
                                                            "copy-cert-path",
                                                            "复制证书路径",
                                                            cx,
                                                        )
                                                        .on_click(cx.listener(|panel, _, _, cx| {
                                                            panel.copy_cert_path(cx);
                                                        })),
                                                    ),
                                            )
                                            .child(div().flex().gap_1().child(
                                                small_action("open-cert-dir", "打开目录", cx).on_click(
                                                    cx.listener(|panel, _, _, cx| {
                                                        panel.open_certificate_dir(cx);
                                                    }),
                                                ),
                                            ))
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_1()
                                                    .child(
                                                        small_action(
                                                            "copy-install-command",
                                                            "复制安装命令",
                                                            cx,
                                                        )
                                                        .on_click(cx.listener(|panel, _, _, cx| {
                                                            panel.copy_install_command(cx);
                                                        })),
                                                    )
                                                    .child(
                                                        small_action(
                                                            "refresh-cert-status",
                                                            "刷新状态",
                                                            cx,
                                                        )
                                                        .on_click(cx.listener(|panel, _, _, cx| {
                                                            panel.refresh_certificate_status(cx);
                                                        })),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(Divider::horizontal().color(ui::border_light(cx)))
                            .child(
                                div()
                                    .rounded(theme::radius_md())
                                    .bg(ui::bg_subtle(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .child(capture_input(search_input.clone())),
                            )
                            .child(
                                div()
                                    .rounded(theme::radius_md())
                                    .bg(ui::bg_subtle(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .child(capture_input(host_input.clone())),
                            )
                            // Method filter chips
                            .child(div().flex().flex_wrap().gap_1().children(
                                ["GET", "POST", "PUT", "DELETE"].iter().map(|&m| {
                                    let active = filter_method == m;
                                    let color = theme::http_method_color(m, dark);
                                    let chip_bg: gpui::Hsla = if active {
                                        theme::rgba_with_alpha(color, 0.18)
                                    } else {
                                        theme::rgba_with_alpha(ui::bg_subtle(cx).into(), 1.0)
                                    };
                                    div()
                                        .id(SharedString::from(format!("method-chip-{m}")))
                                        .px_2()
                                        .h(px(22.0))
                                        .rounded(px(999.0))
                                        .bg(chip_bg)
                                        .border_1()
                                        .border_color(if active {
                                            color
                                        } else {
                                            ui::border_light(cx).into()
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(11.0))
                                        .text_color(if active {
                                            color
                                        } else {
                                            ui::text_secondary(cx).into()
                                        })
                                        .font_weight(if active {
                                            gpui::FontWeight::SEMIBOLD
                                        } else {
                                            gpui::FontWeight::NORMAL
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |panel, _, _, cx| {
                                            panel.toggle_method_filter(m, cx);
                                        }))
                                        .child(m)
                                }),
                            ))
                            // Toggle chips row
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child({
                                        let active = filter_error_only;
                                        let color = ui::danger(cx).into();
                                        let chip_bg: gpui::Hsla = if active {
                                            theme::rgba_with_alpha(color, 0.18)
                                        } else {
                                            theme::rgba_with_alpha(ui::bg_subtle(cx).into(), 1.0)
                                        };
                                        div()
                                            .id("error-toggle")
                                            .px_2()
                                            .h(px(22.0))
                                            .rounded(px(999.0))
                                            .bg(chip_bg)
                                            .border_1()
                                            .border_color(if active {
                                                color
                                            } else {
                                                ui::border_light(cx).into()
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                color
                                            } else {
                                                ui::text_secondary(cx).into()
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|panel, _, _, cx| {
                                                panel.toggle_error_only(cx);
                                            }))
                                            .child("错误")
                                    })
                                    .child({
                                        let active = filter_https_only;
                                        let color = ui::success(cx).into();
                                        let chip_bg: gpui::Hsla = if active {
                                            theme::rgba_with_alpha(color, 0.18)
                                        } else {
                                            theme::rgba_with_alpha(ui::bg_subtle(cx).into(), 1.0)
                                        };
                                        div()
                                            .id("https-toggle")
                                            .px_2()
                                            .h(px(22.0))
                                            .rounded(px(999.0))
                                            .bg(chip_bg)
                                            .border_1()
                                            .border_color(if active {
                                                color
                                            } else {
                                                ui::border_light(cx).into()
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                color
                                            } else {
                                                ui::text_secondary(cx).into()
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|panel, _, _, cx| {
                                                panel.toggle_https_only(cx);
                                            }))
                                            .child("HTTPS")
                                    })
                                    .child({
                                        let active = filter_hide_static;
                                        let color = ui::warning(cx).into();
                                        let chip_bg: gpui::Hsla = if active {
                                            theme::rgba_with_alpha(color, 0.18)
                                        } else {
                                            theme::rgba_with_alpha(ui::bg_subtle(cx).into(), 1.0)
                                        };
                                        div()
                                            .id("hide-static-toggle")
                                            .px_2()
                                            .h(px(22.0))
                                            .rounded(px(999.0))
                                            .bg(chip_bg)
                                            .border_1()
                                            .border_color(if active {
                                                color
                                            } else {
                                                ui::border_light(cx).into()
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                color
                                            } else {
                                                ui::text_secondary(cx).into()
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|panel, _, _, cx| {
                                                panel.toggle_hide_static(cx);
                                            }))
                                            .child("隐藏静态")
                                    }),
                            )
                            .child(Divider::horizontal().color(ui::border_light(cx)))
                            .child(ui::metric_pill(
                                "总计",
                                format!("{total}"),
                                PluginAccent::Cyan,
                                cx,
                            ))
                            .child(ui::metric_pill(
                                "当前页",
                                format!("{page_count}"),
                                PluginAccent::Blue,
                                cx,
                            )),
                    )
                    // ── Center: exchange list / tree view ──
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .min_h(px(0.0))
                            .rounded(theme::radius_lg())
                            .bg(ui::bg_surface(cx))
                            .border_1()
                            .border_color(ui::border_light(cx))
                            .flex()
                            .flex_col()
                            // Table header
                            .child(
                                div()
                                    .h(px(30.0))
                                    .px_3()
                                    .bg(ui::bg_subtle(cx))
                                    .rounded_t(theme::radius_lg())
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(11.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child(if self.tree_view_mode {
                                        div().flex_1().child("Domain / Request")
                                    } else {
                                        div()
                                            .child(div().w(px(58.0)).child("时间"))
                                            .child(div().w(px(54.0)).child("方法"))
                                            .child(div().w(px(130.0)).child("Host"))
                                            .child(div().flex_1().child("URL"))
                                            .child(
                                                div()
                                                    .w(px(48.0))
                                                    .text_align(gpui::TextAlign::Right)
                                                    .child("状态"),
                                            )
                                            .child(
                                                div()
                                                    .w(px(70.0))
                                                    .text_align(gpui::TextAlign::Right)
                                                    .child("大小"),
                                            )
                                            .child(
                                                div()
                                                    .w(px(62.0))
                                                    .text_align(gpui::TextAlign::Right)
                                                    .child("耗时"),
                                            )
                                    }),
                            )
                            // List or tree or empty state
                            .child(if self.tree_view_mode {
                                self.render_tree_view(selected_id, dark, cx)
                            } else if exchanges.is_empty() {

                                div()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(components::empty_state(
                                        "icons/capture.svg",
                                        if has_active_filter {
                                            "暂无匹配记录"
                                        } else {
                                            "暂无抓包记录"
                                        },
                                        if has_active_filter {
                                            "当前过滤条件无匹配记录"
                                        } else {
                                            "暂无抓包记录 — 请先接入代理捕获引擎"
                                        },
                                        cx,
                                    ))
                                    .into_any_element()
                            } else {
                                div()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_h(px(0.0))
                                            .overflow_y_scrollbar()
                                            .children(exchanges.iter().enumerate().map(
                                                 |(i, ex)| {
                                                     let selected = selected_id == Some(ex.id);
                                                     let ex_id = ex.id;
                                                     let method_color = theme::http_method_color(&ex.method, dark);
                                                     let status_color = CaptureView::status_color(
                                                         ex.status,
                                                         cx,
                                                     );
                                                     let timestamp = ex.timestamp.clone();
                                                     let method = ex.method.clone();
                                                     let host = ex.host.clone();
                                                     let url = ex.url.clone();
                                                     let status = ex.status;
                                                     let size = ex.formatted_size();
                                                     let duration = ex.formatted_duration();
                                                     let is_video = is_video_exchange(ex);
                                                     let video_url = ex.url.clone();

                                                     div()
                                                         .id(("exchange-row", ex_id as u64))
                                                        .h(px(32.0))
                                                        .px_3()
                                                        .bg(if selected {
                                                            tokens(cx).primary
                                                        } else if i % 2 == 0 {
                                                            ui::bg_surface(cx)
                                                        } else {
                                                            ui::bg_subtle(cx)
                                                        })
                                                        .hover(|s| {
                                                            s.bg(ui::bg_hover(cx))
                                                                .cursor_pointer()
                                                        })
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .text_size(px(11.0))
                                                        .font_family("SF Mono")
                                                        .cursor_pointer()
                                                        .on_click(cx.listener(
                                                            move |panel, _, _, cx| {
                                                                panel.select_exchange(ex_id, cx);
                                                            },
                                                        ))
                                                        .child(
                                                            div()
                                                                .w(px(58.0))
                                                                .text_color(
                                                                    ui::text_secondary(cx),
                                                                )
                                                                .child(if timestamp.len() >= 16 {
                                                                    timestamp[11..16].to_string()
                                                                } else {
                                                                    timestamp
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(54.0))
                                                                .text_color(method_color)
                                                                .font_weight(
                                                                    gpui::FontWeight::SEMIBOLD,
                                                                )
                                                                .child(method),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(130.0))
                                                                .text_color(
                                                                    ui::text_primary(cx),
                                                                )
                                                                .overflow_hidden()
                                                                .text_ellipsis()
                                                                .child(host),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .text_color(
                                                                    ui::text_primary(cx),
                                                                )
                                                                .overflow_hidden()
                                                                .text_ellipsis()
                                                                .child(url),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(48.0))
                                                                .text_align(
                                                                    gpui::TextAlign::Right,
                                                                )
                                                                .text_color(if status > 0 {
                                                                    status_color
                                                                } else {
                                                                    ui::text_secondary(cx).into()
                                                                })
                                                                .font_weight(if status >= 400 {
                                                                    gpui::FontWeight::SEMIBOLD
                                                                } else {
                                                                    gpui::FontWeight::NORMAL
                                                                })
                                                                .child(if status > 0 {
                                                                    status.to_string()
                                                                } else {
                                                                    "-".to_string()
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(70.0))
                                                                .text_align(
                                                                    gpui::TextAlign::Right,
                                                                )
                                                                .text_color(
                                                                    ui::text_secondary(cx),
                                                                )
                                                                .child(size),
                                                        )
                                                         .child(
                                                             div()
                                                                 .w(px(62.0))
                                                                 .text_align(
                                                                     gpui::TextAlign::Right,
                                                                 )
                                                                 .text_color(
                                                                     ui::text_secondary(cx),
                                                                 )
                                                                 .child(duration),
                                                         )
                                                         .child(if is_video {
                                                             let vid_url = video_url.clone();
                                                             div()
                                                                 .id(("video-dl", ex_id as u64))
                                                                 .w(px(24.0))
                                                                 .h(px(20.0))
                                                                 .rounded(theme::radius_sm())
                                                                 .flex()
                                                                 .items_center()
                                                                 .justify_center()
                                                                 .text_size(px(12.0))
                                                                 .text_color(ui::info(cx))
                                                                 .cursor_pointer()
                                                                 .hover(|s| s.bg(theme::rgba_with_alpha(ui::info(cx).into(), 0.1)))
                                                                 .on_click(cx.listener(move |panel, _, _, cx| {
                                                                     panel.copy_text(vid_url.clone(), "已复制视频链接", cx);
                                                                 }))
                                                                 .child("⬇")
                                                                 .into_any_element()
                                                         } else {
                                                             div().w(px(0.0)).into_any_element()
                                                         })
                                                 },
                                             )),
                                    )
                                    // Pagination row
                                    .child(
                                        div()
                                            .h(px(30.0))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .gap_3()
                                            .border_t_1()
                                            .border_color(ui::border_light(cx))
                                            .text_size(px(11.0))
                                            .child({
                                                let prev_link = div()
                                                    .id("prev-page")
                                                    .text_color(if has_prev {
                                                        tokens(cx).primary
                                                    } else {
                                                        ui::text_tertiary(cx)
                                                    })
                                                    .child("上一页");
                                                if has_prev {
                                                    prev_link.cursor_pointer().on_click(
                                                        cx.listener(|panel, _, _, cx| {
                                                            panel.prev_page(cx);
                                                        }),
                                                    )
                                                } else {
                                                    prev_link
                                                }
                                            })
                                            .child(
                                                div()
                                                    .text_color(ui::text_secondary(cx))
                                                    .child(format!(
                                                        "{}–{} / {}",
                                                        offset + 1,
                                                        (offset + PAGE_SIZE).min(total),
                                                        total
                                                    )),
                                            )
                                            .child({
                                                let next_link = div()
                                                    .id("next-page")
                                                    .text_color(if has_next {
                                                        tokens(cx).primary
                                                    } else {
                                                        ui::text_tertiary(cx)
                                                    })
                                                    .child("下一页");
                                                if has_next {
                                                    next_link.cursor_pointer().on_click(
                                                        cx.listener(|panel, _, _, cx| {
                                                            panel.next_page(cx);
                                                        }),
                                                    )
                                                } else {
                                                    next_link
                                                }
                                            }),
                                    )
                                    .into_any_element()
                            }),
                    )
                    // ── Right detail panel ──
                    .child(
                        div()
                            .w(px(340.0))
                            .min_h(px(0.0))
                            .rounded(theme::radius_lg())
                            .bg(ui::bg_surface(cx))
                            .border_1()
                            .border_color(ui::border_light(cx))
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            // URL header line
                            .child(match selected_detail {
                                Some(ref detail) => div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme::http_method_color(&detail.method, dark))
                                            .font_family("SF Mono")
                                            .child(detail.method.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(11.0))
                                            .font_family("SF Mono")
                                            .text_color(ui::text_primary(cx))
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(detail.url.clone()),
                                    )
                                    .into_any_element(),
                                None => div()
                                    .text_size(px(11.0))
                                    .text_color(ui::text_tertiary(cx))
                                    .child("未选择记录")
                                    .into_any_element(),
                            })
                            .child(match selected_detail {
                                Some(ref detail) => div()
                                    .rounded(theme::radius_md())
                                    .bg(ui::bg_subtle(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .p_2()
                                    .flex()
                                    .gap_3()
                                    .text_size(px(11.0))
                                    .children(vec![
                                        detail_mini(
                                            "状态",
                                            &if detail.status > 0 {
                                                detail.status.to_string()
                                            } else {
                                                "-".to_string()
                                            },
                                            CaptureView::status_color(detail.status, cx),
                                            cx,
                                        ),
                                        detail_mini(
                                            "耗时",
                                            &detail.formatted_duration(),
                                            ui::text_primary(cx),
                                            cx,
                                        ),
                                        detail_mini(
                                            "请求",
                                            &crate::model::format_bytes(detail.request_size),
                                            ui::text_primary(cx),
                                            cx,
                                        ),
                                        detail_mini(
                                            "响应",
                                            &crate::model::format_bytes(detail.response_size),
                                            ui::text_primary(cx),
                                            cx,
                                        ),
                                    ])
                                    .into_any_element(),
                                None => div().into_any_element(),
                            })
                            // Action buttons row
                            .child(match selected_detail {
                                Some(ref detail) => {
                                    let exchange = detail.clone();
                                    div()
                                        .flex()
                                        .gap_1()
                                        .child(
                                            small_action("copy-as-curl", "复制为 cURL", cx).on_click(
                                                cx.listener(move |panel, _, _, cx| {
                                                    panel.copy_as_curl(&exchange, cx);
                                                }),
                                            ),
                                        )
                                        .into_any_element()
                                }
                                None => div().into_any_element(),
                            })
                            // Tab bar
                            .child(
                                div()
                                    .h(px(28.0))
                                    .rounded(theme::radius_sm())
                                    .bg(ui::bg_subtle(cx))
                                    .flex()
                                    .gap_px()
                                    .children(DetailTab::ALL.iter().map(|&tab| {
                                        let active = detail_tab == tab;
                                        let label = tab.label();
                                        div()
                                            .id(SharedString::from(format!("detail-tab-{label}")))
                                            .flex_1()
                                            .h(px(28.0))
                                            .rounded(theme::radius_sm())
                                            .bg(if active {
                                                if tokens(cx).is_dark() {
                                                    tokens(cx).primary
                                                } else {
                                                    Theme::global(cx).primary_hover
                                                }
                                            } else {
                                                ui::bg_subtle(cx)
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                Theme::global(cx).primary_active
                                            } else {
                                                ui::text_secondary(cx)
                                            })
                                            .font_weight(if active {
                                                gpui::FontWeight::SEMIBOLD
                                            } else {
                                                gpui::FontWeight::NORMAL
                                            })
                                            .cursor_pointer()
                                            .hover(|s| {
                                                if !active {
                                                    s.bg(ui::bg_subtle(cx))
                                                } else {
                                                    s
                                                }
                                            })
                                            .on_click(cx.listener(move |panel, _, _, cx| {
                                                panel.set_detail_tab(tab, cx);
                                            }))
                                            .child(label)
                                    })),
                            )
                            // Tab content
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .overflow_y_scrollbar()
                                    .child(match selected_detail {
                                        Some(ref detail) => {
                                            render_detail_tab_content(detail_tab, detail, cx)
                                        }
                                        None => div()
                                            .flex_1()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(12.0))
                                            .text_color(ui::text_tertiary(cx))
                                            .child("选择一条记录查看详情")
                                            .into_any_element(),
                                    }),
                            ),
                    ),
            )
            // ── Performance panel ──
            .child(if self.performance_visible {
                self.render_performance_panel(cx)
            } else {
                div().into_any_element()
            })
            // ── Composer panel ──
            .child(if composer_visible {
                let method_options = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .rounded(theme::radius_lg())
                    .bg(ui::bg_surface(cx))
                    .border_1()
                    .border_color(ui::border_light(cx))
                    .p_3()
                    // Method + URL + Send row
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(section_label("方法", cx))
                                    .child(
                                        div()
                                            .flex()
                                            .gap_px()
                                            .child(
                                                div()
                                                    .w(px(72.0))
                                                    .h(px(28.0))
                                                    .rounded(theme::radius_sm())
                                                    .bg(ui::bg_subtle(cx))
                                                    .border_1()
                                                    .border_color(ui::border_light(cx))
                                                    .px_1()
                                                    .flex()
                                                    .items_center()
                                                    .text_size(px(11.0))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(theme::http_method_color(&composer_method, dark))
                                                    .child(composer_method.clone()),
                                            )
                                            .children(method_options.iter().map(|&m| {
                                                let active = composer_method == m;
                                                let method_color = theme::http_method_color(m, dark);
                                                div()
                                                    .id(SharedString::from(format!("composer-method-{m}")))
                                                    .px_1p5()
                                                    .h(px(20.0))
                                                    .rounded(theme::radius_sm())
                                                    .bg(if active {
                                                        theme::rgba_with_alpha(method_color, 0.18)
                                                    } else {
                                                        ui::bg_subtle(cx)
                                                    })
                                                    .border_1()
                                                    .border_color(if active {
                                                        method_color
                                                    } else {
                                                        ui::border_light(cx).into()
                                                    })
                                                    .flex()
                                                    .items_center()
                                                    .text_size(px(10.0))
                                                    .text_color(if active {
                                                        method_color
                                                    } else {
                                                        ui::text_secondary(cx).into()
                                                    })
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                                        panel.set_composer_method(m.to_string(), cx);
                                                    }))
                                                    .child(m)
                                            })),
                                    ),
                            )
                            .child(div().flex_1().child(
                                Input::new(&composer_url)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .h(px(28.0))
                                    .text_size(px(11.0))
                                    .font_family(ui::font_mono())
                            ))
                            .child(
                                Button::new("composer-send-btn")
                                    .label(if composer_sending { "发送中..." } else { "发送" })
                                    .with_size(Size::Small)
                                    .with_variant(ButtonVariant::Primary)
                                    .disabled(composer_sending)
                                    .on_click(cx.listener(|panel, _, window, cx| {
                                        panel.send_composer_request(window, cx);
                                    })),
                            ),
                    )
                    // Headers
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(section_label("请求头 (每行一个，格式: Key: Value)", cx))
                            .child(
                                div()
                                    .h(px(80.0))
                                    .rounded(theme::radius_sm())
                                    .bg(ui::bg_subtle(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .p_1()
                                    .child(
                                        Input::new(&composer_headers)
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .w(px(260.0))
                                            .h(px(78.0))
                                            .text_size(px(11.0))
                                            .font_family(ui::font_mono())
                                    ),
                            ),
                    )
                    // Body
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(section_label("请求体", cx))
                            .child(
                                div()
                                    .h(px(100.0))
                                    .rounded(theme::radius_sm())
                                    .bg(ui::bg_subtle(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .p_1()
                                    .child(
                                        Input::new(&composer_body)
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .w(px(260.0))
                                            .h(px(98.0))
                                            .text_size(px(11.0))
                                            .font_family(ui::font_mono())
                                    ),
                            ),
                    )
                    // Response
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(section_label("响应", cx))
                            .child(
                                render_composer_response(composer_response, cx),
                            ),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            // ── Breakpoint panel ──
            .child(if self.breakpoint_visible {
                self.render_breakpoint_panel(cx)
            } else {
                div().into_any_element()
            })
            // ── Throttle panel ──
            .child(if self.throttle_visible {
                self.render_throttle_panel(cx)
            } else {
                div().into_any_element()
            })
            // ── Rewrite panel ──
            .child(if self.rewrite_visible {
                self.render_rewrite_panel(cx)
            } else {
                div().into_any_element()
            })
            // ── Status bar ──
            .child(ui::status_bar(
                self.status_text(),
                if notice.is_some() {
                    ui::warning(cx)
                } else if exchanges.is_empty() {
                    ui::text_tertiary(cx)
                } else {
                    ui::text_secondary(cx)
                },
                cx,
            ))
    }
}

fn detail_mini(
    key: &str,
    value: &str,
    value_color: impl Into<gpui::Hsla>,
    cx: &App,
) -> gpui::AnyElement {
    let key = key.to_string();
    let value_color: gpui::Hsla = value_color.into();
    div()
        .flex()
        .flex_col()
        .items_center()
        .text_size(px(11.0))
        .child(div().text_color(ui::text_secondary(cx)).child(key))
        .child(
            div()
                .text_color(value_color)
                .font_family("SF Mono")
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
        .into_any_element()
}

fn render_detail_tab_content(
    tab: DetailTab,
    detail: &CapturedExchange,
    cx: &App,
) -> gpui::AnyElement {
    match tab {
        DetailTab::Overview => render_overview_section(detail, cx),
        DetailTab::RequestHeaders => render_headers_section(
            "请求头",
            &detail.request_headers_entries(),
            detail.has_request_headers(),
            cx,
        ),
        DetailTab::RequestBody => render_body_section("请求体", detail.request_body_display(), cx),
        DetailTab::ResponseHeaders => render_headers_section(
            "响应头",
            &detail.response_headers_entries(),
            detail.has_response_headers(),
            cx,
        ),
        DetailTab::ResponseBody => {
            render_body_section("响应体", detail.response_body_display(), cx)
        }
        DetailTab::Timing => render_timing_section(detail, cx),
    }
}

fn render_timing_section(detail: &CapturedExchange, cx: &App) -> gpui::AnyElement {
    let rows = detail.timing_rows();
    div()
        .flex()
        .flex_col()
        .gap_px()
        .children(rows.into_iter().map(|(key, value)| {
            div()
                .flex()
                .text_size(px(11.0))
                .font_family("SF Mono")
                .p_1()
                .rounded(theme::radius_sm())
                .hover(|s| s.bg(ui::bg_subtle(cx)))
                .child(
                    div()
                        .w(px(80.0))
                        .text_color(ui::text_secondary(cx))
                        .child(key.to_string()),
                )
                .child(div().flex_1().text_color(ui::text_primary(cx)).child(value))
        }))
        .into_any_element()
}

fn render_headers_section(
    title: &str,
    entries: &[crate::model::HeaderEntry],
    has_data: bool,
    cx: &App,
) -> gpui::AnyElement {
    if !has_data || entries.is_empty() {
        return render_empty_tab(title, cx);
    }
    div()
        .flex()
        .flex_col()
        .gap_px()
        .children(entries.iter().map(|entry| {
            div()
                .flex()
                .text_size(px(11.0))
                .font_family("SF Mono")
                .p_1()
                .rounded(theme::radius_sm())
                .hover(|s| s.bg(ui::bg_subtle(cx)))
                .child(
                    div()
                        .w(px(100.0))
                        .text_color(ui::text_secondary(cx))
                        .child(entry.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(ui::text_primary(cx))
                        .child(entry.value.clone()),
                )
        }))
        .into_any_element()
}

fn render_body_section(title: &str, display: BodyDisplay, cx: &App) -> gpui::AnyElement {
    match display {
        BodyDisplay::Empty => render_empty_tab(title, cx),
        BodyDisplay::Hinted(msg) => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .p_3()
            .text_size(px(11.0))
            .text_color(ui::text_tertiary(cx))
            .child(msg)
            .into_any_element(),
        BodyDisplay::Text(body) => div()
            .flex()
            .flex_col()
            .p_1()
            .text_size(px(11.0))
            .font_family("SF Mono")
            .text_color(ui::text_primary(cx))
            .children(body.lines().map(|line| div().child(line.to_string())))
            .into_any_element(),
    }
}

fn render_overview_section(detail: &CapturedExchange, cx: &App) -> gpui::AnyElement {
    let rows: Vec<(&str, String)> = vec![
        ("方法", detail.method.clone()),
        ("URL", detail.url.clone()),
        ("Host", detail.host.clone()),
        (
            "状态",
            if detail.status > 0 {
                detail.status.to_string()
            } else {
                "-".to_string()
            },
        ),
        ("协议", detail.protocol.clone()),
        ("耗时", detail.formatted_duration()),
        ("请求大小", crate::model::format_bytes(detail.request_size)),
        ("响应大小", crate::model::format_bytes(detail.response_size)),
        ("时间", detail.timestamp.clone()),
        (
            "HTTPS",
            if detail.is_https { "是" } else { "否" }.to_string(),
        ),
    ];

    div()
        .flex()
        .flex_col()
        .gap_px()
        .children(rows.into_iter().map(|(key, value)| {
            div()
                .flex()
                .text_size(px(11.0))
                .font_family("SF Mono")
                .p_1()
                .rounded(theme::radius_sm())
                .hover(|s| s.bg(ui::bg_subtle(cx)))
                .child(
                    div()
                        .w(px(80.0))
                        .text_color(ui::text_secondary(cx))
                        .child(key.to_string()),
                )
                .child(div().flex_1().text_color(ui::text_primary(cx)).child(value))
        }))
        .into_any_element()
}

fn render_empty_tab(label: &str, cx: &App) -> gpui::AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(ui::text_tertiary(cx))
        .child(format!("{label}无数据"))
        .into_any_element()
}

fn render_composer_response(
    composer_response: Option<ComposedResponse>,
    cx: &App,
) -> gpui::AnyElement {
    match composer_response {
        Some(resp) => {
            let status_color: gpui::Rgba = if resp.status >= 500 {
                tokens(cx).danger.into()
            } else if resp.status >= 400 {
                tokens(cx).warning.into()
            } else if resp.status >= 200 {
                tokens(cx).success.into()
            } else {
                ui::text_secondary(cx).into()
            };
            let resp_headers: Vec<(String, String)> = resp.headers.clone();
            let resp_body = resp.body.clone();
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .text_size(px(11.0))
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(status_color)
                                .child(format!("{} ", resp.status)),
                        )
                        .child(
                            div()
                                .text_color(ui::text_secondary(cx))
                                .child(format!("{}ms", resp.duration_ms)),
                        ),
                )
                .child(
                    div()
                        .h(px(60.0))
                        .rounded(theme::radius_sm())
                        .bg(ui::bg_subtle(cx))
                        .border_1()
                        .border_color(ui::border_light(cx))
                        .p_1()
                        .overflow_y_scrollbar()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(ui::font_mono())
                                .text_color(ui::text_secondary(cx))
                                .children(resp_headers.iter().map(|(k, v)| {
                                    div().child(format!("{k}: {v}"))
                                })),
                        ),
                )
                .child(
                    div()
                        .h(px(120.0))
                        .rounded(theme::radius_sm())
                        .bg(ui::bg_subtle(cx))
                        .border_1()
                        .border_color(ui::border_light(cx))
                        .p_1()
                        .overflow_y_scrollbar()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(ui::font_mono())
                                .text_color(ui::text_primary(cx))
                                .children(resp_body.lines().map(|line| {
                                    div().child(line.to_string())
                                })),
                        ),
                )
                .into_any_element()
        }
        None => div()
            .h(px(80.0))
            .rounded(theme::radius_sm())
            .bg(ui::bg_subtle(cx))
            .border_1()
            .border_color(ui::border_light(cx))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .text_color(ui::text_tertiary(cx))
            .child("发送请求后，响应将显示在这里")
            .into_any_element(),
    }
}
