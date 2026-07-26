use std::sync::Arc;

use gpui::*;
use gpui::prelude::FluentBuilder;
use qingqi_ui::components::button::Button;
use qingqi_ui::components::styled::{h_flex, v_flex};

use crate::service::RemoteControlService;

pub struct RemoteControlView {
    service: Arc<RemoteControlService>,
    pin: Option<String>,
    server_running: bool,
    ip_address: String,
}

impl RemoteControlView {
    pub fn new(service: Arc<RemoteControlService>) -> Self {
        Self {
            service,
            pin: None,
            server_running: false,
            ip_address: get_local_ip(),
        }
    }

    pub fn init(&mut self, _cx: &mut Context<Self>) {
        self.server_running = self.service.is_server_running();
    }

    fn generate_pin(&mut self, cx: &mut Context<Self>) {
        let pin = self.service.generate_pin();
        self.pin = Some(pin);
        cx.notify();
    }

    fn start_server(&mut self, cx: &mut Context<Self>) {
        self.server_running = true;
        self.service.set_server_running(true, 3721);
        cx.notify();
    }

    fn stop_server(&mut self, cx: &mut Context<Self>) {
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
            .child(
                div()
                    .text_xs()
                    .text_color(gpui::rgba(0x888888))
                    .child("点击生成 PIN 码，在手机上输入以完成配对"),
            )
            .when(!has_pin, |this| {
                this.child(
                    Button::new("btn-gen-pin")
                        .label("生成配对 PIN")
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
}

fn get_local_ip() -> String {
    use local_ip_address::local_ip;
    local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
