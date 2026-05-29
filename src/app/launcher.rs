use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
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
    Subscription, Task, UniformListScrollHandle, Window, div, hsla, point, prelude::FluentBuilder,
    px, rgb, size, uniform_list,
};
use gpui_component::scroll::Scrollbar;

use crate::{
    app::{
        app_catalog::AppCatalog,
        events::{AppEventBus, AppEventKind},
        text_input::{TextInput, TextInputStyle},
        theme, theme_mode, ui,
        window_controller::{PluginOpenTrace, WindowControllerHandle},
    },
    core::{
        command::{
            Activation, CommandItem, CommandKind, ContextKind,
            build_launcher_context_with_clipboard_kinds, detect_text_context_kinds,
            push_context_kind, unique_context_kinds,
        },
        plugin::{InlineView, ListView, PluginListItem, PluginManager},
        plugin_spec::{PluginStatus, PluginVisualSpec, PluginWindowMode},
    },
    features::clipboard::{
        history_store::{ClipboardItemKind, ClipboardRecord},
        service::ClipboardService,
    },
};

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
        session: Box<dyn InlineView>,
    },
    ListPlugin {
        session: Box<dyn ListView>,
        items: Rc<Vec<PluginListItem>>,
        selected: usize,
    },
}

pub struct Launcher {
    plugin_manager: Rc<RefCell<PluginManager>>,
    app_catalog: Arc<AppCatalog>,
    clipboard_service: Arc<Mutex<ClipboardService>>,
    plugin_visuals: HashMap<String, PluginVisualSpec>,
    query_input: Option<Entity<TextInput>>,
    all_commands: Vec<CommandItem>,
    results: Rc<Vec<CommandItem>>,
    selected: usize,
    results_visible_start: usize,
    message: String,
    self_handle: Option<Entity<Launcher>>,
    window_controller: Option<WindowControllerHandle>,
    last_query: String,
    pending_query: String,
    search_generation: u64,
    last_commands_revision: u64,
    plugin_list_revision: u64,
    last_window_height: f32,
    results_scroll: UniformListScrollHandle,
    plugin_list_scroll: UniformListScrollHandle,
    plugin_list_visible_start: usize,
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
        plugin_manager: Rc<RefCell<PluginManager>>,
        app_catalog: Arc<AppCatalog>,
        clipboard_service: Arc<Mutex<ClipboardService>>,
        _cx: &App,
    ) -> Self {
        let (all_commands, plugin_visuals, last_commands_revision) = {
            let mut manager = plugin_manager.borrow_mut();
            let clipboard_kinds = Self::latest_clipboard_context_kinds(&clipboard_service);
            let mut all_commands = app_catalog.search("", COMMAND_SEARCH_LIMIT);
            all_commands.extend(manager.commands_with_clipboard(clipboard_kinds));
            let last_commands_revision = manager.command_cache_revision();
            let plugin_visuals = manager
                .manifests()
                .into_iter()
                .map(|manifest| (manifest.id.to_string(), manifest.visual))
                .collect();
            (all_commands, plugin_visuals, last_commands_revision)
        };
        let results = Self::default_results(&all_commands, &plugin_visuals);
        Self {
            plugin_manager,
            app_catalog,
            clipboard_service,
            plugin_visuals,
            query_input: None,
            all_commands,
            results,
            selected: 0,
            results_visible_start: 0,
            message: String::from("搜索功能或打开应用..."),
            self_handle: None,
            window_controller: None,
            last_query: String::new(),
            pending_query: String::new(),
            search_generation: 0,
            last_commands_revision,
            plugin_list_revision: last_commands_revision,
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
        self.refresh_clipboard_context_async(cx);
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
                        launcher.plugin_manager.borrow_mut().invalidate_commands();
                        launcher.refresh_results_after_commands_changed(cx);
                        cx.notify();
                    });
                }
            }
        }));
    }

    fn refresh_results_after_commands_changed(&mut self, cx: &mut Context<Self>) {
        let clipboard_kinds = self.current_cached_clipboard_context_kinds();
        self.last_commands_revision = self.plugin_manager.borrow_mut().command_cache_revision();
        self.all_commands = self.default_commands_with_clipboard(clipboard_kinds.clone());

        let query = self.query(cx);
        if matches!(self.mode, LauncherMode::Search) {
            self.last_query = query.clone();
            self.pending_query = query.clone();
            if query.trim().is_empty() {
                self.results = Self::default_results(&self.all_commands, &self.plugin_visuals);
            } else {
                self.results = Rc::new(self.commands_for_query(&query, clipboard_kinds));
            }
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            self.results_visible_start = visible_start_for_selection(self.selected);
            if !self.results.is_empty() {
                self.results_scroll
                    .scroll_to_item(self.selected, ScrollStrategy::Top);
            }
            self.update_message();
        }

        if matches!(self.mode, LauncherMode::ListPlugin { .. }) {
            self.plugin_list_revision = 0;
        }
    }

    pub fn clipboard_context_kinds(
        service: &Arc<Mutex<ClipboardService>>,
        cx: &App,
    ) -> Vec<ContextKind> {
        service
            .lock()
            .ok()
            .and_then(|service| {
                let _ = service.capture_current(cx);
                service.latest_record().ok().flatten()
            })
            .map(|record| clipboard_record_context_kinds(&record))
            .unwrap_or_default()
    }

    pub(crate) fn latest_clipboard_context_kinds(
        service: &Arc<Mutex<ClipboardService>>,
    ) -> Vec<ContextKind> {
        service
            .lock()
            .ok()
            .and_then(|service| service.latest_record().ok().flatten())
            .map(|record| clipboard_record_context_kinds(&record))
            .unwrap_or_default()
    }

    pub fn attach_handle(&mut self, handle: Entity<Launcher>) {
        self.self_handle = Some(handle);
    }

    pub fn attach_window_controller(&mut self, handle: WindowControllerHandle) {
        self.window_controller = Some(handle);
    }

    pub fn attach_query_input(&mut self, input: Entity<TextInput>) {
        self.query_input = Some(input);
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
            theme::rgba_with_alpha(rgb(0x333348), 1.0),
            theme::rgba_with_alpha(theme::launcher_faint_text(false), 1.0),
            cx,
        );
    }

    fn default_results(
        commands: &[CommandItem],
        visuals: &HashMap<String, PluginVisualSpec>,
    ) -> Rc<Vec<CommandItem>> {
        Rc::new(
            commands
                .iter()
                .filter(|item| Self::is_default_command(item, visuals))
                .cloned()
                .collect(),
        )
    }

    fn is_default_command(item: &CommandItem, visuals: &HashMap<String, PluginVisualSpec>) -> bool {
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
        self.refresh_commands_if_needed(cx);

        if query_empty {
            self.results = Self::default_results(&self.all_commands, &self.plugin_visuals);
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            self.results_visible_start = visible_start_for_selection(self.selected);
            self.results_scroll
                .scroll_to_item(self.selected, ScrollStrategy::Top);
            return;
        }

        let query_commands =
            self.commands_for_query(&query, self.current_clipboard_context_kinds(cx));
        self.results = Rc::new(query_commands);
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
        self.results_visible_start = visible_start_for_selection(self.selected);
        self.results_scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
    }

    fn current_clipboard_context_kinds(&self, cx: &App) -> Vec<ContextKind> {
        Self::clipboard_context_kinds(&self.clipboard_service, cx)
    }

    fn current_cached_clipboard_context_kinds(&self) -> Vec<ContextKind> {
        Self::latest_clipboard_context_kinds(&self.clipboard_service)
    }

    fn default_commands_with_clipboard(
        &mut self,
        clipboard_kinds: Vec<ContextKind>,
    ) -> Vec<CommandItem> {
        let mut commands = self.app_catalog.search("", COMMAND_SEARCH_LIMIT);
        commands.extend(
            self.plugin_manager
                .borrow_mut()
                .commands_with_clipboard(clipboard_kinds),
        );
        commands
    }

    fn commands_for_query(
        &mut self,
        query: &str,
        clipboard_kinds: Vec<ContextKind>,
    ) -> Vec<CommandItem> {
        let mut commands = self.app_catalog.search(query, COMMAND_SEARCH_LIMIT);
        commands.extend(
            self.plugin_manager
                .borrow_mut()
                .commands_for_query_with_clipboard(
                    query,
                    COMMAND_SEARCH_LIMIT,
                    clipboard_kinds.clone(),
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
        let context =
            build_launcher_context_with_clipboard_kinds(query, &known_prefixes, clipboard_kinds);
        let mut scored = commands
            .into_iter()
            .filter_map(|command| {
                command
                    .score_with_context(&context)
                    .map(|matched| (matched.score, command))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .cmp(left_score)
                .then_with(|| left.title.cmp(&right.title))
        });
        scored.into_iter().map(|(_, command)| command).collect()
    }

    fn refresh_clipboard_context_async(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.clipboard_service);
        self.search_task = Some(cx.spawn(async move |launcher, async_cx| {
            let clipboard_kinds = async_cx
                .background_executor()
                .spawn(async move { Launcher::latest_clipboard_context_kinds(&service) });

            let clipboard_kinds = clipboard_kinds.await;

            let _ = launcher.update(async_cx, |launcher, cx| {
                let current_query = launcher.query(cx);
                launcher.last_commands_revision = launcher
                    .plugin_manager
                    .borrow_mut()
                    .command_cache_revision();
                launcher.all_commands =
                    launcher.default_commands_with_clipboard(clipboard_kinds.clone());

                if current_query.trim().is_empty() {
                    launcher.results =
                        Self::default_results(&launcher.all_commands, &launcher.plugin_visuals);
                } else {
                    launcher.results =
                        Rc::new(launcher.commands_for_query(&current_query, clipboard_kinds));
                }

                launcher.selected = launcher
                    .selected
                    .min(launcher.results.len().saturating_sub(1));
                launcher.results_visible_start = visible_start_for_selection(launcher.selected);
                launcher.update_message();
                if !launcher.results.is_empty() {
                    launcher
                        .results_scroll
                        .scroll_to_item(launcher.selected, ScrollStrategy::Top);
                }
                cx.notify();
            });
        }));
    }

    fn perform_search_now(&mut self, cx: &App) -> bool {
        let query = self.query(cx);
        if query == self.last_query {
            return false;
        }

        self.perform_search_for_query(query, cx);
        self.update_message();
        true
    }

    fn handle_query_changed(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        match &mut self.mode {
            LauncherMode::Search => self.schedule_search(cx),
            LauncherMode::InlinePlugin { session, .. } => {
                session.on_input_changed(&query, cx);
                cx.notify();
            }
            LauncherMode::ListPlugin {
                session,
                items,
                selected,
                ..
            } => {
                *items = Rc::new(session.on_input_changed(&query, cx));
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

    fn refresh_plugin_list_if_needed(&mut self, cx: &mut Context<Self>) {
        let revision = self.plugin_manager.borrow_mut().command_cache_revision();
        if revision == self.plugin_list_revision {
            return;
        }
        self.plugin_list_revision = revision;

        let query = self.query(cx);
        if let LauncherMode::ListPlugin {
            session,
            items,
            selected,
        } = &mut self.mode
        {
            let next_items = session.on_input_changed(&query, cx);
            merge_plugin_list_items(items, next_items);
            *selected = (*selected).min(items.len().saturating_sub(1));
            self.plugin_list_visible_start = visible_start_for_selection(*selected);
        }
    }

    fn refresh_commands_if_needed(&mut self, cx: &App) -> bool {
        let revision = self.plugin_manager.borrow_mut().command_cache_revision();
        if revision == self.last_commands_revision {
            return false;
        }

        self.last_commands_revision = revision;
        self.all_commands =
            self.default_commands_with_clipboard(self.current_clipboard_context_kinds(cx));
        true
    }

    fn selected_item(&self) -> Option<&CommandItem> {
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
            LauncherMode::InlinePlugin { session, .. } => {
                log_launcher_enter_started(session.plugin_id(), trace, "inline plugin", None);
                let enter_started = Instant::now();
                if session.on_enter(cx) {
                    log_launcher_step(
                        session.plugin_id(),
                        "inline plugin enter",
                        enter_started,
                        Some(trace),
                    );
                    log_launcher_total(session.plugin_id(), trace);
                    self.close_window_app(window, cx);
                } else {
                    log_launcher_step(
                        session.plugin_id(),
                        "inline plugin enter",
                        enter_started,
                        Some(trace),
                    );
                }
            }
            LauncherMode::ListPlugin {
                session,
                items,
                selected,
                ..
            } => {
                let Some(item) = items.get(*selected).cloned() else {
                    return;
                };
                if item.enabled {
                    log_launcher_enter_started(
                        session.plugin_id(),
                        trace,
                        "list plugin item",
                        Some(item.title.as_str()),
                    );
                    let select_started = Instant::now();
                    self.plugin_manager
                        .borrow()
                        .record_usage_key_background(plugin_list_usage_key(&item).to_string(), cx);
                    session.on_list_item_selected(&item.id, cx);
                    log_launcher_step(
                        session.plugin_id(),
                        "list item selected",
                        select_started,
                        Some(trace),
                    );
                    log_launcher_total(session.plugin_id(), trace);
                    self.close_window_app(window, cx);
                }
            }
        }
    }

    fn open_command(
        &mut self,
        item: CommandItem,
        window: &mut Window,
        cx: &mut App,
        trace: PluginOpenTrace,
    ) {
        let started = Instant::now();
        self.plugin_manager
            .borrow()
            .record_command_launch_background(&item, cx);
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
        let launch_input = item.launch_input(&self.query(cx));
        match visual
            .map(|visual| visual.mode)
            .unwrap_or(PluginWindowMode::Window)
        {
            PluginWindowMode::Window => {
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
            PluginWindowMode::Inline => {
                let session_started = Instant::now();
                let session_result = self
                    .plugin_manager
                    .borrow_mut()
                    .open_inline_view(plugin_id, cx);
                log_launcher_step(
                    plugin_id,
                    "open inline session",
                    session_started,
                    Some(trace),
                );
                match session_result {
                    Ok(mut session) => {
                        let input_started = Instant::now();
                        session.on_input_changed(&launch_input, cx);
                        log_launcher_step(
                            plugin_id,
                            "inline initial input",
                            input_started,
                            Some(trace),
                        );
                        self.mode = LauncherMode::InlinePlugin { session };
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
            PluginWindowMode::List => {
                let session_started = Instant::now();
                let session_result = self
                    .plugin_manager
                    .borrow_mut()
                    .open_list_view(plugin_id, cx);
                log_launcher_step(plugin_id, "open list session", session_started, Some(trace));
                match session_result {
                    Ok(mut session) => {
                        let list_started = Instant::now();
                        let items = session.on_input_changed(&launch_input, cx);
                        log_launcher_step(
                            plugin_id,
                            "list initial input",
                            list_started,
                            Some(trace),
                        );
                        self.mode = LauncherMode::ListPlugin {
                            session,
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
        if let LauncherMode::ListPlugin { session, items, .. } = &mut self.mode {
            let remaining = items.len().saturating_sub(visible_end);
            if remaining > PLUGIN_LIST_PREFETCH_THRESHOLD {
                return;
            }

            let more_items = session.on_input_changed(&query, cx);
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
        if let Some(input) = self.query_input.clone() {
            if !input.read(cx).text().is_empty() {
                input.update(cx, |input, cx| input.clear(cx));
                return;
            }
        }
        self.close_window(window, cx);
    }

    fn close_plugin_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match std::mem::replace(&mut self.mode, LauncherMode::Search) {
            LauncherMode::InlinePlugin { mut session, .. } => session.on_close(),
            LauncherMode::ListPlugin { mut session, .. } => session.on_close(),
            LauncherMode::Search => {}
        }
        self.results_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.results_visible_start = 0;
        self.plugin_list_visible_start = 0;
        let clipboard_kinds = self.current_cached_clipboard_context_kinds();
        let query = self.query(cx);
        self.all_commands = self.default_commands_with_clipboard(clipboard_kinds.clone());
        if query.trim().is_empty() {
            self.results = Self::default_results(&self.all_commands, &self.plugin_visuals);
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            self.results_visible_start = visible_start_for_selection(self.selected);
        } else {
            self.results = Rc::new(self.commands_for_query(&query, clipboard_kinds));
            self.selected = self.selected.min(self.results.len().saturating_sub(1));
            self.results_visible_start = visible_start_for_selection(self.selected);
        }
        self.update_message();
        if let Some(input) = self.query_input.clone() {
            window.focus(&input.focus_handle(cx));
        }
        cx.notify();
    }

    fn close_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(controller) = self.window_controller.as_ref() {
            controller.borrow_mut().clear_launcher_window();
        }
        window.defer(cx, |window, _cx| window.remove_window());
    }

    fn close_window_app(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(controller) = self.window_controller.as_ref() {
            controller.borrow_mut().clear_launcher_window();
        }
        window.defer(cx, |window, _cx| window.remove_window());
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_started = Instant::now();
        if matches!(self.mode, LauncherMode::ListPlugin { .. }) {
            self.refresh_plugin_list_if_needed(cx);
        }
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
        let handle = self.self_handle.clone();
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
        let title_color = if dark { rgb(0xddd8ec) } else { rgb(0x333348) };
        let placeholder_color = theme::launcher_faint_text(dark);
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
            .bg(theme::launcher_glass(dark))
            .font_family("Inter, PingFang SC")
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
                    .border_color(theme::launcher_soft_line(dark))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .size(px(24.0))
                            .rounded(px(8.0))
                            .bg(theme::launcher_keycap(dark))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.0))
                            .text_color(theme::launcher_muted_text(dark))
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
                                dark,
                            ))
                            .child(launcher_quick_tab(
                                handle.clone(),
                                "system-settings",
                                "设置",
                                "SET",
                                dark,
                            ))
                            .child(
                                div()
                                    .h(px(24.0))
                                    .px(px(10.0))
                                    .rounded(px(6.0))
                                    .bg(theme::launcher_keycap(dark))
                                    .border_1()
                                    .border_color(theme::launcher_soft_line(dark))
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
                                .text_color(theme::launcher_muted_text(dark))
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
                                                        dark,
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
                let content = match &mut self.mode {
                    LauncherMode::InlinePlugin { session, .. } => session.render(window, cx),
                    _ => div().into_any_element(),
                };
                launcher.child(
                    div()
                        .id("launcher-inline-plugin")
                        .w_full()
                        .h(px(RESULTS_MAX_HEIGHT))
                        .overflow_hidden()
                        .child(content),
                )
            })
            .when(!search_mode && !inline_mode, |launcher| {
                launcher.child(plugin_list(
                    handle.clone(),
                    self.plugin_list_scroll.clone(),
                    list_items,
                    list_selected,
                    query,
                    dark,
                ))
            })
    }
}

fn launcher_quick_tab(
    handle: Option<Entity<Launcher>>,
    plugin_id: &'static str,
    label: &'static str,
    icon_label: &'static str,
    dark: bool,
) -> impl IntoElement {
    div()
        .h(px(24.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .bg(theme::launcher_keycap(dark))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .cursor_pointer()
        .hover(move |style| {
            style
                .bg(theme::launcher_row_selected(dark))
                .cursor_pointer()
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            let Some(handle) = handle.clone() else {
                return;
            };
            let _ = cx.update_entity(&handle, |launcher, entity_cx| {
                if let Some(item) = launcher
                    .all_commands
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
                .text_color(theme::launcher_muted_text(dark))
                .child(icon_label),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::launcher_muted_text(dark))
                .child(label),
        )
}

fn plugin_list(
    handle: Option<Entity<Launcher>>,
    scroll: UniformListScrollHandle,
    items: Rc<Vec<PluginListItem>>,
    selected: usize,
    query: String,
    dark: bool,
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
                    .text_color(theme::launcher_muted_text(dark))
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
                    .text_color(theme::launcher_muted_text(dark))
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
                let _ = cx.update_entity(&list_handle, |launcher, cx| {
                    launcher.maybe_prefetch_plugin_items(range.end, cx);
                });
                range
                    .map(|idx| {
                        let item = items[idx].clone();
                        div()
                            .h(px(ROW_SLOT))
                            .flex_none()
                            .pb(px(ROW_GAP))
                            .child(plugin_list_row(
                                handle.clone(),
                                item,
                                idx == selected,
                                idx,
                                dark,
                            ))
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
    dark: bool,
) -> impl IntoElement {
    let item_for_click = item.clone();
    let accent = theme::launcher_accent(dark);
    let row_bg = if selected {
        theme::rgba_with_alpha(accent, if dark { 0.12 } else { 0.08 })
    } else {
        hsla(0.0, 0.0, 0.0, 0.0)
    };
    let title_color = if selected {
        accent
    } else if dark {
        rgb(0xddd8ec)
    } else {
        rgb(0x333348)
    };
    let subtitle_color = theme::launcher_faint_text(dark);
    let icon_surface = if selected {
        theme::rgba_with_alpha(rgb(0xf2f2f7), if dark { 0.15 } else { 0.9 })
    } else if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        theme::rgba_with_alpha(rgb(0xf8f8fb), 0.78)
    };
    let icon_border = if selected {
        theme::rgba_with_alpha(rgb(0xe2e2ea), if dark { 0.2 } else { 0.9 })
    } else if dark {
        hsla(0.0, 0.0, 1.0, 0.04)
    } else {
        theme::rgba_with_alpha(rgb(0xe7e7ee), 0.72)
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
            let _ = cx.update_entity(&handle, |launcher: &mut Launcher, entity_cx| {
                if let LauncherMode::ListPlugin {
                    session,
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
                            .borrow()
                            .record_usage_key_background(
                                plugin_list_usage_key(&item_for_click).to_string(),
                                entity_cx,
                            );
                        session.on_list_item_selected(&item_for_click.id, entity_cx);
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

fn clipboard_record_context_kinds(record: &ClipboardRecord) -> Vec<ContextKind> {
    let mut kinds = vec![ContextKind::Clipboard];
    match record.kind {
        ClipboardItemKind::Text => {
            kinds.extend(detect_text_context_kinds(
                non_empty_str(&record.content).unwrap_or(&record.preview),
            ));
        }
        ClipboardItemKind::Image => {
            push_context_kind(&mut kinds, ContextKind::Image);
        }
        ClipboardItemKind::Files => {
            push_context_kind(&mut kinds, ContextKind::File);
            for path in clipboard_file_candidates(record) {
                if detect_text_context_kinds(&path).contains(&ContextKind::ImageFile) {
                    push_context_kind(&mut kinds, ContextKind::ImageFile);
                }
            }
        }
    }
    unique_context_kinds(kinds)
}

fn non_empty_str(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn clipboard_file_candidates(record: &ClipboardRecord) -> Vec<String> {
    let raw = record
        .content
        .lines()
        .chain(record.preview.lines())
        .flat_map(|line| line.split(['\r', '\t']))
        .map(|part| part.trim().trim_matches('"').trim_matches('\''))
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let mut candidates = Vec::new();
    for value in raw {
        candidates.push(value.clone());
        if value.starts_with('[')
            && let Ok(paths) = serde_json::from_str::<Vec<String>>(&value)
        {
            candidates.extend(paths);
        } else {
            candidates.extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(ToOwned::to_owned),
            );
        }
    }
    candidates
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
    item: CommandItem,
    selected: bool,
    index: usize,
    dark: bool,
) -> impl IntoElement {
    let item_for_click = item.clone();
    let accent = theme::launcher_accent(dark);

    let icon_surface = if selected {
        theme::rgba_with_alpha(rgb(0xf2f2f7), if dark { 0.15 } else { 0.9 })
    } else if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        theme::rgba_with_alpha(rgb(0xf8f8fb), 0.78)
    };
    let icon_border = if selected {
        theme::rgba_with_alpha(rgb(0xe2e2ea), if dark { 0.2 } else { 0.9 })
    } else if dark {
        hsla(0.0, 0.0, 1.0, 0.04)
    } else {
        theme::rgba_with_alpha(rgb(0xe7e7ee), 0.72)
    };

    let (badge_label, badge_bg, badge_fg) = result_badge(&item, dark);

    let row_bg = if selected {
        if dark {
            theme::rgba_with_alpha(accent, 0.12)
        } else {
            theme::rgba_with_alpha(rgb(0xf6f6fa), 0.96)
        }
    } else {
        hsla(0.0, 0.0, 0.0, 0.0)
    };
    let row_border = if selected {
        if dark {
            theme::rgba_with_alpha(accent, 0.2)
        } else {
            theme::rgba_with_alpha(rgb(0xe2e2ea), 0.95)
        }
    } else {
        hsla(0.0, 0.0, 1.0, 0.0)
    };

    let title_color = if selected {
        accent
    } else if dark {
        rgb(0xddd8ec)
    } else {
        rgb(0x333348)
    };
    let subtitle_color = theme::launcher_faint_text(dark);
    let hover_bg = if dark {
        hsla(0.0, 0.0, 1.0, 0.025)
    } else {
        theme::rgba_with_alpha(rgb(0xf7f7fa), 0.72)
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
                color: hsla(0.72, 0.72, 0.56, 0.04),
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
            let _ = cx.update_entity(&handle, |launcher, entity_cx| {
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
            launcher_icon_tint(&item.plugin_id, dark),
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
    item: &CommandItem,
    surface: gpui::Hsla,
    border: gpui::Hsla,
    tint: gpui::Rgba,
) -> impl IntoElement {
    let icon = item.icon.clone();

    div()
        .size(px(36.0))
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

fn launcher_icon_tint(plugin_id: &str, dark: bool) -> gpui::Rgba {
    if dark {
        match plugin_id {
            "api-debugger" => rgb(0xc8b8ff),
            "clipboard" => rgb(0x88dd88),
            "http-capture" => rgb(0xff8888),
            "image-compress" => rgb(0xffcc44),
            "json-parser" => rgb(0xaaccff),
            "ftp-sftp-ssh-client" => rgb(0x88ddff),
            "system-settings" => rgb(0xaaccff),
            _ => theme::launcher_accent(dark),
        }
    } else {
        match plugin_id {
            "api-debugger" => rgb(0x6b4fcf),
            "clipboard" => rgb(0x55aa55),
            "http-capture" => rgb(0xcc6666),
            "image-compress" => rgb(0xccaa33),
            "json-parser" => rgb(0x6688cc),
            "ftp-sftp-ssh-client" => rgb(0x5599cc),
            "system-settings" => rgb(0x6688cc),
            _ => theme::launcher_accent(dark),
        }
    }
}

fn launcher_icon_label(item: &CommandItem) -> &'static str {
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

fn result_badge(item: &CommandItem, dark: bool) -> (String, gpui::Hsla, gpui::Rgba) {
    let tag_bg = if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        theme::rgba_with_alpha(rgb(0xf7f7fa), 0.82)
    };
    let tag_fg = theme::launcher_faint_text(dark);

    match item.kind {
        CommandKind::App => (String::from("应用"), tag_bg, tag_fg),
        CommandKind::DynamicAction => (
            String::from("动作"),
            theme::rgba_with_alpha(theme::launcher_accent(dark), if dark { 0.12 } else { 0.08 }),
            theme::launcher_accent(dark),
        ),
        CommandKind::Plugin => match item.plugin_id.as_str() {
            "system-settings" => (String::from("系统"), tag_bg, tag_fg),
            "about" => (String::from("关于"), tag_bg, tag_fg),
            _ => (String::from("工具"), tag_bg, tag_fg),
        },
    }
}
