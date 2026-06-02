use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

use qingqi_ui::{theme, ui};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AboutView;

impl Render for AboutView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(theme::semantic().bg_page)
            .font_family(ui::font_ui())
            .text_color(theme::semantic().text_primary)
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .rounded(px(16.0))
                            .overflow_hidden()
                            .child(ui::icon_element(
                                "app-icon.svg",
                                ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Amber),
                                72.0,
                            )),
                    )
                    // ── App name ──────────────────────────────────────────
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("Qingqi"),
                    )
                    // ── Version ───────────────────────────────────────────
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme::semantic().text_body)
                            .child(format!("版本 {APP_VERSION}")),
                    )
                    // ── Tech stack ────────────────────────────────────────
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::semantic().text_secondary)
                            .font_family("SF Mono")
                            .child("Rust + GPUI"),
                    )
                    // ── Description card (420x44, matching suishou) ───────
                    .child(
                        div()
                            .w(px(420.0))
                            .h(px(44.0))
                            .rounded(px(10.0))
                            .bg(theme::semantic().bg_surface)
                            .border_1()
                            .border_color(theme::semantic().border_default)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::semantic().text_body)
                                    .child("基于 Rust + GPUI 的桌面工具箱"),
                            ),
                    )
                    // ── Tech stack card ────────────────────────────────────
                    .child(section_card(
                        "技术栈",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(tech_row("UI 框架", "GPUI 0.2.2"))
                            .child(tech_row("渲染后端", "macos-blade (Metal)"))
                            .child(tech_row("数据库", "SQLite (rusqlite)"))
                            .child(tech_row("序列化", "serde / serde_json"))
                            .child(tech_row("日志", "tracing / tracing-subscriber")),
                    ))
                    // ── Architecture card ──────────────────────────────────
                    .child(section_card(
                        "架构概览",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(desc_row(
                                "插件系统",
                                "运行时/会话两层架构，插件静态注册，按需创建窗口",
                            ))
                            .child(desc_row(
                                "命令系统",
                                "统一搜索评分引擎，覆盖插件命令与应用索引",
                            ))
                            .child(desc_row("平台抽象", "剪贴板、文件系统、进程管理抽象层"))
                            .child(desc_row(
                                "主题系统",
                                "设计令牌驱动的亮色/暗色主题，35+ 语义色",
                            )),
                    ))
                    // ── Status bar ─────────────────────────────────────────
                    .child(ui::status_bar(
                        format!("Qingqi v{APP_VERSION} · Rust + GPUI"),
                        theme::semantic().text_secondary,
                    )),
            )
    }
}

fn section_card(title: &'static str, children: impl IntoElement) -> impl IntoElement {
    div()
        .w(px(420.0))
        .rounded(px(10.0))
        .bg(theme::semantic().bg_surface)
        .border_1()
        .border_color(theme::semantic().border_default)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::accent_color(
                    qingqi_plugin::plugin_spec::PluginAccent::Amber,
                ))
                .child(title),
        )
        .child(children)
}

fn tech_row(label: &'static str, value: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme::semantic().text_body)
                .child(label),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_family("SF Mono")
                .text_color(theme::semantic().text_primary)
                .child(value),
        )
}

fn desc_row(label: &'static str, desc: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_primary)
                .child(label),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::semantic().text_body)
                .child(desc),
        )
}
