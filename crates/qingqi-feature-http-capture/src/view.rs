use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    certificate::CaManager,
    engine::CaptureEngine,
    manifest,
    mock_store::MockStore,
    model::{
        BodyDisplay, CaptureEndpoint, CaptureSetupInfo, CapturedExchange, CertificateStatus,
        DetailTab, FilterState,
    },
    store::CaptureStore,
};
use gpui::{
    App, AppContext, ClipboardItem, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
    Window, div, px,
};
use gpui_component::theme::Theme;
use gpui_component::scroll::ScrollableElement;
use qingqi_plugin::{
    events::{AppEventBus, AppEventKind},
    plugin_spec::PluginAccent,
};
use qingqi_ui::{
    text_input::{TextInput, TextInputStyle},
    theme,
    ui::{self, components},
};

const PAGE_SIZE: i64 = 50;
const DEFAULT_PROXY_PORT: u16 = 8899;

pub struct CaptureView {
    store: Arc<Mutex<CaptureStore>>,
    engine: Arc<CaptureEngine>,
    ca_manager: Arc<Mutex<CaManager>>,
    search_input: Entity<TextInput>,
    host_input: Entity<TextInput>,
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
        let search_input = cx.new(|cx| {
            let mut input = TextInput::new(cx, "搜索 URL 关键词", "");
            input.set_style(
                TextInputStyle {
                    height: 32.0,
                    font_size: 12.0,
                    padding: 8.0,
                },
                cx,
            );
            input.set_chrome(false, cx);
            input
        });
        let host_input = cx.new(|cx| {
            let mut input = TextInput::new(cx, "Host 过滤", "");
            input.set_style(
                TextInputStyle {
                    height: 32.0,
                    font_size: 12.0,
                    padding: 8.0,
                },
                cx,
            );
            input.set_chrome(false, cx);
            input
        });

        let setup_info = build_setup_info(&engine, &ca_manager, DEFAULT_PROXY_PORT);
        let mut this = Self {
            store,
            engine,
            ca_manager,
            search_input,
            host_input,
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
        };
        this.observe_inputs(cx);
        this.start_event_watch(events, cx);
        this.refresh_from_store(cx);
        this
    }

    fn observe_inputs(&mut self, cx: &mut Context<Self>) {
        let search = self.search_input.clone();
        let sub = cx.observe(&search, |panel, _, cx| {
            panel.filter.search = panel.search_input.read(cx).text();
            panel.offset = 0;
            panel.refresh_from_store(cx);
        });
        self.subscriptions.push(sub);

        let host = self.host_input.clone();
        let sub = cx.observe(&host, |panel, _, cx| {
            panel.filter.host = panel.host_input.read(cx).text();
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
        self.search_input.update(cx, |input, cx| {
            input.clear(cx);
        });
        self.host_input.update(cx, |input, cx| {
            input.clear(cx);
        });
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
            Theme::global(cx).danger.into()
        } else if status >= 400 {
            Theme::global(cx).warning.into()
        } else if status >= 300 {
            Theme::global(cx).info.into()
        } else if status >= 200 {
            Theme::global(cx).success.into()
        } else {
            Theme::global(cx).muted_foreground.into()
        }
    }
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
    action: gpui::Stateful<gpui::Div>,
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

fn small_action(id: &'static str, label: &str, cx: &App) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(24.0))
        .px_2()
        .rounded(theme::radius_sm())
        .bg(ui::bg_surface(cx))
        .border_1()
        .border_color(ui::border_light(cx))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(ui::text_primary(cx))
        .cursor_pointer()
        .hover(|s| s.bg(ui::bg_hover(cx)))
        .child(label.to_string())
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let exchanges = self.exchanges.clone();
        let total = self.total;
        let selected_id = self.selected_id;
        let selected_detail = self.selected_detail.clone();
        let offset = self.offset;
        let engine_running = self.engine_running;
        let search_input = self.search_input.clone();
        let host_input = self.host_input.clone();
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
                        ui::ui_button("停止代理", "secondary", dark, None, true, cx)
                            .id("stop-proxy-btn")
                            .cursor_pointer()
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.stop_proxy(cx);
                            }))
                    } else {
                        ui::ui_button("启动代理", "primary", dark, None, false, cx)
                            .id("start-proxy-btn")
                            .cursor_pointer()
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.start_proxy(cx);
                            }))
                    })
                    .child({
                        let reset_btn = ui::ui_button("重置过滤", "ghost", dark, None, false, cx)
                            .id("reset-filter-btn");
                        if has_active_filter {
                            reset_btn
                                .cursor_pointer()
                                .on_click(cx.listener(|panel, _, _, cx| {
                                    panel.reset_filters(cx);
                                }))
                        } else {
                            reset_btn.opacity(0.4)
                        }
                    })
                    .child({
                        let clear_btn =
                            ui::ui_button("清空记录", "ghost", dark, None, true, cx).id("clear-btn");
                        if total > 0 {
                            clear_btn
                                .cursor_pointer()
                                .on_click(cx.listener(|panel, _, _, cx| {
                                    panel.clear_all(cx);
                                }))
                        } else {
                            clear_btn.opacity(0.4)
                        }
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
                                                Theme::global(cx).primary.into(),
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
                            .child(ui::separator(cx))
                            .child(
                                div()
                                    .rounded(theme::radius_md())
                                    .bg(ui::bg_subtle(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .child(search_input.clone()),
                            )
                            .child(
                                div()
                                    .rounded(theme::radius_md())
                                    .bg(ui::bg_subtle(cx))
                                    .border_1()
                                    .border_color(ui::border_light(cx))
                                    .child(host_input.clone()),
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
                            .child(ui::separator(cx))
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
                    // ── Center: exchange list ──
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
                                    ),
                            )
                            // List or empty state
                            .child(if exchanges.is_empty() {
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

                                                    div()
                                                        .id(("exchange-row", ex_id as u64))
                                                        .h(px(32.0))
                                                        .px_3()
                                                        .bg(if selected {
                                                            Theme::global(cx).primary
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
                                                        Theme::global(cx).primary
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
                                                        Theme::global(cx).primary
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
                                                if Theme::global(cx).is_dark() {
                                                    Theme::global(cx).primary
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

fn detail_mini(key: &str, value: &str, value_color: impl Into<gpui::Hsla>, cx: &App) -> gpui::AnyElement {
    let key = key.to_string();
    let value_color: gpui::Hsla = value_color.into();
    div()
        .flex()
        .flex_col()
        .items_center()
        .text_size(px(11.0))
        .child(
            div()
                .text_color(ui::text_secondary(cx))
                .child(key),
        )
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
        DetailTab::RequestBody => {
            render_body_section("请求体", detail.request_body_display(), cx)
        }
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
                .child(
                    div()
                        .flex_1()
                        .text_color(ui::text_primary(cx))
                        .child(value),
                )
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
                .child(
                    div()
                        .flex_1()
                        .text_color(ui::text_primary(cx))
                        .child(value),
                )
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
