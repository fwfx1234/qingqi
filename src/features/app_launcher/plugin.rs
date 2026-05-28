use std::{sync::Arc, time::Duration};

use gpui::{
    AnyElement, App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollStrategy, StatefulInteractiveElement, Styled,
    Subscription, UniformListScrollHandle, Window, div, prelude::FluentBuilder, px, uniform_list,
};

use crate::{
    app::{
        events::{AppEventBus, AppEventKind},
        text_input::{TextInput, TextInputStyle},
        theme, ui,
    },
    core::{
        command::{CommandInvocation, CommandItem, CommandOutcome, CommandTarget},
        page::Page,
        plugin::{PluginListItem, PluginRuntime, PluginSession},
        storage::AppPaths,
    },
    features::app_launcher::{
        manifest,
        service::{AppEntry, AppIndexService, AppIndexSnapshot},
    },
};

pub struct AppLauncherRuntime {
    service: Arc<AppIndexService>,
    watch_started: bool,
}

impl AppLauncherRuntime {
    pub fn new(paths: AppPaths) -> Self {
        Self {
            service: Arc::new(AppIndexService::new(paths)),
            watch_started: false,
        }
    }

    pub fn with_service(service: Arc<AppIndexService>) -> Self {
        Self {
            service,
            watch_started: false,
        }
    }
}

impl PluginRuntime for AppLauncherRuntime {
    fn manifest(&self) -> crate::core::plugin::PluginManifest {
        manifest::manifest()
    }

    fn commands_revision(&self) -> u64 {
        self.service.revision()
    }

    fn open_session(
        &mut self,
        _: AppEventBus,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        if self.service.snapshot().apps.is_empty() {
            self.service.request_scan();
        }

        let view = cx.new(|cx| AppLauncherView::new(Arc::clone(&self.service), cx));
        Ok(Box::new(AppLauncherSession {
            view,
            service: Arc::clone(&self.service),
            query: String::new(),
            loaded_limit: 0,
        }))
    }

    fn commands(&self) -> Vec<CommandItem> {
        if self.service.snapshot().apps.is_empty() {
            self.service.request_scan();
        } else {
            self.service.request_probe_scan();
        }
        let manifest = self.manifest();
        vec![CommandItem::plugin_open(
            manifest.id,
            manifest.name,
            manifest.description,
            manifest.keywords.iter().copied(),
            manifest.command_prefixes.iter().copied(),
            manifest.visual.icon,
        )]
    }

    fn commands_for_query(&self, query: &str, limit: usize) -> Vec<CommandItem> {
        let manifest = self.manifest();
        let trimmed = query.trim();
        let snapshot = self.service.snapshot();
        if snapshot.apps.is_empty() {
            self.service.request_scan();
        } else {
            self.service.request_probe_scan();
        }
        if trimmed.is_empty() {
            return self.commands();
        }

        let max = if limit == 0 { 50 } else { limit.min(200) };
        self.service
            .search(trimmed, max)
            .into_iter()
            .map(|app| app_command(&manifest, app))
            .collect()
    }

    fn handle_command(
        &mut self,
        invocation: CommandInvocation,
        _cx: &mut App,
    ) -> anyhow::Result<CommandOutcome> {
        if let CommandTarget::PluginAction { payload, .. } = invocation.target
            && let Some(path) = payload
        {
            return Ok(CommandOutcome {
                message: Some(match self.service.open_app(&path) {
                    Ok(()) => format!("已打开 {}", std::path::Path::new(&path).display()),
                    Err(error) => error,
                }),
            });
        }
        Ok(CommandOutcome::default())
    }

    fn start_background(&mut self, events: AppEventBus, cx: &mut App) {
        self.service.request_scan();

        if self.watch_started {
            return;
        }
        self.watch_started = true;

        let service = Arc::clone(&self.service);
        cx.spawn(async move |async_cx| {
            let mut revision = service.revision();
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let next_revision = service.revision();
                if next_revision != revision {
                    revision = next_revision;
                    events.publish(manifest::PLUGIN_ID, AppEventKind::CommandsChanged);
                }
            }
        })
        .detach();
    }

    fn close_idle(&mut self) {}
}

fn app_command(manifest: &crate::core::plugin::PluginManifest, app: AppEntry) -> CommandItem {
    let mut keywords = vec![
        app.name.clone(),
        app.bundle_id.clone().unwrap_or_default(),
        app.path.clone(),
    ];
    keywords.extend(app.aliases.clone());
    CommandItem::plugin_action(
        manifest.id,
        format!("open-{}", app.name),
        app.name.clone(),
        app.bundle_id.clone().unwrap_or_else(|| app.path.clone()),
        keywords,
        ["app".to_string(), "open".to_string()],
        app.icon_path.clone().unwrap_or_default(),
        Some(app.path.clone()),
    )
}

struct AppLauncherSession {
    view: Entity<AppLauncherView>,
    service: Arc<AppIndexService>,
    query: String,
    loaded_limit: usize,
}

impl PluginSession for AppLauncherSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "软件快速启动"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }

    fn list_items(&mut self, _cx: &mut App) -> Vec<PluginListItem> {
        if self.loaded_limit == 0 {
            self.loaded_limit = 120;
        }
        self.service
            .search(&self.query, self.loaded_limit)
            .into_iter()
            .map(|app| {
                let path = app.path.clone();
                PluginListItem::new(
                    path.clone(),
                    app.name,
                    app.bundle_id.unwrap_or_else(|| path.clone()),
                    app.icon_path.unwrap_or_default(),
                )
                .with_usage_key(format!("app:{path}"))
            })
            .collect()
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) -> Vec<PluginListItem> {
        let next_query = text.trim().to_string();
        if next_query != self.query {
            self.query = next_query;
            self.loaded_limit = 120;
        } else {
            self.loaded_limit = self.loaded_limit.saturating_add(120).min(5000);
        }
        self.list_items(cx)
    }

    fn on_enter(&mut self, cx: &mut App) -> bool {
        let Some(item) = self.list_items(cx).into_iter().find(|item| item.enabled) else {
            return false;
        };
        self.service.record_launch(&item.id).unwrap_or_else(
            |error| tracing::warn!(error = %error, "app launch usage record failed"),
        );
        let _ = self.service.open_app(&item.id);
        true
    }

    fn on_list_item_selected(&mut self, item_id: &str, _cx: &mut App) {
        self.service.record_launch(item_id).unwrap_or_else(
            |error| tracing::warn!(error = %error, "app launch usage record failed"),
        );
        let _ = self.service.open_app(item_id);
    }
}

struct AppLauncherView {
    service: Arc<AppIndexService>,
    query_input: Entity<TextInput>,
    query: String,
    page_offset: usize,
    selected: usize,
    notice: Option<String>,
    focus_pending: bool,
    list_scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl AppLauncherView {
    fn new(service: Arc<AppIndexService>, cx: &mut Context<Self>) -> Self {
        let query_input = cx.new(|cx| {
            let mut input = TextInput::new(cx, "搜索应用、Bundle ID 或路径", "");
            input.set_style(
                TextInputStyle {
                    height: 36.0,
                    font_size: 13.0,
                    padding: 10.0,
                },
                cx,
            );
            input
        });

        let mut this = Self {
            service,
            query_input,
            query: String::new(),
            page_offset: 0,
            selected: 0,
            notice: None,
            focus_pending: true,
            list_scroll: UniformListScrollHandle::new(),
            _subscriptions: Vec::new(),
        };
        this.observe_query_input(cx);
        this
    }

    fn observe_query_input(&mut self, cx: &mut Context<Self>) {
        let query_input = self.query_input.clone();
        let subscription = cx.observe(&query_input, |view, _, cx| {
            view.sync_query(cx);
            cx.notify();
        });
        self._subscriptions.push(subscription);
    }

    fn page_limit(&self) -> usize {
        AppIndexService::DEFAULT_PAGE_LIMIT
    }

    fn filtered_page(&self) -> Page<AppEntry> {
        self.service
            .search_page(&self.query, self.page_offset, self.page_limit())
    }

    fn sync_query(&mut self, cx: &App) {
        self.query = self.query_input.read(cx).text();
        self.page_offset = 0;
        self.selected = 0;
        self.notice = None;
    }

    fn refresh_index(&mut self) {
        self.notice = Some(if self.service.request_scan() {
            String::from("正在后台刷新应用索引")
        } else {
            String::from("应用索引正在刷新")
        });
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let filtered = self.filtered_page();
        if filtered.rows.is_empty() {
            self.selected = 0;
            cx.notify();
            return;
        }

        let len = filtered.rows.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.list_scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        self.notice = None;
        cx.notify();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index.min(self.filtered_page().rows.len().saturating_sub(1));
        self.notice = None;
        cx.notify();
    }

    fn launch_selected(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_page();
        let Some(app) = filtered.rows.get(self.selected) else {
            self.notice = Some(String::from("没有可启动的应用"));
            cx.notify();
            return;
        };

        self.notice = Some(match self.service.open_app(&app.path) {
            Ok(()) => format!("已打开 {}", app.name),
            Err(error) => error,
        });
        cx.notify();
    }

    fn launch_index(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        self.launch_selected(cx);
    }

    fn clear_query(&mut self, cx: &mut Context<Self>) {
        self.query_input
            .update(cx, |input, input_cx| input.clear(input_cx));
        self.query.clear();
        self.page_offset = 0;
        self.selected = 0;
        self.notice = None;
        cx.notify();
    }

    fn can_page_backward(&self) -> bool {
        self.page_offset > 0
    }

    fn can_page_forward(&self) -> bool {
        let page = self.filtered_page();
        page.offset + page.rows.len() < page.total
    }

    fn page_backward(&mut self, cx: &mut Context<Self>) {
        if !self.can_page_backward() {
            return;
        }
        let step = self.page_limit();
        self.page_offset = self.page_offset.saturating_sub(step);
        self.selected = 0;
        self.notice = None;
        cx.notify();
    }

    fn page_forward(&mut self, cx: &mut Context<Self>) {
        let page = self.filtered_page();
        if page.offset + page.rows.len() >= page.total {
            return;
        }
        self.page_offset += self.page_limit();
        self.selected = 0;
        self.notice = None;
        cx.notify();
    }

    fn status_text(&self, snapshot: &AppIndexSnapshot, total_matches: usize) -> String {
        if let Some(notice) = self.notice.as_ref() {
            return notice.clone();
        }

        if let Some(error) = snapshot.last_error.as_ref() {
            return error.clone();
        }

        if snapshot.scan_running {
            if snapshot.apps.is_empty() {
                return String::from("正在后台索引应用");
            }
            if snapshot.icon_refresh_running {
                return format!("已索引 {} 个应用，正在补全图标", snapshot.apps.len());
            }
            return format!("已缓存 {} 个应用，后台刷新中", snapshot.apps.len());
        }

        if self.query.trim().is_empty() {
            if snapshot.apps.is_empty() {
                return String::from("暂无应用缓存，可手动刷新索引");
            }
            if let Some(last_scan) = snapshot.last_scan.as_ref() {
                return format!("已索引 {} 个应用 · {}", snapshot.apps.len(), last_scan);
            }
            return format!("已缓存 {} 个应用", snapshot.apps.len());
        }

        if total_matches == 0 {
            String::from("暂无匹配应用")
        } else {
            format!("匹配到 {} 个应用", total_matches)
        }
    }

    fn status_metrics(
        &self,
        snapshot: &AppIndexSnapshot,
        page: &Page<AppEntry>,
    ) -> AppLauncherStatusMetrics {
        let filtered_count = if self.query.trim().is_empty() {
            0
        } else {
            page.total
        };
        AppLauncherStatusMetrics {
            status_text: self.status_text(snapshot, page.total),
            total_apps: snapshot.apps.len(),
            match_count: match_count_for_query(&self.query, filtered_count),
            page_start: page_row_start(page.total, page.offset),
            page_end: page_row_end(page),
            page_total: page.total,
            last_scan: snapshot.last_scan.clone(),
            scan_running: snapshot.scan_running,
            icon_refresh_running: snapshot.icon_refresh_running,
            error_text: snapshot.last_error.clone(),
        }
    }
}

impl Render for AppLauncherView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_pending {
            window.focus(&self.query_input.focus_handle(cx));
            self.focus_pending = false;
        }

        let handle = cx.entity();
        let dark = crate::app::theme_mode::is_dark();
        let snapshot = self.service.snapshot();
        let mut filtered = self.filtered_page();
        if filtered.rows.is_empty() && filtered.total > 0 && self.page_offset >= filtered.total {
            self.page_offset = last_page_offset(filtered.total, self.page_limit());
            filtered = self.filtered_page();
        }
        let rows = filtered.rows;
        let selected = self.selected.min(rows.len().saturating_sub(1));
        let has_query = !self.query.trim().is_empty();
        let page_for_metrics = Page {
            rows: rows.clone(),
            total: filtered.total,
            offset: filtered.offset,
            limit: filtered.limit,
        };
        let metrics = self.status_metrics(&snapshot, &page_for_metrics);
        let query_input = self.query_input.clone();

        div()
            .size_full()
            .bg(theme::token("color-bg-page", dark))
            .text_color(theme::token("color-text-primary", dark))
            .font_family("PingFang SC")
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                match event.keystroke.key.as_str() {
                    "up" => view.move_selection(-1, cx),
                    "down" => view.move_selection(1, cx),
                    "enter" => view.launch_selected(cx),
                    _ => {}
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("软件快速启动"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::token("color-text-secondary", dark))
                            .child("搜索名称、Bundle ID 或路径"),
                    ),
            )
            .child(search_row(handle.clone(), query_input, dark))
            .child(pagination_row(
                handle.clone(),
                dark,
                self.page_offset,
                self.page_limit(),
                filtered.total,
                self.can_page_backward(),
                self.can_page_forward(),
            ))
            .child({
                let list_container = div()
                    .id("app-launcher-list")
                    .flex_1()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme::token("color-border-default", dark))
                    .bg(theme::token("color-bg-surface", dark));
                if rows.is_empty() {
                    list_container
                        .overflow_y_scroll()
                        .child(empty_state(&snapshot, has_query, dark))
                        .into_any_element()
                } else {
                    let scroll = self.list_scroll.clone();
                    let handle = handle.clone();
                    let total = rows.len();
                    list_container
                        .child(
                            uniform_list("app-launcher-rows", total, move |range, _window, _cx| {
                                range
                                    .map(|index| {
                                        let app = rows[index].clone();
                                        app_row(handle.clone(), app, index, index == selected, dark)
                                    })
                                    .collect()
                            })
                            .track_scroll(scroll)
                            .size_full(),
                        )
                        .into_any_element()
                }
            })
            .child(app_status_bar(metrics, dark))
    }
}

fn search_row(
    handle: Entity<AppLauncherView>,
    query_input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    div()
        .h(px(38.0))
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .h(px(36.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme::token("color-border-default", dark))
                .bg(theme::token("color-bg-surface", dark))
                .child(query_input),
        )
        .child(action_button("打开选中项", dark, {
            let handle = handle.clone();
            move |_, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.launch_selected(cx));
            }
        }))
        .child(action_button("刷新索引", dark, {
            let handle = handle.clone();
            move |_, cx| {
                let _ = cx.update_entity(&handle, |view, cx| {
                    view.refresh_index();
                    cx.notify();
                });
            }
        }))
        .child(action_button("重置", dark, move |_, cx| {
            let _ = cx.update_entity(&handle, |view, cx| view.clear_query(cx));
        }))
}

fn pagination_row(
    handle: Entity<AppLauncherView>,
    dark: bool,
    page_offset: usize,
    page_limit: usize,
    total: usize,
    can_page_backward: bool,
    can_page_forward: bool,
) -> impl IntoElement {
    div()
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(11.0))
                .font_family("SF Mono")
                .text_color(theme::token("color-text-secondary", dark))
                .child(page_range_text(page_offset, page_limit, total)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(page_button("上一页", 0, dark, can_page_backward, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.page_backward(cx));
                    }
                }))
                .child(page_button(
                    "下一页",
                    1,
                    dark,
                    can_page_forward,
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.page_forward(cx));
                    },
                )),
        )
}

fn empty_state(snapshot: &AppIndexSnapshot, has_query: bool, dark: bool) -> impl IntoElement {
    let (title, subtitle) = if has_query {
        ("暂无匹配应用", "换个关键词，或者清空搜索条件")
    } else if snapshot.scan_running {
        ("正在索引应用", "缓存列表会在后台刷新完成后自动更新")
    } else if snapshot.apps.is_empty() {
        ("暂无应用缓存", "点击刷新索引，准备本机应用列表")
    } else {
        ("暂无匹配应用", "当前条件下没有找到可启动的应用")
    };

    div()
        .w_full()
        .min_h(px(220.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            div()
                .size(px(44.0))
                .rounded(px(12.0))
                .bg(theme::launcher_icon_surface(dark))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(18.0))
                .text_color(theme::launcher_accent(dark))
                .child("A"),
        )
        .child(div().text_size(px(14.0)).child(title))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::token("color-text-secondary", dark))
                .child(subtitle),
        )
}

fn app_row(
    handle: Entity<AppLauncherView>,
    app: AppEntry,
    index: usize,
    selected: bool,
    dark: bool,
) -> impl IntoElement {
    let row_bg = if selected {
        theme::launcher_row_selected(dark)
    } else {
        theme::token("color-bg-surface", dark)
    };
    let handle_for_select = handle.clone();
    let handle_for_open = handle.clone();

    div()
        .id(("app-launcher-row", index))
        .h(px(60.0))
        .px_3()
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(row_bg)
        .hover(move |style| {
            style
                .bg(theme::launcher_row_selected(dark))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .gap_3()
        .on_click(move |event, _window, cx| {
            let _ = cx.update_entity(&handle_for_select, |view, cx| {
                view.select(index, cx);
                if event.click_count() >= 2 {
                    view.launch_selected(cx);
                }
            });
        })
        .child(app_icon_tile(&app, dark))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(div().text_size(px(14.0)).child(app.name))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family("SF Mono")
                        .text_color(theme::token("color-text-regular", dark))
                        .child(app.bundle_id.unwrap_or(app.path)),
                ),
        )
        .child(action_button("打开", dark, move |_, cx| {
            let _ = cx.update_entity(&handle_for_open, |view, cx| view.launch_index(index, cx));
        }))
}

fn app_icon_tile(app: &AppEntry, dark: bool) -> impl IntoElement {
    let tile = div().size(px(36.0)).flex().items_center().justify_center();

    if let Some(icon_path) = app.icon_path.as_deref() {
        tile.child(ui::icon_element(
            icon_path,
            theme::launcher_accent(dark),
            34.0,
        ))
        .into_any_element()
    } else {
        tile.rounded(px(8.0))
            .bg(theme::launcher_icon_surface(dark))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(theme::launcher_accent(dark))
                    .child(app.icon_letter.clone()),
            )
            .into_any_element()
    }
}

struct AppLauncherStatusMetrics {
    status_text: String,
    total_apps: usize,
    match_count: Option<usize>,
    page_start: Option<usize>,
    page_end: Option<usize>,
    page_total: usize,
    last_scan: Option<String>,
    scan_running: bool,
    icon_refresh_running: bool,
    error_text: Option<String>,
}

fn app_status_bar(metrics: AppLauncherStatusMetrics, dark: bool) -> impl IntoElement {
    div()
        .h(px(30.0))
        .rounded(px(6.0))
        .bg(theme::token("color-status-bar-bg", dark))
        .px_3()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .text_color(if metrics.error_text.is_some() {
                    theme::token("color-danger", dark)
                } else if metrics.scan_running {
                    theme::launcher_accent(dark)
                } else {
                    theme::token("color-text-regular", dark)
                })
                .child(metrics.status_text),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family("SF Mono")
                .text_color(theme::token("color-text-secondary", dark))
                .child(format!("apps {}", metrics.total_apps)),
        )
        .when(metrics.match_count.is_some(), |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .font_family("SF Mono")
                    .text_color(theme::token("color-text-secondary", dark))
                    .child(format!("match {}", metrics.match_count.unwrap_or_default())),
            )
        })
        .when(metrics.page_start.is_some(), |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .font_family("SF Mono")
                    .text_color(theme::token("color-text-secondary", dark))
                    .child(format!(
                        "page {}-{} / {}",
                        metrics.page_start.unwrap_or_default(),
                        metrics.page_end.unwrap_or_default(),
                        metrics.page_total
                    )),
            )
        })
        .when(metrics.scan_running, |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .font_family("SF Mono")
                    .text_color(theme::launcher_accent(dark))
                    .child(if metrics.icon_refresh_running {
                        "icons"
                    } else {
                        "refreshing"
                    }),
            )
        })
        .when(
            metrics.last_scan.is_some() && !metrics.scan_running,
            |bar| {
                bar.child(
                    div()
                        .text_size(px(11.0))
                        .font_family("SF Mono")
                        .text_color(theme::token("color-text-secondary", dark))
                        .child(metrics.last_scan.unwrap_or_default()),
                )
            },
        )
}

fn match_count_for_query(query: &str, filtered_count: usize) -> Option<usize> {
    (!query.trim().is_empty()).then_some(filtered_count)
}

fn action_button(
    label: &'static str,
    dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(30.0))
        .px_3()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(theme::token("color-bg-surface", dark))
        .hover(move |style| {
            style
                .bg(theme::launcher_row_selected(dark))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::token("color-text-primary", dark))
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn page_button(
    label: &'static str,
    id_suffix: usize,
    dark: bool,
    enabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(("app-launcher-page-button", id_suffix))
        .h(px(24.0))
        .px_2()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .bg(theme::token("color-bg-surface", dark))
        .when(enabled, |button| {
            button
                .hover(move |style| {
                    style
                        .bg(theme::launcher_row_selected(dark))
                        .cursor_pointer()
                })
                .on_click(move |event, _window, cx| on_click(event, cx))
        })
        .when(!enabled, |button| button.opacity(0.45))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::token("color-text-primary", dark))
        .child(label)
}

fn page_range_text(offset: usize, limit: usize, total: usize) -> String {
    match page_row_start(total, offset) {
        Some(start) => {
            let end = ((offset + limit).min(total)).max(start);
            format!("{start}-{end} / {total}")
        }
        None => String::from("0-0 / 0"),
    }
}

fn page_row_start(total: usize, offset: usize) -> Option<usize> {
    (total > 0).then_some(offset + 1)
}

fn page_row_end(page: &Page<AppEntry>) -> Option<usize> {
    (!page.rows.is_empty()).then_some(page.offset + page.rows.len())
}

fn last_page_offset(total: usize, page_limit: usize) -> usize {
    total.saturating_sub(1) / page_limit * page_limit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_count_is_hidden_without_query() {
        assert_eq!(match_count_for_query("", 5), None);
        assert_eq!(match_count_for_query("   ", 5), None);
    }

    #[test]
    fn match_count_is_reported_for_active_query() {
        assert_eq!(match_count_for_query("safari", 0), Some(0));
        assert_eq!(match_count_for_query("app", 7), Some(7));
    }

    #[test]
    fn page_range_text_uses_human_indices() {
        assert_eq!(page_range_text(0, 40, 83), "1-40 / 83");
        assert_eq!(page_range_text(40, 40, 83), "41-80 / 83");
        assert_eq!(page_range_text(80, 40, 83), "81-83 / 83");
        assert_eq!(page_range_text(0, 40, 0), "0-0 / 0");
    }

    #[test]
    fn last_page_offset_clamps_to_page_boundary() {
        assert_eq!(last_page_offset(0, 40), 0);
        assert_eq!(last_page_offset(1, 40), 0);
        assert_eq!(last_page_offset(40, 40), 0);
        assert_eq!(last_page_offset(41, 40), 40);
        assert_eq!(last_page_offset(83, 40), 80);
    }
}
