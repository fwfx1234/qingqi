use std::rc::Rc;

use crate::{
    app::{
        text_input::{TextInput, TextInputStyle},
        theme, ui,
    },
    core::plugin_spec::PluginAccent,
    features::http_capture::{
        model::{BodyDisplay, CapturedExchange, DetailTab, FilterState},
        store::CaptureStore,
    },
};
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};

const PAGE_SIZE: i64 = 50;

pub struct CapturePanel {
    store: Rc<CaptureStore>,
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
    notice: Option<String>,
    subscriptions: Vec<Subscription>,
}

impl CapturePanel {
    pub fn new(store: Rc<CaptureStore>, cx: &mut Context<Self>) -> Self {
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

        let mut this = Self {
            store,
            search_input,
            host_input,
            filter: FilterState::default(),
            exchanges: Vec::new(),
            total: 0,
            selected_id: None,
            selected_detail: None,
            detail_tab: DetailTab::Overview,
            offset: 0,
            engine_running: false,
            notice: None,
            subscriptions: Vec::new(),
        };
        this.observe_inputs(cx);
        this.refresh_from_store();
        this
    }

    fn observe_inputs(&mut self, cx: &mut Context<Self>) {
        let search = self.search_input.clone();
        let sub = cx.observe(&search, |panel, _, cx| {
            panel.filter.search = panel.search_input.read(cx).text();
            panel.offset = 0;
            panel.refresh_from_store();
            cx.notify();
        });
        self.subscriptions.push(sub);

        let host = self.host_input.clone();
        let sub = cx.observe(&host, |panel, _, cx| {
            panel.filter.host = panel.host_input.read(cx).text();
            panel.offset = 0;
            panel.refresh_from_store();
            cx.notify();
        });
        self.subscriptions.push(sub);
    }

    fn refresh_from_store(&mut self) {
        match self.store.query(&self.filter, self.offset, PAGE_SIZE) {
            Ok(rows) => {
                // Apply hide_static in-memory (extension-based filtering can't be
                // pushed to SQL cleanly).
                if self.filter.hide_static {
                    self.exchanges = rows
                        .into_iter()
                        .filter(|ex| self.filter.matches(ex))
                        .collect();
                } else {
                    self.exchanges = rows;
                }
            }
            Err(e) => {
                self.exchanges.clear();
                self.notice = Some(format!("查询失败: {e}"));
            }
        }
        match self.store.count(&self.filter) {
            Ok(n) => self.total = n,
            Err(e) => {
                self.total = 0;
                self.notice = Some(format!("计数失败: {e}"));
            }
        }
        self.selected_id = None;
        self.selected_detail = None;
    }

    fn select_exchange(&mut self, id: i64, cx: &mut Context<Self>) {
        self.selected_id = Some(id);
        match self.store.get_by_id(id) {
            Ok(detail) => self.selected_detail = detail,
            Err(e) => {
                self.notice = Some(format!("读取详情失败: {e}"));
            }
        }
        cx.notify();
    }

    fn clear_all(&mut self, cx: &mut Context<Self>) {
        match self.store.clear() {
            Ok(_) => {
                self.notice = Some(String::from("已清空所有抓包记录"));
            }
            Err(e) => {
                self.notice = Some(format!("清空失败: {e}"));
            }
        }
        self.refresh_from_store();
        cx.notify();
    }

    fn toggle_method_filter(&mut self, method: &str, cx: &mut Context<Self>) {
        if self.filter.method == method {
            self.filter.method.clear();
        } else {
            self.filter.method = method.to_string();
        }
        self.offset = 0;
        self.refresh_from_store();
        cx.notify();
    }

    fn toggle_error_only(&mut self, cx: &mut Context<Self>) {
        self.filter.error_only = !self.filter.error_only;
        self.offset = 0;
        self.refresh_from_store();
        cx.notify();
    }

    fn toggle_https_only(&mut self, cx: &mut Context<Self>) {
        self.filter.https_only = !self.filter.https_only;
        self.offset = 0;
        self.refresh_from_store();
        cx.notify();
    }

    fn toggle_hide_static(&mut self, cx: &mut Context<Self>) {
        self.filter.hide_static = !self.filter.hide_static;
        self.offset = 0;
        self.refresh_from_store();
        cx.notify();
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
        self.refresh_from_store();
        cx.notify();
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if self.offset + PAGE_SIZE < self.total {
            self.offset += PAGE_SIZE;
            self.refresh_from_store();
            cx.notify();
        }
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.offset > 0 {
            self.offset = (self.offset - PAGE_SIZE).max(0);
            self.refresh_from_store();
            cx.notify();
        }
    }

    fn status_text(&self) -> String {
        if let Some(ref notice) = self.notice {
            return notice.clone();
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

    fn method_color(dark: bool, method: &str) -> gpui::Rgba {
        match method.to_uppercase().as_str() {
            "GET" => theme::token("color-success", dark),
            "POST" => theme::token("color-info", dark),
            "PUT" => theme::token("color-warning", dark),
            "DELETE" => theme::token("color-danger", dark),
            "PATCH" => theme::token("color-warning", dark),
            _ => theme::token("color-text-secondary", dark),
        }
    }

    fn status_color(dark: bool, status: i64) -> gpui::Rgba {
        if status >= 500 {
            theme::token("color-danger", dark)
        } else if status >= 400 {
            theme::token("color-warning", dark)
        } else if status >= 300 {
            theme::token("color-info", dark)
        } else if status >= 200 {
            theme::token("color-success", dark)
        } else {
            theme::token("color-text-secondary", dark)
        }
    }
}

impl Render for CapturePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = crate::app::theme_mode::is_dark();
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
            .bg(theme::token("color-bg-page", dark))
            .text_color(theme::token("color-text-primary", dark))
            .font_family("PingFang SC")
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            // ── Header ──
            .child(
                div()
                    .h(px(54.0))
                    .rounded(theme::radius_lg())
                    .bg(theme::token("color-bg-surface", dark))
                    .border_1()
                    .border_color(theme::token("color-border-default", dark))
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
                                if dark {
                                    theme::token("color-success", dark)
                                } else {
                                    theme::token("color-success", dark)
                                }
                            } else {
                                theme::token("color-bg-subtle", dark)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(16.0))
                            .child("📡"),
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
                                    .child("HTTP 抓包"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::token("color-text-secondary", dark))
                                    .child(if engine_running {
                                        "代理引擎运行中"
                                    } else {
                                        "捕获引擎未接入 — 仅可浏览已存储的抓包记录"
                                    }),
                            ),
                    )
                    .child(div().flex_1())
                    .child({
                        let reset_btn = ui::ui_button("重置过滤", "ghost", dark, None, false)
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
                            ui::ui_button("清空记录", "ghost", dark, None, true).id("clear-btn");
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
                    // ── Left filter panel ──
                    .child(
                        div()
                            .w(px(220.0))
                            .rounded(theme::radius_lg())
                            .bg(theme::token("color-bg-surface", dark))
                            .border_1()
                            .border_color(theme::token("color-border-default", dark))
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme::token("color-text-secondary", dark))
                                    .child("过滤器"),
                            )
                            .child(
                                div()
                                    .rounded(theme::radius_md())
                                    .bg(theme::token("color-bg-subtle", dark))
                                    .border_1()
                                    .border_color(theme::token("color-border-default", dark))
                                    .child(search_input.clone()),
                            )
                            .child(
                                div()
                                    .rounded(theme::radius_md())
                                    .bg(theme::token("color-bg-subtle", dark))
                                    .border_1()
                                    .border_color(theme::token("color-border-default", dark))
                                    .child(host_input.clone()),
                            )
                            // Method filter chips
                            .child(div().flex().flex_wrap().gap_1().children(
                                ["GET", "POST", "PUT", "DELETE"].iter().map(|&m| {
                                    let active = filter_method == m;
                                    let color = CapturePanel::method_color(dark, m);
                                    let chip_bg: gpui::Hsla = if active {
                                        theme::rgba_with_alpha(color, 0.18)
                                    } else {
                                        theme::rgba_with_alpha(
                                            theme::token("color-bg-subtle", dark),
                                            1.0,
                                        )
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
                                            theme::token("color-border-default", dark)
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(11.0))
                                        .text_color(if active {
                                            color
                                        } else {
                                            theme::token("color-text-secondary", dark)
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
                                        let color = theme::token("color-danger", dark);
                                        let chip_bg: gpui::Hsla = if active {
                                            theme::rgba_with_alpha(color, 0.18)
                                        } else {
                                            theme::rgba_with_alpha(
                                                theme::token("color-bg-subtle", dark),
                                                1.0,
                                            )
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
                                                theme::token("color-border-default", dark)
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                color
                                            } else {
                                                theme::token("color-text-secondary", dark)
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|panel, _, _, cx| {
                                                panel.toggle_error_only(cx);
                                            }))
                                            .child("错误")
                                    })
                                    .child({
                                        let active = filter_https_only;
                                        let color = theme::token("color-success", dark);
                                        let chip_bg: gpui::Hsla = if active {
                                            theme::rgba_with_alpha(color, 0.18)
                                        } else {
                                            theme::rgba_with_alpha(
                                                theme::token("color-bg-subtle", dark),
                                                1.0,
                                            )
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
                                                theme::token("color-border-default", dark)
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                color
                                            } else {
                                                theme::token("color-text-secondary", dark)
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|panel, _, _, cx| {
                                                panel.toggle_https_only(cx);
                                            }))
                                            .child("HTTPS")
                                    })
                                    .child({
                                        let active = filter_hide_static;
                                        let color = theme::token("color-warning", dark);
                                        let chip_bg: gpui::Hsla = if active {
                                            theme::rgba_with_alpha(color, 0.18)
                                        } else {
                                            theme::rgba_with_alpha(
                                                theme::token("color-bg-subtle", dark),
                                                1.0,
                                            )
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
                                                theme::token("color-border-default", dark)
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                color
                                            } else {
                                                theme::token("color-text-secondary", dark)
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|panel, _, _, cx| {
                                                panel.toggle_hide_static(cx);
                                            }))
                                            .child("隐藏静态")
                                    }),
                            )
                            .child(ui::separator())
                            // Stats
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme::token("color-text-secondary", dark))
                                    .child("统计"),
                            )
                            .child(ui::metric_pill(
                                "总计",
                                format!("{total}"),
                                PluginAccent::Cyan,
                            ))
                            .child(ui::metric_pill(
                                "当前页",
                                format!("{page_count}"),
                                PluginAccent::Blue,
                            )),
                    )
                    // ── Center: exchange list ──
                    .child(
                        div()
                            .flex_1()
                            .rounded(theme::radius_lg())
                            .bg(theme::token("color-bg-surface", dark))
                            .border_1()
                            .border_color(theme::token("color-border-default", dark))
                            .flex()
                            .flex_col()
                            // Table header
                            .child(
                                div()
                                    .h(px(30.0))
                                    .px_3()
                                    .bg(theme::token("color-table-header", dark))
                                    .rounded_t(theme::radius_lg())
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(11.0))
                                    .text_color(theme::token("color-text-secondary", dark))
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
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(ui::ui_empty_state(
                                        if has_active_filter {
                                            "当前过滤条件无匹配记录"
                                        } else {
                                            "暂无抓包记录 — 请先接入代理捕获引擎"
                                        },
                                        dark,
                                    ))
                                    .into_any_element()
                            } else {
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .children(exchanges.iter().enumerate().map(|(i, ex)| {
                                        let selected = selected_id == Some(ex.id);
                                        let ex_id = ex.id;
                                        let method_color =
                                            CapturePanel::method_color(dark, &ex.method);
                                        let status_color =
                                            CapturePanel::status_color(dark, ex.status);
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
                                                theme::token("color-primary-bg", dark)
                                            } else if i % 2 == 0 {
                                                theme::token("color-bg-surface", dark)
                                            } else {
                                                theme::token("color-bg-subtle-2", dark)
                                            })
                                            .hover(|s| {
                                                s.bg(theme::token("color-row-hover", dark))
                                                    .cursor_pointer()
                                            })
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
                                                    .w(px(58.0))
                                                    .text_color(theme::token(
                                                        "color-text-secondary",
                                                        dark,
                                                    ))
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
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child(method),
                                            )
                                            .child(
                                                div()
                                                    .w(px(130.0))
                                                    .text_color(theme::token(
                                                        "color-text-primary",
                                                        dark,
                                                    ))
                                                    .overflow_hidden()
                                                    .text_ellipsis()
                                                    .child(host),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_color(theme::token(
                                                        "color-text-primary",
                                                        dark,
                                                    ))
                                                    .overflow_hidden()
                                                    .text_ellipsis()
                                                    .child(url),
                                            )
                                            .child(
                                                div()
                                                    .w(px(48.0))
                                                    .text_align(gpui::TextAlign::Right)
                                                    .text_color(if status > 0 {
                                                        status_color
                                                    } else {
                                                        theme::token("color-text-secondary", dark)
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
                                                    .text_align(gpui::TextAlign::Right)
                                                    .text_color(theme::token(
                                                        "color-text-secondary",
                                                        dark,
                                                    ))
                                                    .child(size),
                                            )
                                            .child(
                                                div()
                                                    .w(px(62.0))
                                                    .text_align(gpui::TextAlign::Right)
                                                    .text_color(theme::token(
                                                        "color-text-secondary",
                                                        dark,
                                                    ))
                                                    .child(duration),
                                            )
                                    }))
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
                                            .border_color(theme::token(
                                                "color-border-default",
                                                dark,
                                            ))
                                            .text_size(px(11.0))
                                            .child({
                                                let prev_link = div()
                                                    .id("prev-page")
                                                    .text_color(if has_prev {
                                                        theme::token("color-primary", dark)
                                                    } else {
                                                        theme::token("color-text-tertiary", dark)
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
                                                    .text_color(theme::token(
                                                        "color-text-secondary",
                                                        dark,
                                                    ))
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
                                                        theme::token("color-primary", dark)
                                                    } else {
                                                        theme::token("color-text-tertiary", dark)
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
                            .rounded(theme::radius_lg())
                            .bg(theme::token("color-bg-surface", dark))
                            .border_1()
                            .border_color(theme::token("color-border-default", dark))
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
                                            .text_color(CapturePanel::method_color(
                                                dark,
                                                &detail.method,
                                            ))
                                            .font_family("SF Mono")
                                            .child(detail.method.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(11.0))
                                            .font_family("SF Mono")
                                            .text_color(theme::token("color-text-primary", dark))
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(detail.url.clone()),
                                    )
                                    .into_any_element(),
                                None => div()
                                    .text_size(px(11.0))
                                    .text_color(theme::token("color-text-tertiary", dark))
                                    .child("未选择记录")
                                    .into_any_element(),
                            })
                            .child(match selected_detail {
                                Some(ref detail) => div()
                                    .rounded(theme::radius_md())
                                    .bg(theme::token("color-bg-subtle-2", dark))
                                    .border_1()
                                    .border_color(theme::token("color-border-default", dark))
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
                                            CapturePanel::status_color(dark, detail.status),
                                        ),
                                        detail_mini(
                                            "耗时",
                                            &detail.formatted_duration(),
                                            theme::token("color-text-primary", dark),
                                        ),
                                        detail_mini(
                                            "请求",
                                            &crate::features::http_capture::model::format_bytes(
                                                detail.request_size,
                                            ),
                                            theme::token("color-text-primary", dark),
                                        ),
                                        detail_mini(
                                            "响应",
                                            &crate::features::http_capture::model::format_bytes(
                                                detail.response_size,
                                            ),
                                            theme::token("color-text-primary", dark),
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
                                    .bg(theme::token("color-bg-subtle", dark))
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
                                                theme::token("color-nav-active-bg", dark)
                                            } else {
                                                theme::token("color-bg-subtle", dark)
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(11.0))
                                            .text_color(if active {
                                                theme::token("color-primary-active", dark)
                                            } else {
                                                theme::token("color-text-secondary", dark)
                                            })
                                            .font_weight(if active {
                                                gpui::FontWeight::SEMIBOLD
                                            } else {
                                                gpui::FontWeight::NORMAL
                                            })
                                            .cursor_pointer()
                                            .hover(|s| {
                                                if !active {
                                                    s.bg(theme::token("color-bg-subtle-2", dark))
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
                                div().flex_1().flex().flex_col().overflow_hidden().child(
                                    match selected_detail {
                                        Some(ref detail) => {
                                            render_detail_tab_content(detail_tab, detail, dark)
                                        }
                                        None => div()
                                            .flex_1()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(12.0))
                                            .text_color(theme::token("color-text-tertiary", dark))
                                            .child("选择一条记录查看详情")
                                            .into_any_element(),
                                    },
                                ),
                            ),
                    ),
            )
            // ── Status bar ──
            .child(ui::status_bar(
                self.status_text(),
                if notice.is_some() {
                    theme::token("color-warning", dark)
                } else if exchanges.is_empty() {
                    theme::token("color-text-tertiary", dark)
                } else {
                    theme::token("color-text-secondary", dark)
                },
            ))
    }
}

fn detail_mini(key: &str, value: &str, value_color: impl Into<gpui::Hsla>) -> gpui::AnyElement {
    let key = key.to_string();
    let value_color: gpui::Hsla = value_color.into();
    div()
        .flex()
        .flex_col()
        .items_center()
        .text_size(px(11.0))
        .child(
            div()
                .text_color(theme::token(
                    "color-text-secondary",
                    crate::app::theme_mode::is_dark(),
                ))
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
    dark: bool,
) -> gpui::AnyElement {
    match tab {
        DetailTab::Overview => render_overview_section(detail, dark),
        DetailTab::RequestHeaders => render_headers_section(
            "请求头",
            &detail.request_headers_entries(),
            detail.has_request_headers(),
            dark,
        ),
        DetailTab::RequestBody => {
            render_body_section("请求体", detail.request_body_display(), dark)
        }
        DetailTab::ResponseHeaders => render_headers_section(
            "响应头",
            &detail.response_headers_entries(),
            detail.has_response_headers(),
            dark,
        ),
        DetailTab::ResponseBody => {
            render_body_section("响应体", detail.response_body_display(), dark)
        }
        DetailTab::Timing => render_timing_section(detail, dark),
    }
}

fn render_timing_section(detail: &CapturedExchange, dark: bool) -> gpui::AnyElement {
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
                .hover(|s| s.bg(theme::token("color-bg-subtle-2", dark)))
                .child(
                    div()
                        .w(px(80.0))
                        .text_color(theme::token("color-text-secondary", dark))
                        .child(key.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(theme::token("color-text-primary", dark))
                        .child(value),
                )
        }))
        .into_any_element()
}

fn render_headers_section(
    title: &str,
    entries: &[crate::features::http_capture::model::HeaderEntry],
    has_data: bool,
    dark: bool,
) -> gpui::AnyElement {
    if !has_data || entries.is_empty() {
        return render_empty_tab(title, dark);
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
                .hover(|s| s.bg(theme::token("color-bg-subtle-2", dark)))
                .child(
                    div()
                        .w(px(100.0))
                        .text_color(theme::token("color-text-secondary", dark))
                        .child(entry.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(theme::token("color-text-primary", dark))
                        .child(entry.value.clone()),
                )
        }))
        .into_any_element()
}

fn render_body_section(title: &str, display: BodyDisplay, dark: bool) -> gpui::AnyElement {
    match display {
        BodyDisplay::Empty => render_empty_tab(title, dark),
        BodyDisplay::Hinted(msg) => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .p_3()
            .text_size(px(11.0))
            .text_color(theme::token("color-text-tertiary", dark))
            .child(msg)
            .into_any_element(),
        BodyDisplay::Text(body) => div()
            .flex()
            .flex_col()
            .p_1()
            .text_size(px(11.0))
            .font_family("SF Mono")
            .text_color(theme::token("color-text-primary", dark))
            .children(body.lines().map(|line| div().child(line.to_string())))
            .into_any_element(),
    }
}

fn render_overview_section(detail: &CapturedExchange, dark: bool) -> gpui::AnyElement {
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
        (
            "请求大小",
            crate::features::http_capture::model::format_bytes(detail.request_size),
        ),
        (
            "响应大小",
            crate::features::http_capture::model::format_bytes(detail.response_size),
        ),
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
                .hover(|s| s.bg(theme::token("color-bg-subtle-2", dark)))
                .child(
                    div()
                        .w(px(80.0))
                        .text_color(theme::token("color-text-secondary", dark))
                        .child(key.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(theme::token("color-text-primary", dark))
                        .child(value),
                )
        }))
        .into_any_element()
}

fn render_empty_tab(label: &str, dark: bool) -> gpui::AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::token("color-text-tertiary", dark))
        .child(format!("{label}无数据"))
        .into_any_element()
}
