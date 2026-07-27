use std::sync::Arc;

use gpui::*;
use gpui::prelude::FluentBuilder;
use qingqi_plugin::database::DatabaseService;
use qingqi_ui::components::button::Button;
use qingqi_ui::components::styled::{h_flex, v_flex};

use crate::server::{AppState, RemoteServer};
use crate::service::{PairedDevice, RemoteControlService};

pub struct RemoteControlView {
    service: Arc<RemoteControlService>,
    database: Arc<DatabaseService>,
    pin: Option<String>,
    server_running: bool,
    ip_address: String,
    paired_devices: Vec<PairedDevice>,
}

impl RemoteControlView {
    pub fn new(service: Arc<RemoteControlService>, database: Arc<DatabaseService>) -> Self {
        let paired_devices = service.list_paired_devices(&database);
        Self {
            service,
            database,
            pin: None,
            server_running: false,
            ip_address: get_local_ip(),
            paired_devices,
        }
    }

    pub fn init(&mut self, _cx: &mut Context<Self>) {
        self.server_running = self.service.is_server_running();
        self.refresh_paired_devices();
    }

    fn generate_pin(&mut self, cx: &mut Context<Self>) {
        let pin = self.service.generate_pin();
        self.pin = Some(pin);
        cx.notify();
    }

    fn refresh_paired_devices(&mut self) {
        self.paired_devices = self.service.list_paired_devices(&self.database);
    }

    #[allow(dead_code)]
    fn revoke_device(&mut self, name: String, cx: &mut Context<Self>) {
        if self.service.revoke_device(&name, &self.database) {
            self.refresh_paired_devices();
            cx.notify();
        }
    }

    fn start_server(&mut self, cx: &mut Context<Self>) {
        let port = 3721;
        tracing::info!("[远程控制] 用户点击启动服务器，端口: {}", port);
        let state = AppState::new((*self.service).clone(), Arc::clone(&self.database));

        // Use the shared Tokio runtime to spawn the server task
        qingqi_core::tokio_runtime::spawn(async move {
            match RemoteServer::run(state, port).await {
                Ok((addr, server_handle)) => {
                    tracing::info!("[远程控制] 服务器启动成功: {}", addr);
                    // Keep the server running until the handle is dropped or aborted
                    let _ = server_handle.await;
                    tracing::warn!("[远程控制] 服务器任务已结束");
                }
                Err(e) => {
                    tracing::error!("[远程控制] 启动失败: {}", e);
                }
            }
        });

        self.server_running = true;
        self.service.set_server_running(true, port);
        cx.notify();
    }

    fn stop_server(&mut self, cx: &mut Context<Self>) {
        tracing::info!("[远程控制] 用户点击停止服务器");
        self.server_running = false;
        self.service.set_server_running(false, 0);
        cx.notify();
    }
}

impl Render for RemoteControlView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(self.render_header())
            .child(self.render_status())
            .child(self.render_actions())
            .child(self.render_pin_section(cx))
    }
}

impl RemoteControlView {
    fn render_header(&self) -> impl IntoElement {
        h_flex()
            .items_center()
            .gap_2()
            .child(div().text_xl().font_weight(FontWeight::BOLD).child("远程控制"))
    }

    fn render_status(&self) -> impl IntoElement {
        let status_color = if self.server_running {
            gpui::green()
        } else {
            gpui::red()
        };
        let status_text = if self.server_running { "运行中" } else { "已停止" };

        v_flex()
            .gap_2()
            .p_3()
            .bg(gpui::rgba(0x1e1e1e))
            .rounded_md()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().w_3().h_3().rounded_full().bg(status_color))
                    .child(div().text_sm().child(status_text)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(gpui::rgba(0x888888))
                    .child(format!("IP: {}  端口: {}", self.ip_address, 3721)),
            )
    }

    fn render_actions(&self) -> impl IntoElement {
        let svc = Arc::clone(&self.service);
        let svc2 = Arc::clone(&self.service);
        let svc3 = Arc::clone(&self.service);
        let svc4 = Arc::clone(&self.service);

        h_flex()
            .gap_2()
            .child(
                Button::new("btn-shutdown")
                    .label("关机")
                    .compact()
                    .on_click(move |_, _, _cx| {
                        let _ = svc.system_service().shutdown(false, 0);
                    }),
            )
            .child(
                Button::new("btn-sleep")
                    .label("睡眠")
                    .compact()
                    .on_click(move |_, _, _cx| {
                        let _ = svc2.system_service().sleep(false);
                    }),
            )
            .child(
                Button::new("btn-restart")
                    .label("重启")
                    .compact()
                    .on_click(move |_, _, _cx| {
                        let _ = svc3.system_service().restart(false);
                    }),
            )
            .child(
                Button::new("btn-lock")
                    .label("锁屏")
                    .compact()
                    .on_click(move |_, _, _cx| {
                        let _ = svc4.system_service().lock();
                    }),
            )
    }

    fn render_pin_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_pin = self.pin.is_some();
        let pin_text = self.pin.clone().unwrap_or_default();
        let has_paired = !self.paired_devices.is_empty();

        v_flex()
            .gap_3()
            .p_3()
            .bg(gpui::rgba(0x1e1e1e))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("设备配对"),
            )
            .when(has_paired, |this| {
                this.child(self.render_paired_devices(cx))
            })
            .when(!has_pin, |this| {
                this.child(
                    Button::new("btn-gen-pin")
                        .label(if has_paired { "配对新设备" } else { "生成配对 PIN" })
                        .compact()
                        .on_click(cx.listener(|view, _, _window, cx| {
                            view.generate_pin(cx);
                        })),
                )
            })
            .when(has_pin, |this| {
                this.child(
                    v_flex()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_decoration_1()
                                .child(pin_text),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(gpui::rgba(0x888888))
                                .child("请在手机上输入此 PIN 码"),
                        )
                        .child(
                            Button::new("btn-cancel-pin")
                                .label("取消")
                                .compact()
                                .on_click(cx.listener(|view, _, _window, cx| {
                                    view.pin = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(!self.server_running, |this| {
                this.child(
                    Button::new("btn-start")
                        .label("启动服务器")
                        .compact()
                        .on_click(cx.listener(|view, _, _window, cx| {
                            view.start_server(cx);
                        })),
                )
            })
            .when(self.server_running, |this| {
                this.child(
                    Button::new("btn-stop")
                        .label("停止服务器")
                        .compact()
                        .on_click(cx.listener(|view, _, _window, cx| {
                            view.stop_server(cx);
                        })),
                )
            })
    }

    fn render_paired_devices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let devices: Vec<_> = self
            .paired_devices
            .iter()
            .enumerate()
            .map(|(idx, device)| {
                let name = device.name.clone();
                let expires_at = device.expires_at;
                let paired_at = device.paired_at;
                let svc = Arc::clone(&self.service);
                let database = Arc::clone(&self.database);
                let entity_id = cx.entity_id();

                v_flex()
                    .gap_1()
                    .p_2()
                    .bg(gpui::rgba(0x2a2a2a))
                    .rounded_md()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(device.name.clone()),
                            )
                            .child(
                                Button::new(("btn-revoke", idx as u64))
                                    .label("移除")
                                    .compact()
                                    .on_click(move |_, _, _cx| {
                                        svc.revoke_device(&name, &database);
                                        _cx.notify(entity_id);
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgba(0x888888))
                            .child(format!(
                                "配对时间: {} · 过期时间: {}",
                                format_time(paired_at),
                                format_time(expires_at)
                            )),
                    )
            })
            .collect();

        v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(gpui::rgba(0x888888))
                    .child(format!("已配对设备 ({})", self.paired_devices.len())),
            )
            .children(devices)
    }
}

fn format_time(timestamp: i64) -> String {
    use time::OffsetDateTime;
    match OffsetDateTime::from_unix_timestamp(timestamp) {
        Ok(dt) => {
            let date = dt.date();
            let time = dt.time();
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                date.year(),
                date.month() as u8,
                date.day(),
                time.hour(),
                time.minute()
            )
        }
        Err(_) => "未知".to_string(),
    }
}

fn get_local_ip() -> String {
    use local_ip_address::local_ip;
    local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
