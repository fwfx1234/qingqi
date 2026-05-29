use std::{collections::HashSet, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollStrategy, StatefulInteractiveElement, Styled,
    Subscription, Task, UniformListScrollHandle, Window, div, prelude::FluentBuilder, px,
    uniform_list,
};

use crate::{
    app::{
        events::{AppEventBus, AppEventKind},
        text_input::{TextInput, TextInputStyle},
        theme, ui,
    },
    core::{
        command::{CommandInvocation, CommandItem, CommandOutcome, CommandTarget},
        database::DatabaseService,
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

const APP_PAGE_SIZE: usize = 120;
const APP_PREFETCH_THRESHOLD: usize = 40;
const LAUNCHER_APP_COMMAND_LIMIT: usize = 5_000;

impl AppLauncherRuntime {
    pub fn new(paths: AppPaths) -> Self {
        let database = Arc::new(DatabaseService::new(paths.clone()));
        database
            .register_database(crate::core::database::DatabaseSpec::app(
                "app-launcher/index",
                "app_index.db",
            ))
            .expect("app index database registration should succeed");
        Self {
            service: Arc::new(AppIndexService::new(database)),
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
        let snapshot = self.service.snapshot();
        if snapshot.apps.is_empty() {
            self.service.request_scan();
        } else {
            self.service.request_probe_scan();
        }
        let manifest = self.manifest();
        let apps = self.service.search("", LAUNCHER_APP_COMMAND_LIMIT);
        let mut commands = Vec::with_capacity(apps.len() + 1);

        for app in apps {
            commands.push(app_command(&manifest, app));
        }

        commands.push(CommandItem::plugin_open(
            manifest.id,
            manifest.name,
            manifest.description,
            manifest.keywords.iter().copied(),
            manifest.command_prefixes.iter().copied(),
            manifest.visual.icon,
        ));
        commands
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

        let max = if limit == 0 {
            LAUNCHER_APP_COMMAND_LIMIT
        } else {
            limit.min(LAUNCHER_APP_COMMAND_LIMIT)
        };
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
        let service = Arc::clone(&self.service);
        let item_id = item.id.clone();
        std::thread::spawn(move || {
            service.record_launch(&item_id).unwrap_or_else(
                |error| tracing::warn!(error = %error, "app launch usage record failed"),
            );
        });
        let _ = self.service.open_app(&item.id);
        true
    }

    fn on_list_item_selected(&mut self, item_id: &str, _cx: &mut App) {
        let service = Arc::clone(&self.service);
        let item_id = item_id.to_string();
        let launch_id = item_id.clone();
        std::thread::spawn(move || {
            service.record_launch(&item_id).unwrap_or_else(
                |error| tracing::warn!(error = %error, "app launch usage record failed"),
            );
        });
        let _ = self.service.open_app(&launch_id);
    }
}

struct AppLauncherView {
    service: Arc<AppIndexService>,
    query_input: Entity<TextInput>,
    query: String,
    rows: Rc<Vec<AppEntry>>,
    total_matches: usize,
    selected: usize,
    notice: Option<String>,
    loading: bool,
    has_more: bool,
    load_generation: u64,
    load_task: Option<Task<()>>,
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
            rows: Rc::new(Vec::new()),
            total_matches: 0,
            selected: 0,
            notice: None,
            loading: false,
            has_more: false,
            load_generation: 0,
            load_task: None,
            focus_pending: true,
            list_scroll: UniformListScrollHandle::new(),
            _subscriptions: Vec::new(),
        };
        this.observe_query_input(cx);
        this.refresh_async(cx);
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
        APP_PAGE_SIZE
    }

    fn filtered_page(&self) -> Page<AppEntry> {
        Page {
            rows: self.rows.as_ref().clone(),
            total: self.total_matches,
            offset: 0,
            limit: self.page_limit(),
        }
    }

    fn sync_query(&mut self, cx: &mut Context<Self>) {
        self.query = self.query_input.read(cx).text();
        self.notice = None;
        self.refresh_async(cx);
    }

    fn refresh_index(&mut self) {
        self.notice = Some(if self.service.request_scan() {
            String::from("正在后台刷新应用索引")
        } else {
            String::from("应用索引正在刷新")
        });
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            self.selected = 0;
            cx.notify();
            return;
        }

        let len = self.rows.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.list_scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        self.maybe_prefetch(self.selected.saturating_add(12), cx);
        self.notice = None;
        cx.notify();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index.min(self.rows.len().saturating_sub(1));
        self.maybe_prefetch(self.selected.saturating_add(12), cx);
        self.notice = None;
        cx.notify();
    }

    fn launch_selected(&mut self, cx: &mut Context<Self>) {
        let Some(app) = self.rows.get(self.selected) else {
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
        self.notice = None;
        self.refresh_async(cx);
        cx.notify();
    }

    fn refresh_async(&mut self, cx: &mut Context<Self>) {
        self.rows = Rc::new(Vec::new());
        self.total_matches = 0;
        self.selected = 0;
        self.loading = true;
        self.has_more = false;
        self.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.schedule_load(true, cx);
    }

    fn maybe_prefetch(&mut self, visible_end: usize, cx: &mut Context<Self>) {
        if self.loading || !self.has_more {
            return;
        }
        let remaining = self.rows.len().saturating_sub(visible_end);
        if remaining <= APP_PREFETCH_THRESHOLD {
            self.loading = true;
            self.schedule_load(false, cx);
        }
    }

    fn schedule_load(&mut self, reset: bool, cx: &mut Context<Self>) {
        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;
        let service = Arc::clone(&self.service);
        let query = self.query.clone();
        let offset = if reset { 0 } else { self.rows.len() };
        let limit = self.page_limit();

        self.load_task = Some(cx.spawn(async move |view, async_cx| {
            let page = async_cx
                .background_executor()
                .spawn(async move { service.search_page(&query, offset, limit) })
                .await;

            let _ = view.update(async_cx, |view, cx| {
                if view.load_generation != generation {
                    return;
                }
                view.loading = false;
                view.apply_loaded_page(page, reset);
                if reset && !view.rows.is_empty() {
                    view.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                }
                cx.notify();
            });
        }));
    }

    fn apply_loaded_page(&mut self, page: Page<AppEntry>, reset: bool) {
        self.total_matches = page.total;
        self.has_more = page.offset + page.rows.len() < page.total;
        if reset {
            self.rows = Rc::new(page.rows);
        } else {
            let rows = merge_app_rows(self.rows.as_ref(), page.rows);
            self.rows = Rc::new(rows);
        }
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }

    fn status_text(&self, snapshot: &AppIndexSnapshot, total_matches: usize) -> String {
        if let Some(notice) = self.notice.as_ref() {
            return notice.clone();
        }

        if let Some(error) = snapshot.last_error.as_ref() {
            return error.clone();
        }

        if self.loading {
            if self.rows.is_empty() {
                return String::from("正在加载应用列表");
            }
            return format!("已加载 {} 个应用，正在预取更多", self.rows.len());
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
                let more = if self.has_more {
                    " · 继续滚动加载更多"
                } else {
                    ""
                };
                return format!(
                    "已索引 {} 个应用 · 已加载 {} 个{more} · {}",
                    snapshot.apps.len(),
                    self.rows.len(),
                    last_scan
                );
            }
            let more = if self.has_more {
                " · 继续滚动加载更多"
            } else {
                ""
            };
            return format!(
                "已缓存 {} 个应用 · 已加载 {} 个{more}",
                snapshot.apps.len(),
                self.rows.len()
            );
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
        let filtered = self.filtered_page();
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
            .child(loaded_range_row(
                rows.len(),
                filtered.total,
                self.loading,
                dark,
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
                            uniform_list("app-launcher-rows", total, move |range, _window, cx| {
                                let _ = cx.update_entity(&handle, |view, cx| {
                                    view.maybe_prefetch(range.end, cx);
                                });
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

fn loaded_range_row(loaded: usize, total: usize, loading: bool, dark: bool) -> impl IntoElement {
    let label = if total == 0 {
        String::from("暂无应用")
    } else if loading {
        format!("已加载 {loaded} / {total} · 正在预取")
    } else {
        format!("已加载 {loaded} / {total}")
    };

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
                .child(label),
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

fn merge_app_rows(existing: &[AppEntry], next_rows: Vec<AppEntry>) -> Vec<AppEntry> {
    let mut merged = existing.to_vec();
    let mut seen = merged
        .iter()
        .map(|app| app.path.clone())
        .collect::<HashSet<_>>();

    for next_app in next_rows {
        if let Some(existing_app) = merged.iter_mut().find(|app| app.path == next_app.path) {
            *existing_app = next_app;
        } else if seen.insert(next_app.path.clone()) {
            merged.push(next_app);
        }
    }

    merged
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

fn page_row_start(total: usize, offset: usize) -> Option<usize> {
    (total > 0).then_some(offset + 1)
}

fn page_row_end(page: &Page<AppEntry>) -> Option<usize> {
    (!page.rows.is_empty()).then_some(page.offset + page.rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{database::DatabaseService, storage::AppPaths},
        features::app_launcher::{
            service::AppIndexService,
            store::{AppIndexCache, AppIndexStore},
        },
        platform::apps::InstalledApp,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

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
    fn runtime_commands_include_all_cached_apps() {
        let paths = temp_app_paths("runtime-commands");
        let database = Arc::new(DatabaseService::new(paths.clone()));
        database
            .register_database(crate::core::database::DatabaseSpec::app(
                "app-launcher/index",
                "app_index.db",
            ))
            .unwrap();
        let store = AppIndexStore::new(Arc::clone(&database), "app-launcher/index");
        store
            .save(&AppIndexCache {
                apps: sample_apps(12),
                last_scan: None,
            })
            .expect("cache should save");

        let runtime = AppLauncherRuntime::with_service(Arc::new(AppIndexService::new(database)));
        let commands = runtime.commands();
        let app_commands = commands
            .iter()
            .filter(|command| matches!(command.target, CommandTarget::PluginAction { .. }))
            .count();

        assert_eq!(app_commands, 12);
        assert!(
            commands
                .iter()
                .any(|command| matches!(command.target, CommandTarget::PluginOpen { .. })),
            "plugin open command should still be available"
        );
    }

    #[test]
    fn runtime_query_does_not_cap_results_at_fifty() {
        let paths = temp_app_paths("runtime-query");
        let database = Arc::new(DatabaseService::new(paths.clone()));
        database
            .register_database(crate::core::database::DatabaseSpec::app(
                "app-launcher/index",
                "app_index.db",
            ))
            .unwrap();
        let store = AppIndexStore::new(Arc::clone(&database), "app-launcher/index");
        store
            .save(&AppIndexCache {
                apps: sample_apps(80),
                last_scan: None,
            })
            .expect("cache should save");

        let runtime = AppLauncherRuntime::with_service(Arc::new(AppIndexService::new(database)));
        let commands = runtime.commands_for_query("fixture", 0);

        assert_eq!(commands.len(), 80);
    }

    fn sample_apps(count: usize) -> Vec<InstalledApp> {
        (0..count)
            .map(|index| InstalledApp {
                name: format!("Fixture App {index:03}"),
                path: format!("/Applications/Fixture App {index:03}.app"),
                bundle_id: Some(format!("dev.fixture.app{index:03}")),
                icon_path: None,
                aliases: vec![String::from("fixture")],
                icon_letter: String::from("F"),
            })
            .collect()
    }

    fn temp_app_paths(name: &str) -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-app-launcher-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("temp dir");
        AppPaths::for_test(dir)
    }
}
