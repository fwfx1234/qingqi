use std::{
    collections::{HashMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{
    App, AppContext, BoxShadow, Context, Entity, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollStrategy, StatefulInteractiveElement, Styled,
    Subscription, Task, UniformListScrollHandle, Window, div, point, prelude::FluentBuilder, px,
    size, uniform_list,
};
use gpui_component::scroll::Scrollbar;
use qingqi_ui::{
    text_input::{TextInput, TextInputStyle},
    theme, theme_mode, ui,
};

use crate::{
    app::{
        app_catalog::AppCatalog,
        window_controller::{PluginOpenTrace, WindowControllerHandle},
    },
    core::{
        clipboard::current_payload,
    },
};
use qingqi_core::command_usage::CommandUsage;
use qingqi_core::plugin::{
    InlineView, ListView, PluginListItem, PluginManager, command_kind_priority,
};
use qingqi_plugin::command::{Activation, Command, CommandKind, build_launcher_context};
use qingqi_plugin::events::{AppEventBus, AppEventKind};
use qingqi_plugin::plugin_spec::{PluginStatus, ViewMode, WindowSize};

// ── Design A · Deep Frost (launcher-glass-5.html), default light mode

const INPUT_HEIGHT: f32 = 60.0;
const ROW_HEIGHT: f32 = 64.0;
const HEADER_HEIGHT: f32 = INPUT_HEIGHT + 10.0;
const RESULTS_PADDING_TOP: f32 = 8.0;
const RESULTS_PADDING_BOTTOM: f32 = 14.0;
const ROW_GAP: f32 = 6.0;
const ROW_SLOT: f32 = ROW_HEIGHT + ROW_GAP;
const VISIBLE_ROWS: usize = 6;
const RESULTS_MAX_HEIGHT: f32 =
    RESULTS_PADDING_TOP + RESULTS_PADDING_BOTTOM + ROW_SLOT * VISIBLE_ROWS as f32 - ROW_GAP;
const LAUNCHER_WIDTH: f32 = 800.0;
const EMPTY_RESULTS_HEIGHT: f32 = 180.0;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(70);
const COMMAND_SEARCH_LIMIT: usize = 5_000;
const PLUGIN_LIST_PREFETCH_THRESHOLD: usize = 40;
static PLUGIN_OPEN_TRACE_ID: AtomicU64 = AtomicU64::new(1);

enum LauncherMode {
    Search,
    InlinePlugin {
        view: Box<dyn InlineView>,
    },
    ListPlugin {
        view: Box<dyn ListView>,
        items: Rc<Vec<PluginListItem>>,
        selected: usize,
    },
}

#[derive(Clone, Copy)]
struct PluginVisual {
    mode: ViewMode,
    status: PluginStatus,
}

pub struct Launcher {
    plugin_manager: Arc<Mutex<PluginManager>>,
    app_catalog: Arc<AppCatalog>,
    plugin_visuals: HashMap<String, PluginVisual>,
    query_input: Option<Entity<TextInput>>,
    results: Rc<Vec<Command>>,
    selected: usize,
    results_visible_start: usize,
    message: String,
    window_controller: Option<WindowControllerHandle>,
    last_query: String,
    pending_query: String,
    search_generation: u64,
    last_window_height: f32,
    results_scroll: UniformListScrollHandle,
    plugin_list_scroll: UniformListScrollHandle,
    plugin_list_visible_start: usize,
    clipboard_boost_map: HashMap<String, i32>,
    search_task: Option<Task<()>>,
    event_task: Option<Task<()>>,
    mode: LauncherMode,
    _subscriptions: Vec<Subscription>,
}

impl Launcher {
    pub fn window_width() -> f32 {
        LAUNCHER_WIDTH
    }

    pub fn min_window_height() -> f32 {
        HEADER_HEIGHT + Self::results_height_for_count(1)
    }

    pub fn window_height_for_results(results_len: usize) -> f32 {
        HEADER_HEIGHT + Self::results_height_for_count(results_len)
    }

    pub fn new(
        plugin_manager: Arc<Mutex<PluginManager>>,
        app_catalog: Arc<AppCatalog>,
        cx: &App,
    ) -> Self {
        let (all_commands, plugin_visuals, boost_map, usage) = {
            let mut manager = plugin_manager.lock().unwrap_or_else(|e| {
                tracing::error!("plugin manager poisoned, recovering");
                e.into_inner()
            });
            let boost_map = Self::latest_boost_map(cx, &*manager);
            let mut all_commands = app_catalog.search("", COMMAND_SEARCH_LIMIT);
            all_commands.extend(manager.commands_with_clipboard(&boost_map));
            let plugin_visuals = manager
                .manifests()
                .into_iter()
                .map(|manifest| {
                    (
                        manifest.id.to_string(),
                        PluginVisual {
                            mode: manifest.mode,
                            status: manifest.status,
                        },
                    )
                })
                .collect();
            let usage = manager.usage_map();
            (all_commands, plugin_visuals, boost_map, usage)
        };
        let results = Launcher::build_default_results(&all_commands, &plugin_visuals, &usage);
        Self {
            plugin_manager,
            app_catalog,
            plugin_visuals,
            query_input: None,
            results,
            selected: 0,
            clipboard_boost_map: boost_map,
            results_visible_start: 0,
            message: String::from("搜索功能或打开应用..."),
            window_controller: None,
            last_query: String::new(),
            pending_query: String::new(),
            search_generation: 0,
            last_window_height: 0.0,
            results_scroll: UniformListScrollHandle::new(),
            plugin_list_scroll: UniformListScrollHandle::new(),
            plugin_list_visible_start: 0,
            search_task: None,
            event_task: None,
            mode: LauncherMode::Search,
            _subscriptions: Vec::new(),
        }
    }

    pub fn initialize_async(&mut self, events: AppEventBus, cx: &mut Context<Self>) {
        self.refresh_clipboard_boost_map(cx);
        self.start_event_watch(events, cx);
    }

    fn start_event_watch(&mut self, events: AppEventBus, cx: &mut Context<Self>) {
        if self.event_task.is_some() {
            return;
        }

        self.event_task = Some(cx.spawn(async move |launcher, async_cx| {
            let receiver = Arc::new(Mutex::new(events.subscribe()));
            loop {
                let rx = Arc::clone(&receiver);
                let event = async_cx
                    .background_executor()
                    .spawn(async move { rx.lock().ok()?.recv().ok() })
                    .await;
                let Some(event) = event else {
                    break;
                };
                if event.kind == AppEventKind::CommandsChanged {
                    let _ = launcher.update(async_cx, |launcher, cx| {
                        if event.source.as_ref() != "app-catalog" {
                            launcher
                                .plugin_manager
                                .lock()
                                .unwrap_or_else(|e| {
                                    tracing::error!("plugin manager poisoned, recovering");
                                    e.into_inner()
                                })
                                .invalidate_commands();
                        }
                        launcher.refresh_results_after_commands_changed(cx);
                        cx.notify();
                    });
                }
            }
        }));
    }

    /// 事件驱动：唤起启动器时，从 DB 读最新使用数据重建排序。
    pub fn refresh_on_show(&mut self, cx: &mut App) {
        self.refresh_clipboard_boost_map(cx);
        self.refresh_plugin_visuals();
        let commands = self.default_commands_with_clipboard();
        let query = self.query(cx);
        if query.trim().is_empty() {
            self.results = self.default_results(&commands, &self.plugin_visuals);
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
        } else {
            self.results = Rc::new(self.query_commands(&query));
            self.selected = 0;
            self.results_visible_start = 0;
            self.results_scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        cx.refresh_windows();
    }

    fn refresh_results_after_commands_changed(&mut self, cx: &mut Context<Self>) {
        self.refresh_clipboard_boost_map(cx);
        let commands = self.default_commands_with_clipboard();

        let query = self.query(cx);
        if matches!(self.mode, LauncherMode::Search) {
            self.last_query = query.clone();
            self.pending_query = query.clone();
            if query.trim().is_empty() {
                self.results = self.default_results(&commands, &self.plugin_visuals);
            } else {
                self.results = Rc::new(self.query_commands(&query));
            }
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            // Preserve the current scroll offset: this runs on every background
            // `CommandsChanged` event, and forcing `scroll_to_item` here would snap the
            // list back to the top mid-wheel-scroll. Keyboard navigation re-aligns the
            // viewport on demand via `scroll_selection_into_view`.
            self.update_message();
        }

        if let LauncherMode::ListPlugin {
            view,
            items,
            selected,
        } = &mut self.mode
        {
            let plugin_id = view.plugin_id().to_string();
            let next_items = catch_unwind(AssertUnwindSafe(|| view.on_input_changed(&query, cx)))
                .unwrap_or_else(|error| {
                    tracing::error!(
                        plugin_id = %plugin_id,
                        error = %qingqi_plugin::plugin::panic_message(error),
                        "plugin panicked in on_input_changed (list refresh)"
                    );
                    Vec::new()
                });
            merge_plugin_list_items(items, next_items);
            *selected = (*selected).min(items.len().saturating_sub(1));
            self.plugin_list_visible_start = visible_start_for_selection(*selected);
        }
    }

    pub fn attach_handle(&mut self, _handle: Entity<Launcher>) {
        // No longer needed — cx.entity() is used in render instead.
        // Keep method as no-op for compatibility with window_controller.rs.
    }

    pub fn attach_window_controller(&mut self, handle: WindowControllerHandle) {
        self.window_controller = Some(handle);
    }

    pub fn attach_query_input(&mut self, input: Entity<TextInput>) {
        self.query_input = Some(input);
    }

    pub fn focus_query_input(&self, window: &mut Window, cx: &App) {
        if let Some(input) = self.query_input.as_ref() {
            window.focus(&input.focus_handle(cx));
        }
    }

    pub fn configure_query_input(input: &mut TextInput, cx: &mut Context<TextInput>) {
        input.set_style(
            TextInputStyle {
                height: INPUT_HEIGHT,
                font_size: 14.0,
                padding: 0.0,
            },
            cx,
        );
        input.set_chrome(false, cx);
        input.set_text_colors(
            theme::rgba_with_alpha(theme::launcher_title_text(), 1.0),
            theme::rgba_with_alpha(theme::launcher_faint_text(), 1.0),
            cx,
        );
    }

    fn default_results(
        &self,
        commands: &[Command],
        visuals: &HashMap<String, PluginVisual>,
    ) -> Rc<Vec<Command>> {
        let usage = self
            .plugin_manager
            .lock()
            .map(|pm| pm.usage_map())
            .unwrap_or_default();
        Self::build_default_results(commands, visuals, &usage)
    }

    fn build_default_results(
        commands: &[Command],
        visuals: &HashMap<String, PluginVisual>,
        usage: &HashMap<String, CommandUsage>,
    ) -> Rc<Vec<Command>> {
        let mut results: Vec<Command> = commands
            .iter()
            .filter(|item| Self::is_default_command(item, visuals))
            .cloned()
            .collect();
        // Sort by frecency (pre-computed decay score: recent + frequent = high)
        results.sort_by(|a, b| {
            let a_u = usage.get(&a.usage_key).cloned().unwrap_or_default();
            let b_u = usage.get(&b.usage_key).cloned().unwrap_or_default();
            b_u.frecency
                .partial_cmp(&a_u.frecency)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| command_kind_priority(a.kind).cmp(&command_kind_priority(b.kind)))
                .then_with(|| a.title.cmp(&b.title))
        });
        Rc::new(results)
    }

    fn is_default_command(item: &Command, visuals: &HashMap<String, PluginVisual>) -> bool {
        match item.kind {
            CommandKind::App => true,
            CommandKind::DynamicAction => true,
            CommandKind::Plugin => visuals
                .get(&item.plugin_id)
                .map(|visual| visual.status != PluginStatus::Preview)
                .unwrap_or(true),
        }
    }

    pub fn observe_query_input(&mut self, cx: &mut Context<Self>) {
        let Some(query_input) = self.query_input.clone() else {
            return;
        };
        // Guard against duplicate calls — clear old subscriptions
        if !self._subscriptions.is_empty() {
            self._subscriptions.clear();
        }
        let subscription = cx.observe(&query_input, |launcher, _, cx| {
            launcher.handle_query_changed(cx);
        });
        self._subscriptions.push(subscription);
    }

    fn query(&self, cx: &App) -> String {
        self.query_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default()
    }

    fn perform_search_for_query(&mut self, query: String, cx: &App) {
        self.last_query = query.clone();
        self.pending_query = query.clone();
        let query_empty = query.trim().is_empty();

        if query_empty {
            self.refresh_clipboard_boost_map(cx);
            let commands = self.default_commands_with_clipboard();
            self.results = self.default_results(&commands, &self.plugin_visuals);
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            self.results_visible_start = visible_start_for_selection(self.selected);
            self.results_scroll
                .scroll_to_item(self.selected, ScrollStrategy::Top);
            return;
        }

        let query_commands = self.query_commands(&query);
        self.results = Rc::new(query_commands);
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
        self.results_visible_start = visible_start_for_selection(self.selected);
        self.results_scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
    }

    fn latest_boost_map(cx: &App, plugin_manager: &PluginManager) -> HashMap<String, i32> {
        let payload = current_payload(cx);
        match payload {
            Some(p) => plugin_manager.build_clipboard_boost_map(&p),
            None => HashMap::new(),
        }
    }

    fn refresh_clipboard_boost_map(&mut self, cx: &App) {
        self.clipboard_boost_map = Self::latest_boost_map(
            cx,
            &*self.plugin_manager.lock().unwrap_or_else(|e| {
                tracing::error!("plugin manager poisoned, recovering");
                e.into_inner()
            }),
        );
    }

    fn refresh_plugin_visuals(&mut self) {
        if let Ok(manager) = self.plugin_manager.lock() {
            self.plugin_visuals = manager
                .manifests()
                .into_iter()
                .map(|m| {
                    (m.id.to_string(), PluginVisual { mode: m.mode, status: m.status })
                })
                .collect();
        }
    }

    fn default_commands_with_clipboard(&mut self) -> Vec<Command> {
        let mut commands = self.app_catalog.search("", COMMAND_SEARCH_LIMIT);
        commands.extend(
            self.plugin_manager
                .lock()
                .unwrap_or_else(|e| {
                    tracing::error!("plugin manager poisoned, recovering");
                    e.into_inner()
                })
                .commands_with_clipboard(&self.clipboard_boost_map),
        );
        commands
    }

    fn query_commands(&mut self, query: &str) -> Vec<Command> {
        let mut commands = self.app_catalog.search(query, COMMAND_SEARCH_LIMIT);
        commands.extend(
            self.plugin_manager
                .lock()
                .unwrap_or_else(|e| {
                    tracing::error!("plugin manager poisoned, recovering");
                    e.into_inner()
                })
                .query_commands_with_clipboard(
                    query,
                    COMMAND_SEARCH_LIMIT,
                    &self.clipboard_boost_map,
                ),
        );

        let mut seen = HashSet::new();
        let commands = commands
            .into_iter()
            .filter(|command| seen.insert(command.id.clone()))
            .collect::<Vec<_>>();
        let known_prefixes = commands
            .iter()
            .flat_map(|command| command.prefixes.iter().cloned())
            .collect::<Vec<_>>();
        let context = build_launcher_context(query, &known_prefixes);
        let mut scored = commands
            .into_iter()
            .filter_map(|command| {
                command
                    .score_with_context(&context)
                    .map(|matched| (matched.score, command))
            })
            .collect::<Vec<_>>();
        let usage = self
            .plugin_manager
            .lock()
            .map(|pm| pm.usage_map())
            .unwrap_or_default();
        // 使用 PluginManager 的统一排序逻辑，包含 clipboard boost 权重
        PluginManager::sort_commands(&mut scored, &usage, &self.clipboard_boost_map);
        scored.into_iter().map(|(_, command)| command).collect()
    }

    fn handle_query_changed(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        match &mut self.mode {
            LauncherMode::Search => self.schedule_search(cx),
            LauncherMode::InlinePlugin { view, .. } => {
                let plugin_id = view.plugin_id().to_string();
                let result = catch_unwind(AssertUnwindSafe(|| view.on_input_changed(&query, cx)));
                if let Err(error) = result {
                    tracing::error!(
                        plugin_id = %plugin_id,
                        error = %qingqi_plugin::plugin::panic_message(error),
                        "plugin panicked in on_input_changed (inline)"
                    );
                }
                cx.notify();
            }
            LauncherMode::ListPlugin {
                view,
                items,
                selected,
                ..
            } => {
                let plugin_id = view.plugin_id().to_string();
                let result = catch_unwind(AssertUnwindSafe(|| view.on_input_changed(&query, cx)));
                match result {
                    Ok(new_items) => {
                        *items = Rc::new(new_items);
                    }
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %qingqi_plugin::plugin::panic_message(error),
                            "plugin panicked in on_input_changed (list)"
                        );
                    }
                }
                *selected = (*selected).min(items.len().saturating_sub(1));
                self.plugin_list_visible_start = visible_start_for_selection(*selected);
                cx.notify();
            }
        }
    }

    fn schedule_search(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        self.pending_query = query.clone();
        self.search_generation = self.search_generation.wrapping_add(1);
        let generation = self.search_generation;

        self.search_task = Some(cx.spawn(async move |launcher, async_cx| {
            async_cx.background_executor().timer(SEARCH_DEBOUNCE).await;
            let _ = launcher.update(async_cx, |launcher, cx| {
                if launcher.search_generation != generation
                    || launcher.pending_query == launcher.last_query
                {
                    return;
                }

                launcher.perform_search_for_query(query, cx);
                launcher.update_message();
                cx.notify();
            });
        }));
    }

    fn update_message(&mut self) {
        let count = self.results.len();
        self.message = if count > 0 {
            format!("共匹配 {} 项", count)
        } else {
            "未找到匹配的功能".into()
        };
    }

    fn selected_item(&self) -> Option<&Command> {
        self.results.get(self.selected)
    }

    fn results_height_for_count(count: usize) -> f32 {
        if count == 0 {
            return RESULTS_PADDING_TOP + EMPTY_RESULTS_HEIGHT + RESULTS_PADDING_BOTTOM;
        }

        let visible = count.min(VISIBLE_ROWS) as f32;
        let content_height =
            RESULTS_PADDING_TOP + ROW_SLOT * visible - ROW_GAP + RESULTS_PADDING_BOTTOM;
        content_height.min(RESULTS_MAX_HEIGHT)
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut App) {
        let trace = PluginOpenTrace::new(PLUGIN_OPEN_TRACE_ID.fetch_add(1, Ordering::Relaxed));
        match &mut self.mode {
            LauncherMode::Search => {
                let Some(item) = self.selected_item().cloned() else {
                    return;
                };
                log_launcher_enter_started(
                    item.plugin_id.as_str(),
                    trace,
                    "search result",
                    Some(item.title.as_str()),
                );
                self.open_command(item, window, cx, trace);
            }
            LauncherMode::InlinePlugin { view, .. } => {
                let plugin_id = view.plugin_id().to_string();
                log_launcher_enter_started(&plugin_id, trace, "inline plugin", None);
                let enter_started = Instant::now();
                let confirmed = catch_unwind(AssertUnwindSafe(|| view.on_enter(cx)))
                    .unwrap_or_else(|error| {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %qingqi_plugin::plugin::panic_message(error),
                            "plugin panicked in on_enter (inline)"
                        );
                        false
                    });
                if confirmed {
                    log_launcher_step(
                        &plugin_id,
                        "inline plugin enter",
                        enter_started,
                        Some(trace),
                    );
                    log_launcher_total(&plugin_id, trace);
                    self.close_window_app(window, cx);
                } else {
                    log_launcher_step(
                        &plugin_id,
                        "inline plugin enter",
                        enter_started,
                        Some(trace),
                    );
                }
            }
            LauncherMode::ListPlugin {
                view,
                items,
                selected,
                ..
            } => {
                let Some(item) = items.get(*selected).cloned() else {
                    return;
                };
                if item.enabled {
                    let plugin_id = view.plugin_id().to_string();
                    log_launcher_enter_started(
                        &plugin_id,
                        trace,
                        "list plugin item",
                        Some(item.title.as_str()),
                    );
                    let select_started = Instant::now();
                    self.plugin_manager
                        .lock()
                        .unwrap_or_else(|e| {
                            tracing::error!("plugin manager poisoned, recovering");
                            e.into_inner()
                        })
                        .record_usage_key_background(plugin_list_usage_key(&item).to_string(), cx);
                    let item_id = item.id.clone();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        view.on_list_item_selected(&item_id, cx)
                    }));
                    if let Err(error) = result {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %qingqi_plugin::plugin::panic_message(error),
                            "plugin panicked in on_list_item_selected"
                        );
                    }
                    log_launcher_step(
                        &plugin_id,
                        "list item selected",
                        select_started,
                        Some(trace),
                    );
                    log_launcher_total(&plugin_id, trace);
                    self.close_window_app(window, cx);
                }
            }
        }
    }

    fn open_command(
        &mut self,
        item: Command,
        window: &mut Window,
        cx: &mut App,
        trace: PluginOpenTrace,
    ) {
        let started = Instant::now();
        self.plugin_manager
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!("plugin manager poisoned, recovering");
                e.into_inner()
            })
            .record_command_launch(&item);
        let activation = item.activation.clone();
        let Activation::Open { plugin_id } = &activation else {
            if let Some(window_controller) = self.window_controller.clone() {
                let _ = crate::app::runtime::run_command_with_trace(
                    window_controller,
                    activation,
                    cx,
                    Some(trace),
                );
            }
            self.close_window_app(window, cx);
            return;
        };

        let visual = self.plugin_visuals.get(plugin_id).cloned();
        let query = self.query(cx);
        let mut context = build_launcher_context(&query, &item.prefixes);
        if query.trim().is_empty() {
            context.clipboard_payload = current_payload(cx);
        }
        let launch_input =
            item.launch_input_with_context(&query, &context, &self.clipboard_boost_map);
        match visual.map(|visual| visual.mode).unwrap_or(ViewMode::Window) {
            ViewMode::Window => {
                if let Some(window_controller) = self.window_controller.clone() {
                    let _ = crate::app::runtime::run_command_with_trace(
                        window_controller,
                        activation.clone(),
                        cx,
                        Some(trace),
                    );
                }
                log_launcher_step(
                    plugin_id,
                    "launcher schedule window open",
                    started,
                    Some(trace),
                );
                self.close_window_app(window, cx);
            }
            ViewMode::Inline => {
                let view_started = Instant::now();
                let view_result = self
                    .plugin_manager
                    .lock()
                    .unwrap_or_else(|e| {
                        tracing::error!("plugin manager poisoned, recovering");
                        e.into_inner()
                    })
                    .open_inline_view(plugin_id, cx);
                log_launcher_step(plugin_id, "open inline view", view_started, Some(trace));
                match view_result {
                    Ok(mut view) => {
                        let input_started = Instant::now();
                        view.on_input_changed(&launch_input, cx);
                        log_launcher_step(
                            plugin_id,
                            "inline initial input",
                            input_started,
                            Some(trace),
                        );
                        self.mode = LauncherMode::InlinePlugin { view };
                        self.enter_plugin_mode_input(&launch_input, cx);
                    }
                    Err(error) => {
                        tracing::warn!(
                            plugin_id,
                            trace_id = trace.id,
                            error = %error,
                            "open inline plugin failed"
                        );
                        self.run_command(item.activation, cx);
                        self.close_window_app(window, cx);
                    }
                }
                log_launcher_step(plugin_id, "launcher open command", started, Some(trace));
                log_launcher_total(plugin_id, trace);
            }
            ViewMode::List => {
                let view_started = Instant::now();
                let view_result = self
                    .plugin_manager
                    .lock()
                    .unwrap_or_else(|e| {
                        tracing::error!("plugin manager poisoned, recovering");
                        e.into_inner()
                    })
                    .open_list_view(plugin_id, cx);
                log_launcher_step(plugin_id, "open list view", view_started, Some(trace));
                match view_result {
                    Ok(mut view) => {
                        let list_started = Instant::now();
                        let items = view.on_input_changed(&launch_input, cx);
                        log_launcher_step(
                            plugin_id,
                            "list initial input",
                            list_started,
                            Some(trace),
                        );
                        self.mode = LauncherMode::ListPlugin {
                            view,
                            items: Rc::new(items),
                            selected: 0,
                        };
                        self.plugin_list_visible_start = 0;
                        self.enter_plugin_mode_input(&launch_input, cx);
                        self.plugin_list_scroll
                            .scroll_to_item(0, ScrollStrategy::Top);
                    }
                    Err(error) => {
                        tracing::warn!(
                            plugin_id,
                            trace_id = trace.id,
                            error = %error,
                            "open list plugin failed"
                        );
                        self.run_command(item.activation, cx);
                        self.close_window_app(window, cx);
                    }
                }
                log_launcher_step(plugin_id, "launcher open command", started, Some(trace));
                log_launcher_total(plugin_id, trace);
            }
        }
    }

    fn enter_plugin_mode_input(&mut self, text: &str, cx: &mut App) {
        self.set_query_text(text, cx);
        self.last_query = text.to_string();
        self.pending_query = text.to_string();
    }

    fn set_query_text(&self, text: &str, cx: &mut App) {
        if let Some(input) = self.query_input.clone() {
            input.update(cx, |input, input_cx| {
                if input.text() != text {
                    input.set_text(text.to_string(), input_cx);
                }
            });
        }
    }

    fn run_command(&mut self, activation: Activation, cx: &mut App) -> Option<String> {
        if let Some(window_controller) = self.window_controller.clone() {
            crate::app::runtime::run_command(window_controller, activation, cx)
        } else {
            tracing::warn!("launcher missing window controller while running command");
            None
        }
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let started = Instant::now();
        let (len, selected, visible_start, scroll_handle) = match &mut self.mode {
            LauncherMode::Search => (
                self.results.len(),
                &mut self.selected,
                &mut self.results_visible_start,
                &self.results_scroll,
            ),
            LauncherMode::ListPlugin {
                items, selected, ..
            } => (
                items.len(),
                selected,
                &mut self.plugin_list_visible_start,
                &self.plugin_list_scroll,
            ),
            LauncherMode::InlinePlugin { .. } => return,
        };
        if len == 0 {
            *selected = 0;
            *visible_start = 0;
            cx.notify();
            return;
        }
        let next = (*selected as isize + delta).clamp(0, len as isize - 1) as usize;
        if next == *selected {
            return;
        }
        *selected = next;
        scroll_selection_into_view(next, len, visible_start, scroll_handle);
        self.maybe_prefetch_plugin_items(next.saturating_add(VISIBLE_ROWS), cx);
        log_slow_launcher_interaction(
            "move selection",
            started,
            &[("selected", next.to_string()), ("items", len.to_string())],
        );
        cx.notify();
    }

    fn maybe_prefetch_plugin_items(&mut self, visible_end: usize, cx: &mut Context<Self>) {
        let query = self.query(cx);
        if let LauncherMode::ListPlugin { view, items, .. } = &mut self.mode {
            let remaining = items.len().saturating_sub(visible_end);
            if remaining > PLUGIN_LIST_PREFETCH_THRESHOLD {
                return;
            }

            let plugin_id = view.plugin_id().to_string();
            let more_items = catch_unwind(AssertUnwindSafe(|| view.on_input_changed(&query, cx)))
                .unwrap_or_else(|error| {
                    tracing::error!(
                        plugin_id = %plugin_id,
                        error = %qingqi_plugin::plugin::panic_message(error),
                        "plugin panicked in on_input_changed (prefetch)"
                    );
                    Vec::new()
                });
            if more_items.len() > items.len() {
                merge_plugin_list_items(items, more_items);
            }
        }
    }

    fn select_prev(&mut self, cx: &mut Context<Self>) {
        self.move_selection(-1, cx);
    }

    fn select_next(&mut self, cx: &mut Context<Self>) {
        self.move_selection(1, cx);
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_selected(window, cx);
        cx.notify();
    }

    fn handle_launcher_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let started = Instant::now();
        match event.keystroke.key.as_str() {
            "up" => {
                self.select_prev(cx);
                log_slow_launcher_interaction("key up", started, &[]);
                cx.stop_propagation();
            }
            "down" => {
                self.select_next(cx);
                log_slow_launcher_interaction("key down", started, &[]);
                cx.stop_propagation();
            }
            "enter" => {
                self.confirm(window, cx);
                log_slow_launcher_interaction("key enter", started, &[]);
                cx.stop_propagation();
            }
            "escape" => {
                self.dismiss(window, cx);
                log_slow_launcher_interaction("key escape", started, &[]);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.mode, LauncherMode::Search) {
            self.close_plugin_mode(window, cx);
            return;
        }
        if let Some(input) = self.query_input.clone()
            && !input.read(cx).text().is_empty()
        {
            input.update(cx, |input, cx| input.clear(cx));
            return;
        }
        self.close_window(window, cx);
    }

    fn close_plugin_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match std::mem::replace(&mut self.mode, LauncherMode::Search) {
            LauncherMode::InlinePlugin { mut view, .. } => {
                let plugin_id = view.plugin_id().to_string();
                let result = catch_unwind(AssertUnwindSafe(|| view.on_close()));
                if let Err(error) = result {
                    tracing::error!(
                        plugin_id = %plugin_id,
                        error = %qingqi_plugin::plugin::panic_message(error),
                        "plugin panicked in on_close (inline)"
                    );
                }
            }
            LauncherMode::ListPlugin { mut view, .. } => {
                let plugin_id = view.plugin_id().to_string();
                let result = catch_unwind(AssertUnwindSafe(|| view.on_close()));
                if let Err(error) = result {
                    tracing::error!(
                        plugin_id = %plugin_id,
                        error = %qingqi_plugin::plugin::panic_message(error),
                        "plugin panicked in on_close (list)"
                    );
                }
            }
            LauncherMode::Search => {}
        }
        self.results_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.results_visible_start = 0;
        self.plugin_list_visible_start = 0;
        let query = self.query(cx);
        if query.trim().is_empty() {
            let commands = self.default_commands_with_clipboard();
            self.results = self.default_results(&commands, &self.plugin_visuals);
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            self.results_visible_start = visible_start_for_selection(self.selected);
        } else {
            self.results = Rc::new(self.query_commands(&query));
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            self.results_visible_start = visible_start_for_selection(self.selected);
        }
        self.update_message();
        if let Some(input) = self.query_input.clone() {
            window.focus(&input.focus_handle(cx));
        }
        cx.notify();
    }

    /// Must be called before window removal to ensure plugin views get on_close().
    pub fn cleanup_before_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.mode, LauncherMode::Search) {
            self.close_plugin_mode(window, cx);
        }
    }

    fn close_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Clean up plugin mode — ensures on_close() is called
        if !matches!(self.mode, LauncherMode::Search) {
            self.close_plugin_mode(window, cx);
        }
        if let Some(controller) = self.window_controller.as_ref() {
            controller
                .lock()
                .unwrap_or_else(|e| {
                    tracing::error!("plugin manager poisoned, recovering");
                    e.into_inner()
                })
                .clear_launcher_window();
        }
        window.defer(cx, |window, _cx| window.remove_window());
    }

    fn close_window_app(&mut self, window: &mut Window, cx: &mut App) {
        // Clean up plugin mode — inline because close_plugin_mode needs &mut Context<Self>
        if !matches!(self.mode, LauncherMode::Search) {
            match std::mem::replace(&mut self.mode, LauncherMode::Search) {
                LauncherMode::InlinePlugin { mut view, .. } => {
                    let _ = catch_unwind(AssertUnwindSafe(|| view.on_close()));
                }
                LauncherMode::ListPlugin { mut view, .. } => {
                    let _ = catch_unwind(AssertUnwindSafe(|| view.on_close()));
                }
                LauncherMode::Search => {}
            }
        }
        if let Some(controller) = self.window_controller.as_ref() {
            controller
                .lock()
                .unwrap_or_else(|e| {
                    tracing::error!("plugin manager poisoned, recovering");
                    e.into_inner()
                })
                .clear_launcher_window();
        }
        window.defer(cx, |window, _cx| window.remove_window());
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        self.search_task.take();
        self.event_task.take();
        self._subscriptions.clear();
        match std::mem::replace(&mut self.mode, LauncherMode::Search) {
            LauncherMode::InlinePlugin { mut view, .. } => {
                let _ = catch_unwind(AssertUnwindSafe(|| view.on_close()));
            }
            LauncherMode::ListPlugin { mut view, .. } => {
                let _ = catch_unwind(AssertUnwindSafe(|| view.on_close()));
            }
            LauncherMode::Search => {}
        }
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_started = Instant::now();
        let desired_height = match &self.mode {
            LauncherMode::Search => Self::window_height_for_results(self.results.len()),
            LauncherMode::InlinePlugin { .. } => HEADER_HEIGHT + RESULTS_MAX_HEIGHT,
            LauncherMode::ListPlugin { items, .. } => Self::window_height_for_results(items.len()),
        };
        if (desired_height - self.last_window_height).abs() > 0.5 {
            window.resize(size(px(LAUNCHER_WIDTH), px(desired_height)));
            self.last_window_height = desired_height;
        }

        let dark = theme_mode::is_dark();
        let handle = Some(cx.entity());
        let query_input = self.query_input.clone();
        let query = self.query(cx);
        let results = self.results.clone();
        let selected = self.selected;
        let search_mode = matches!(self.mode, LauncherMode::Search);
        let inline_mode = matches!(self.mode, LauncherMode::InlinePlugin { .. });
        let list_items = match &self.mode {
            LauncherMode::ListPlugin { items, .. } => items.clone(),
            _ => Rc::new(Vec::new()),
        };
        let list_selected = match &self.mode {
            LauncherMode::ListPlugin { selected, .. } => *selected,
            _ => 0,
        };
        let title_color = if dark {
            theme::launcher_title_text()
        } else {
            theme::launcher_title_text()
        };
        let placeholder_color = theme::launcher_faint_text();
        log_slow_launcher_interaction(
            "render prepare",
            render_started,
            &[
                ("results", self.results.len().to_string()),
                ("selected", self.selected.to_string()),
            ],
        );

        div()
            .size_full()
            .bg(theme::launcher_glass())
            .font_family(ui::font_ui())
            .text_color(title_color)
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .capture_key_down(cx.listener(Self::handle_launcher_key))
            .child(
                div()
                    .relative()
                    .h(px(HEADER_HEIGHT))
                    .flex_none()
                    .w_full()
                    .px(px(20.0))
                    .border_b_1()
                    .border_color(theme::launcher_soft_line())
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .size(px(24.0))
                            .rounded(px(8.0))
                            .bg(theme::launcher_keycap())
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.0))
                            .text_color(theme::launcher_muted_text())
                            .child("⌥"),
                    )
                    .child(
                        query_input
                            .map(|input| {
                                div()
                                    .flex_1()
                                    .h(px(INPUT_HEIGHT))
                                    .flex()
                                    .items_center()
                                    .child(input)
                                    .into_any_element()
                            })
                            .unwrap_or_else(|| {
                                div()
                                    .flex_1()
                                    .h(px(INPUT_HEIGHT))
                                    .flex()
                                    .items_center()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(placeholder_color)
                                    .child("搜索工具、命令、文件...")
                                    .into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(launcher_quick_tab(
                                handle.clone(),
                                "clipboard",
                                "剪贴板",
                                "TAB",
                            ))
                            .child(launcher_quick_tab(
                                handle.clone(),
                                "system-settings",
                                "设置",
                                "SET",
                            ))
                            .child(
                                div()
                                    .h(px(24.0))
                                    .px(px(10.0))
                                    .rounded(px(6.0))
                                    .bg(theme::launcher_keycap())
                                    .border_1()
                                    .border_color(theme::launcher_soft_line())
                                    .flex()
                                    .items_center()
                                    .text_size(px(10.0))
                                    .text_color(placeholder_color)
                                    .child("Space"),
                            ),
                    ),
            )
            .when(search_mode, |launcher| {
                let results_clone = results.clone();
                let sel = selected;
                let scroll_handle = handle.clone();
                let results_count = results_clone.len();

                if results_count == 0 {
                    if query.is_empty() {
                        launcher
                    } else {
                        launcher.child(
                            div()
                                .id("launcher-results-empty")
                                .w_full()
                                .h(px(Self::results_height_for_count(0)))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(13.0))
                                .text_color(theme::launcher_muted_text())
                                .child("未找到匹配的功能"),
                        )
                    }
                } else {
                    launcher.child(
                        div()
                            .relative()
                            .h(px(Self::results_height_for_count(results_count)))
                            .max_h(px(RESULTS_MAX_HEIGHT))
                            .child(
                                uniform_list(
                                    "launcher-results",
                                    results_count,
                                    move |range, _window, _cx| {
                                        range
                                            .map(|idx| {
                                                let item = results_clone[idx].clone();
                                                div()
                                                    .h(px(ROW_SLOT))
                                                    .flex_none()
                                                    .pb(px(ROW_GAP))
                                                    .child(result_row(
                                                        scroll_handle.clone(),
                                                        item,
                                                        idx == sel,
                                                        idx,
                                                    ))
                                            })
                                            .collect::<Vec<_>>()
                                    },
                                )
                                .track_scroll(self.results_scroll.clone())
                                .size_full()
                                .px(px(12.0))
                                .pt(px(RESULTS_PADDING_TOP))
                                .pb(px(RESULTS_PADDING_BOTTOM)),
                            )
                            .child(Scrollbar::vertical(&self.results_scroll)),
                    )
                }
            })
            .when(inline_mode, |launcher| {
                // Resolve auto-height preference from the plugin manifest
                // before rendering, so we can size the container accordingly.
                let use_auto_height = match &self.mode {
                    LauncherMode::InlinePlugin { view, .. } => self
                        .plugin_manager
                        .lock()
                        .unwrap_or_else(|e| {
                            tracing::error!("plugin manager poisoned, recovering");
                            e.into_inner()
                        })
                        .manifests()
                        .into_iter()
                        .any(|m| {
                            m.id.as_ref() == view.plugin_id().as_ref()
                                && matches!(m.window.size, WindowSize::Auto)
                        }),
                    _ => false,
                };

                let content = match &mut self.mode {
                    LauncherMode::InlinePlugin { view, .. } => {
                        let plugin_id = view.plugin_id().as_ref().to_string();
                        let result = catch_unwind(AssertUnwindSafe(|| view.render(window, cx)));
                        match result {
                            Ok(element) => element,
                            Err(error) => {
                                tracing::error!(
                                    plugin_id = %plugin_id,
                                    error = %qingqi_plugin::plugin::panic_message(error),
                                    "plugin panicked while rendering inline view"
                                );
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme::launcher_faint_text())
                                    .child("插件渲染出错")
                                    .into_any_element()
                            }
                        }
                    }
                    _ => div().into_any_element(),
                };

                let container = div()
                    .id("launcher-inline-plugin")
                    .w_full()
                    .overflow_hidden();

                launcher.child(if use_auto_height {
                    container
                        .min_h(px(EMPTY_RESULTS_HEIGHT))
                        .max_h(px(RESULTS_MAX_HEIGHT))
                        .child(content)
                } else {
                    container.h(px(RESULTS_MAX_HEIGHT)).child(content)
                })
            })
            .when(!search_mode && !inline_mode, |launcher| {
                launcher.child(plugin_list(
                    handle.clone(),
                    self.plugin_list_scroll.clone(),
                    list_items,
                    list_selected,
                    query,
                ))
            })
    }
}

fn launcher_quick_tab(
    handle: Option<Entity<Launcher>>,
    plugin_id: &'static str,
    label: &'static str,
    icon_label: &'static str,
) -> impl IntoElement {
    div()
        .h(px(24.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .bg(theme::launcher_keycap())
        .border_1()
        .border_color(theme::launcher_soft_line())
        .cursor_pointer()
        .hover(move |style| style.bg(theme::launcher_row_selected()).cursor_pointer())
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            let Some(handle) = handle.clone() else {
                return;
            };
            cx.update_entity(&handle, |launcher, entity_cx| {
                if let Some(item) = launcher
                    .default_commands_with_clipboard()
                    .iter()
                    .find(|item| {
                        item.plugin_id == plugin_id
                            && item.kind == CommandKind::Plugin
                            && matches!(item.activation, Activation::Open { .. })
                    })
                    .cloned()
                {
                    let trace =
                        PluginOpenTrace::new(PLUGIN_OPEN_TRACE_ID.fetch_add(1, Ordering::Relaxed));
                    launcher.open_command(item, window, entity_cx, trace);
                }
            });
        })
        .flex()
        .items_center()
        .gap(px(5.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::launcher_muted_text())
                .child(icon_label),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::launcher_muted_text())
                .child(label),
        )
}

fn plugin_list(
    handle: Option<Entity<Launcher>>,
    scroll: UniformListScrollHandle,
    items: Rc<Vec<PluginListItem>>,
    selected: usize,
    query: String,
) -> impl IntoElement {
    let count = items.len();
    if count == 0 {
        return div()
            .id("launcher-plugin-list-empty")
            .relative()
            .w_full()
            .h(px(Launcher::results_height_for_count(0)))
            .max_h(px(RESULTS_MAX_HEIGHT))
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .h(px(EMPTY_RESULTS_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.0))
                    .text_color(theme::launcher_muted_text())
                    .child(if query.trim().is_empty() {
                        "暂无内容"
                    } else {
                        "暂无匹配结果"
                    }),
            )
            .into_any_element();
    }
    let Some(list_handle) = handle.clone() else {
        return div()
            .relative()
            .w_full()
            .h(px(Launcher::results_height_for_count(0)))
            .max_h(px(RESULTS_MAX_HEIGHT))
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .h(px(EMPTY_RESULTS_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.0))
                    .text_color(theme::launcher_muted_text())
                    .child("启动器尚未完成初始化"),
            )
            .into_any_element();
    };
    div()
        .relative()
        .h(px(Launcher::results_height_for_count(count)))
        .max_h(px(RESULTS_MAX_HEIGHT))
        .child(
            uniform_list("launcher-plugin-list", count, move |range, _window, cx| {
                cx.update_entity(&list_handle, |launcher, cx| {
                    launcher.maybe_prefetch_plugin_items(range.end, cx);
                });
                range
                    .map(|idx| {
                        let item = items[idx].clone();
                        div()
                            .h(px(ROW_SLOT))
                            .flex_none()
                            .pb(px(ROW_GAP))
                            .child(plugin_list_row(handle.clone(), item, idx == selected, idx))
                    })
                    .collect::<Vec<_>>()
            })
            .track_scroll(scroll.clone())
            .size_full()
            .px(px(12.0))
            .pt(px(RESULTS_PADDING_TOP))
            .pb(px(RESULTS_PADDING_BOTTOM)),
        )
        .child(Scrollbar::vertical(&scroll))
        .into_any_element()
}

fn merge_plugin_list_items(items: &mut Rc<Vec<PluginListItem>>, next_items: Vec<PluginListItem>) {
    let mut merged = items.as_ref().clone();
    let mut seen = merged
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();

    for next_item in next_items {
        if let Some(existing) = merged.iter_mut().find(|item| item.id == next_item.id) {
            *existing = next_item;
        } else if seen.insert(next_item.id.clone()) {
            merged.push(next_item);
        }
    }

    *items = Rc::new(merged);
}

fn plugin_list_row(
    handle: Option<Entity<Launcher>>,
    item: PluginListItem,
    selected: bool,
    index: usize,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
    let item_for_click = item.clone();
    let accent = theme::launcher_accent();
    let row_bg = if selected {
        theme::rgba_with_alpha(accent, if dark { 0.12 } else { 0.08 })
    } else {
        theme::launcher_transparent()
    };
    let title_color = if selected {
        accent
    } else if dark {
        theme::launcher_title_text()
    } else {
        theme::launcher_title_text()
    };
    let subtitle_color = theme::launcher_faint_text();
    let icon_surface = if selected {
        theme::launcher_icon_surface_selected()
    } else if dark {
        theme::launcher_badge_bg()
    } else {
        theme::launcher_icon_surface()
    };
    let icon_border = if selected {
        theme::launcher_icon_border_selected()
    } else if dark {
        theme::launcher_icon_border()
    } else {
        theme::launcher_icon_border()
    };
    div()
        .id(("launcher-plugin-row", index))
        .h(px(ROW_HEIGHT))
        .flex_none()
        .w_full()
        .px(px(14.0))
        .rounded(px(12.0))
        .bg(row_bg)
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            let Some(handle) = handle.clone() else {
                return;
            };
            cx.update_entity(&handle, |launcher: &mut Launcher, entity_cx| {
                if let LauncherMode::ListPlugin {
                    view,
                    items,
                    selected,
                    ..
                } = &mut launcher.mode
                {
                    if let Some(index) = items.iter().position(|row| row.id == item_for_click.id) {
                        *selected = index;
                    }
                    if item_for_click.enabled {
                        launcher
                            .plugin_manager
                            .lock()
                            .unwrap_or_else(|e| {
                                tracing::error!("plugin manager poisoned, recovering");
                                e.into_inner()
                            })
                            .record_usage_key_background(
                                plugin_list_usage_key(&item_for_click).to_string(),
                                entity_cx,
                            );
                        view.on_list_item_selected(&item_for_click.id, entity_cx);
                        launcher.close_window(window, entity_cx);
                    }
                }
            });
        })
        .flex()
        .items_center()
        .gap(px(12.0))
        .child({
            let icon = item.icon.clone();
            div()
                .size(px(36.0))
                .flex_none()
                .rounded(px(10.0))
                .bg(icon_surface)
                .border_1()
                .border_color(icon_border)
                .flex()
                .items_center()
                .justify_center()
                .child(if icon.is_empty() {
                    div()
                        .text_size(px(15.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(accent)
                        .child("↗")
                        .into_any_element()
                } else {
                    ui::icon_element(icon.as_str(), accent, 28.0).into_any_element()
                })
                .into_any_element()
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(title_color)
                        .line_height(px(20.0))
                        .child(item.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(17.0))
                        .text_color(subtitle_color)
                        .child(item.subtitle.clone()),
                ),
        )
}

fn plugin_list_usage_key(item: &PluginListItem) -> &str {
    if item.usage_key.is_empty() {
        &item.id
    } else {
        &item.usage_key
    }
}

fn visible_start_for_selection(selected: usize) -> usize {
    selected / VISIBLE_ROWS * VISIBLE_ROWS
}

fn scroll_selection_into_view(
    selected: usize,
    len: usize,
    visible_start: &mut usize,
    scroll_handle: &UniformListScrollHandle,
) {
    let visible_end = (*visible_start + VISIBLE_ROWS).min(len);
    if selected < *visible_start {
        *visible_start = selected;
        scroll_handle.scroll_to_item(selected, ScrollStrategy::Top);
    } else if selected >= visible_end {
        *visible_start = selected.saturating_sub(VISIBLE_ROWS - 1);
        scroll_handle.scroll_to_item(selected, ScrollStrategy::Bottom);
    }
}

fn result_row(
    handle: Option<Entity<Launcher>>,
    item: Command,
    selected: bool,
    index: usize,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
    let item_for_click = item.clone();
    let accent = theme::launcher_accent();

    let icon_surface = if selected {
        theme::launcher_icon_surface_selected()
    } else if dark {
        theme::launcher_badge_bg()
    } else {
        theme::launcher_icon_surface()
    };
    let icon_border = if selected {
        theme::launcher_icon_border_selected()
    } else if dark {
        theme::launcher_icon_border()
    } else {
        theme::launcher_icon_border()
    };

    let (badge_label, badge_bg, badge_fg) = result_badge(&item);

    let row_bg = if selected {
        if dark {
            theme::rgba_with_alpha(accent, 0.12)
        } else {
            gpui::Hsla::from(theme::launcher_row_bg_selected_light())
        }
    } else {
        theme::launcher_transparent()
    };
    let row_border = if selected {
        if dark {
            theme::rgba_with_alpha(accent, 0.2)
        } else {
            gpui::Hsla::from(theme::launcher_row_border_selected_light())
        }
    } else {
        theme::launcher_transparent()
    };

    let title_color = if selected {
        accent
    } else if dark {
        theme::launcher_title_text()
    } else {
        theme::launcher_title_text()
    };
    let subtitle_color = theme::launcher_faint_text();
    let hover_bg = if dark {
        theme::launcher_row_hover()
    } else {
        theme::launcher_row_hover()
    };

    div()
        .id(("launcher-row", index))
        .h(px(ROW_HEIGHT))
        .flex_none()
        .w_full()
        .px(px(14.0))
        .rounded(px(12.0))
        .bg(row_bg)
        .border_1()
        .border_color(row_border)
        .when(selected && dark, |row| {
            row.shadow(vec![BoxShadow {
                color: theme::launcher_row_glow_dark(),
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(30.0),
                spread_radius: px(0.0),
            }])
        })
        .cursor_pointer()
        .when(!selected, move |row| {
            row.hover(move |style| style.bg(hover_bg))
        })
        .on_click(move |_, window, cx| {
            let Some(handle) = handle.clone() else {
                return;
            };
            cx.update_entity(&handle, |launcher, entity_cx| {
                if let Some(index) = launcher
                    .results
                    .iter()
                    .position(|row| row.id == item_for_click.id)
                {
                    launcher.selected = index;
                }
                let trace =
                    PluginOpenTrace::new(PLUGIN_OPEN_TRACE_ID.fetch_add(1, Ordering::Relaxed));
                launcher.open_command(item_for_click.clone(), window, entity_cx, trace);
                entity_cx.notify();
            });
        })
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(launcher_icon(
            &item,
            icon_surface,
            icon_border,
            launcher_icon_tint(&item.plugin_id),
        ))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(title_color)
                        .line_height(px(20.0))
                        .child(item.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(17.0))
                        .text_color(subtitle_color)
                        .child(item.subtitle.clone()),
                ),
        )
        .when(!badge_label.is_empty(), |row| {
            row.child(
                div()
                    .h(px(22.0))
                    .px(px(12.0))
                    .rounded(px(20.0))
                    .bg(badge_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .text_color(badge_fg)
                    .child(badge_label),
            )
        })
}

fn launcher_icon(
    item: &Command,
    surface: gpui::Hsla,
    border: gpui::Hsla,
    tint: gpui::Rgba,
) -> impl IntoElement {
    let icon = item.icon.clone();

    div()
        .size(px(36.0))
        .flex_none()
        .rounded(px(10.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .flex()
        .items_center()
        .justify_center()
        .child({
            if icon.is_empty() {
                div()
                    .text_size(px(10.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(tint)
                    .child(launcher_icon_label(item))
                    .into_any_element()
            } else {
                ui::icon_element(icon.as_str(), tint, 28.0).into_any_element()
            }
        })
}

fn launcher_icon_tint(plugin_id: &str) -> gpui::Rgba {
    theme::launcher_plugin_icon_tint(plugin_id)
}

fn launcher_icon_label(item: &Command) -> &'static str {
    match item.plugin_id.as_str() {
        "api-debugger" => "API",
        "app-launcher" => "↗",
        "clipboard" => "⌘",
        "download-manager" => "↓",
        "ftp-sftp-ssh-client" => "FTP",
        "gpui-demo" => "GP",
        "http-capture" => "HTTP",
        "image-compress" => "IMG",
        "json-parser" => "{}",
        "qr-code" => "QR",
        "quick-launch" => "⚡",
        "system-settings" => "SET",
        "about" => "i",
        _ => "•",
    }
}

fn log_launcher_step(
    plugin_id: &str,
    step: &'static str,
    started: Instant,
    trace: Option<PluginOpenTrace>,
) {
    let duration_ms = started.elapsed().as_millis() as u64;
    if duration_ms < 50 {
        tracing::debug!(
            plugin_id,
            step,
            duration_ms,
            trace_id = trace.map(|trace| trace.id),
            "launcher plugin step"
        );
    } else {
        tracing::warn!(
            plugin_id,
            step,
            duration_ms,
            trace_id = trace.map(|trace| trace.id),
            "slow launcher plugin step"
        );
    }
}

fn log_launcher_enter_started(
    plugin_id: &str,
    trace: PluginOpenTrace,
    mode: &'static str,
    item_title: Option<&str>,
) {
    tracing::debug!(
        plugin_id,
        trace_id = trace.id,
        mode,
        item_title,
        "plugin enter started"
    );
}

fn log_launcher_total(plugin_id: &str, trace: PluginOpenTrace) {
    let duration_ms = trace.started.elapsed().as_millis() as u64;
    if duration_ms < 50 {
        tracing::debug!(
            plugin_id,
            trace_id = trace.id,
            duration_ms,
            "plugin enter total"
        );
    } else {
        tracing::warn!(
            plugin_id,
            trace_id = trace.id,
            duration_ms,
            "slow plugin enter total"
        );
    }
}

fn log_slow_launcher_interaction(step: &'static str, started: Instant, fields: &[(&str, String)]) {
    let duration_ms = started.elapsed().as_millis() as u64;
    if duration_ms >= 16 {
        tracing::warn!(step, duration_ms, ?fields, "slow launcher interaction");
    }
}

fn result_badge(item: &Command) -> (String, gpui::Hsla, gpui::Rgba) {
    let dark = theme_mode::is_dark();
    let tag_bg = if dark {
        theme::launcher_badge_bg()
    } else {
        theme::launcher_badge_bg()
    };
    let tag_fg = theme::launcher_faint_text();

    match item.kind {
        CommandKind::App => (String::from("应用"), tag_bg, tag_fg),
        CommandKind::DynamicAction => (
            String::from("动作"),
            theme::rgba_with_alpha(theme::launcher_accent(), if dark { 0.12 } else { 0.08 }),
            theme::launcher_accent(),
        ),
        CommandKind::Plugin => match item.plugin_id.as_str() {
            "system-settings" => (String::from("系统"), tag_bg, tag_fg),
            "about" => (String::from("关于"), tag_bg, tag_fg),
            _ => (String::from("工具"), tag_bg, tag_fg),
        },
    }
}
