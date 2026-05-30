use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ObjectFit, ParentElement, Render, ScrollStrategy,
    StatefulInteractiveElement, Styled, StyledImage, Subscription, Task, UniformListScrollHandle,
    Window, div, hsla, img, px,
};

use crate::{
    app::{
        text_input::{TextInput, TextInputStyle},
        theme, ui,
    },
    core::shortcut::ShortcutService,
    features::clipboard::{
        history_store::{self, ClipboardConfig, ClipboardRecord},
        service::{ClipboardFilter, ClipboardService},
    },
};

mod history;
mod settings;
mod shared;

use history::{history_page, keyboard_filters};
use settings::{format_ignore_patterns, settings_page};

const HISTORY_PAGE_SIZE: usize = 120;
const HISTORY_PREFETCH_THRESHOLD: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClipboardTab {
    History,
    Settings,
}

pub struct ClipboardPanel {
    focus_handle: Option<FocusHandle>,
    service: Arc<Mutex<ClipboardService>>,
    query_input: Option<Entity<TextInput>>,
    preview_input: Option<Entity<TextInput>>,
    ignore_patterns_input: Option<Entity<TextInput>>,
    max_text_chars_input: Option<Entity<TextInput>>,
    hotkey_input: Option<Entity<TextInput>>,
    query: String,
    filter: ClipboardFilter,
    items: Arc<Vec<ClipboardRecord>>,
    selected: usize,
    tab: ClipboardTab,
    message: String,
    status_text: String,
    focus_pending: bool,
    load_generation: u64,
    loading: bool,
    has_more: bool,
    load_task: Option<Task<()>>,
    action_task: Option<Task<()>>,
    history_scroll: UniformListScrollHandle,
    preview_file_scroll: UniformListScrollHandle,
    subscriptions: Vec<Subscription>,
}

impl ClipboardPanel {
    pub(crate) fn new(service: Arc<Mutex<ClipboardService>>) -> Self {
        Self {
            focus_handle: None,
            service,
            query_input: None,
            preview_input: None,
            ignore_patterns_input: None,
            max_text_chars_input: None,
            hotkey_input: None,
            query: String::new(),
            filter: ClipboardFilter::All,
            items: Arc::new(Vec::new()),
            selected: 0,
            tab: ClipboardTab::History,
            message: String::new(),
            status_text: String::new(),
            focus_pending: false,
            load_generation: 0,
            loading: false,
            has_more: false,
            load_task: None,
            action_task: None,
            history_scroll: UniformListScrollHandle::new(),
            preview_file_scroll: UniformListScrollHandle::new(),
            subscriptions: Vec::new(),
        }
    }

    pub(crate) fn init(&mut self, cx: &mut Context<Self>) {
        self.focus_handle = Some(cx.focus_handle());
        self.ensure_inputs(cx);
        self.observe_query_input(cx);
    }

    pub(crate) fn refresh_async(&mut self, cx: &mut Context<Self>) {
        self.items = Arc::new(Vec::new());
        self.selected = 0;
        self.has_more = false;
        self.loading = true;
        self.history_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.preview_file_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.schedule_load(true, cx);
    }

    pub(crate) fn reopen(&mut self, cx: &mut Context<Self>) {
        self.tab = ClipboardTab::History;
        self.focus_pending = false;
        if let Ok(service) = self.service.lock() {
            let _ = service.capture_current(cx);
        }
        self.message.clear();
        self.refresh_async(cx);
        self.status_text = self.status_text();
        cx.notify();
    }

    fn set_filter_async(&mut self, filter: ClipboardFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        self.message.clear();
        self.refresh_async(cx);
    }

    fn ensure_inputs(&mut self, cx: &mut Context<Self>) {
        if self.query_input.is_none() {
            let query = self.query.clone();
            let input = cx.new(|cx| {
                let mut input = TextInput::new(cx, "搜索内容...", query);
                input.set_style(
                    TextInputStyle {
                        height: 26.0,
                        font_size: 12.0,
                        padding: 0.0,
                    },
                    cx,
                );
                input.set_chrome(false, cx);
                input
            });
            self.query_input = Some(input);
        }

        if self.preview_input.is_none() {
            let input = cx.new(|cx| {
                let mut input = TextInput::new(cx, "", "");
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_read_only(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 9999.0,
                        font_size: 12.0,
                        padding: 0.0,
                    },
                    cx,
                );
                input
            });
            self.preview_input = Some(input);
        }

        let config = self.settings_snapshot();
        if self.ignore_patterns_input.is_none() {
            let value = format_ignore_patterns(&config);
            let input = cx.new(|cx| {
                let mut input = TextInput::new(cx, "每行一条规则，或使用 | 分隔", value);
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 96.0,
                        font_size: 12.0,
                        padding: 10.0,
                    },
                    cx,
                );
                input
            });
            self.ignore_patterns_input = Some(input);
        }

        if self.max_text_chars_input.is_none() {
            let value = config.max_text_chars.to_string();
            let input = cx.new(|cx| {
                let mut input = TextInput::new(cx, "例如 20000，0 表示不限", value);
                input.set_style(
                    TextInputStyle {
                        height: 34.0,
                        font_size: 12.0,
                        padding: 9.0,
                    },
                    cx,
                );
                input.set_chrome(false, cx);
                input
            });
            self.max_text_chars_input = Some(input);
        }

        if self.hotkey_input.is_none() {
            let value = config.hotkey.clone();
            let input = cx.new(|cx| {
                let mut input = TextInput::new(cx, "例如 Alt+V", value);
                input.set_style(
                    TextInputStyle {
                        height: 34.0,
                        font_size: 12.0,
                        padding: 9.0,
                    },
                    cx,
                );
                input.set_chrome(false, cx);
                input
            });
            self.hotkey_input = Some(input);
        }
    }

    fn observe_query_input(&mut self, cx: &mut Context<Self>) {
        let Some(query_input) = self.query_input.clone() else {
            return;
        };
        let subscription = cx.observe(&query_input, |panel, _, cx| {
            panel.sync_query_from_input(cx);
            cx.notify();
        });
        self.subscriptions.push(subscription);
    }

    fn sync_query_from_input(&mut self, cx: &mut Context<Self>) {
        let Some(query_input) = self.query_input.as_ref() else {
            return;
        };
        let next_query = query_input.read(cx).text();
        if next_query == self.query {
            return;
        }
        self.sync_query_text(next_query, cx);
    }

    fn sync_query_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.query = text;
        self.message.clear();
        self.refresh_async(cx);
    }

    fn maybe_prefetch_history(&mut self, visible_end: usize, cx: &mut Context<Self>) {
        if self.loading || !self.has_more {
            return;
        }

        let remaining = self.items.len().saturating_sub(visible_end);
        if remaining <= HISTORY_PREFETCH_THRESHOLD {
            self.loading = true;
            self.schedule_load(false, cx);
        }
    }

    fn schedule_load(&mut self, reset: bool, cx: &mut Context<Self>) {
        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;
        let service = Arc::clone(&self.service);
        let query = self.query.clone();
        let filter = self.filter;
        let offset = if reset { 0 } else { self.items.len() };
        let limit = HISTORY_PAGE_SIZE;

        self.load_task = Some(cx.spawn(async move |panel, async_cx| {
            let rows_result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.search(&query, filter, offset, limit + 1)
                })
                .await;

            let _ = panel.update(async_cx, |panel, cx| {
                if panel.load_generation != generation {
                    return;
                }
                panel.loading = false;
                match rows_result {
                    Ok(rows) => panel.apply_loaded_rows(rows, reset, limit, cx),
                    Err(error) => panel.message = format!("加载失败: {error}"),
                }
                if reset && !panel.items.is_empty() {
                    panel
                        .history_scroll
                        .scroll_to_item(panel.selected, ScrollStrategy::Top);
                }
                panel.status_text = panel.status_text();
                cx.notify();
            });
        }));
    }

    fn apply_loaded_rows(
        &mut self,
        mut rows: Vec<ClipboardRecord>,
        reset: bool,
        limit: usize,
        cx: &mut Context<Self>,
    ) {
        self.has_more = rows.len() > limit;
        if self.has_more {
            rows.truncate(limit);
        }

        if reset {
            self.items = Arc::new(rows);
        } else {
            let mut merged = self.items.as_ref().clone();
            merged.extend(rows);
            self.items = Arc::new(merged);
        }

        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
        self.sync_preview_input(cx);
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = index.min(self.items.len() - 1);
        self.history_scroll
            .scroll_to_item(self.selected, ScrollStrategy::Center);
        self.preview_file_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.sync_preview_input(cx);
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if self.tab != ClipboardTab::History || self.items.is_empty() {
            return false;
        }

        let len = self.items.len() as isize;
        let next = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.selected = next;
        self.history_scroll
            .scroll_to_item(self.selected, ScrollStrategy::Center);
        self.preview_file_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.sync_preview_input(cx);
        self.status_text = self.status_text();
        cx.notify();
        true
    }

    fn cycle_filter_to(&mut self, filter: ClipboardFilter, cx: &mut Context<Self>) -> bool {
        if self.tab != ClipboardTab::History {
            self.tab = ClipboardTab::History;
        }
        if self.filter == filter {
            return false;
        }
        self.set_filter_async(filter, cx);
        self.status_text = self.status_text();
        true
    }

    fn set_filter_shortcut(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        if self.tab != ClipboardTab::History {
            self.tab = ClipboardTab::History;
        }
        let Some(filter) = keyboard_filters().get(index).copied() else {
            return false;
        };
        self.set_filter_async(filter, cx);
        self.status_text = self.status_text();
        true
    }

    fn cycle_visible_filter(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if self.tab != ClipboardTab::History {
            self.tab = ClipboardTab::History;
        }
        let filters = keyboard_filters();
        let current = filters
            .iter()
            .position(|filter| *filter == self.filter)
            .unwrap_or(0) as isize;
        let len = filters.len() as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        self.set_filter_async(filters[next], cx);
        self.status_text = self.status_text();
        true
    }

    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = ClipboardTab::History;
        if let Some(input) = self.query_input.clone() {
            window.focus(&input.focus_handle(cx));
            input.update(cx, |input, cx| input.select_all_text(cx));
        }
        self.focus_pending = false;
    }

    fn query_focused(&self, window: &Window, cx: &App) -> bool {
        self.query_input
            .as_ref()
            .is_some_and(|input| input.focus_handle(cx).is_focused(window))
    }

    fn settings_input_focused(&self, window: &Window, cx: &App) -> bool {
        [
            self.ignore_patterns_input.as_ref(),
            self.max_text_chars_input.as_ref(),
            self.hotkey_input.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|input| input.focus_handle(cx).is_focused(window))
    }

    fn delete_key_owned_by_input(&self, window: &Window, cx: &App) -> bool {
        self.settings_input_focused(window, cx)
            || (self.query_focused(window, cx) && !self.query.is_empty())
    }

    fn copy_key_owned_by_input(&self, window: &Window, cx: &App) -> bool {
        self.settings_input_focused(window, cx)
            || (self.query_focused(window, cx) && !self.query.is_empty())
    }

    fn navigation_key_owned_by_input(&self, window: &Window, cx: &App) -> bool {
        self.settings_input_focused(window, cx) || self.query_focused(window, cx)
    }

    fn sync_preview_input(&self, cx: &mut Context<Self>) {
        let text = self
            .items
            .get(self.selected)
            .map(preview_text_for_record_for_panel)
            .unwrap_or_default();
        if let Some(input) = self.preview_input.as_ref() {
            input.update(cx, |input, input_cx| {
                if input.text() != text {
                    input.set_text(text.clone(), input_cx);
                }
            });
        }
    }

    fn handle_panel_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let primary = modifiers.platform || modifiers.control;

        if key == "escape" {
            window.defer(cx, |window, _cx| window.remove_window());
            cx.stop_propagation();
            return;
        }

        if primary {
            let handled = match key {
                "f" => {
                    self.focus_search(window, cx);
                    true
                }
                "p" if !self.settings_input_focused(window, cx) => {
                    self.toggle_selected_pin(cx);
                    true
                }
                "c" if !self.copy_key_owned_by_input(window, cx) => {
                    self.copy_selected(cx);
                    true
                }
                "1" => self.set_filter_shortcut(0, cx),
                "2" => self.set_filter_shortcut(1, cx),
                "3" => self.set_filter_shortcut(2, cx),
                "4" => self.set_filter_shortcut(3, cx),
                "5" => self.set_filter_shortcut(4, cx),
                "6" => self.cycle_filter_to(ClipboardFilter::Link, cx),
                "7" => self.cycle_filter_to(ClipboardFilter::Code, cx),
                _ => false,
            };
            if handled {
                cx.notify();
                cx.stop_propagation();
                return;
            }
        }

        let handled = match key {
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            "enter" if !self.settings_input_focused(window, cx) => {
                self.copy_selected(cx);
                window.defer(cx, |window, _cx| window.remove_window());
                true
            }
            "backspace" | "delete" if !self.delete_key_owned_by_input(window, cx) => {
                self.delete_selected(cx);
                true
            }
            "left" if !self.navigation_key_owned_by_input(window, cx) => {
                self.cycle_visible_filter(-1, cx)
            }
            "right" if !self.navigation_key_owned_by_input(window, cx) => {
                self.cycle_visible_filter(1, cx)
            }
            _ => false,
        };

        if handled {
            cx.notify();
            cx.stop_propagation();
        }
    }

    fn copy_selected(&mut self, cx: &mut App) {
        let Some(item) = self.items.get(self.selected) else {
            return;
        };
        let result = self
            .service
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))
            .and_then(|service| service.copy_record_to_clipboard(item, cx));
        self.message = if result.is_ok() {
            String::from("已写回系统剪贴板")
        } else {
            String::from("写回剪贴板失败")
        };
        self.status_text = self.message.clone();
    }

    fn focus_panel(&self, window: &mut Window, cx: &App) {
        if let Some(focus_handle) = self.focus_handle.as_ref() {
            window.focus(focus_handle);
            let _ = cx;
        }
    }

    fn toggle_selected_pin(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(self.selected) else {
            return;
        };
        let id = item.id;
        let service = Arc::clone(&self.service);
        self.message = String::from("正在更新置顶状态...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.toggle_pin(id)
                })
                .await;

            let _ = panel.update(async_cx, |panel, cx| {
                match result {
                    Ok(Some(true)) => panel.message = String::from("已置顶"),
                    Ok(Some(false)) => panel.message = String::from("已取消置顶"),
                    Ok(None) | Err(_) => panel.message = String::from("置顶失败"),
                }
                panel.status_text = panel.message.clone();
                panel.refresh_async(cx);
            });
        }));
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(self.selected) else {
            return;
        };
        let id = item.id;
        let service = Arc::clone(&self.service);
        self.message = String::from("正在删除...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.delete(id)
                })
                .await;

            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = if matches!(result, Ok(true)) {
                    String::from("已删除")
                } else {
                    String::from("删除失败")
                };
                panel.status_text = panel.message.clone();
                panel.refresh_async(cx);
            });
        }));
    }

    fn open_selected_parent_dir(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(self.selected) else {
            self.message = String::from("没有选中记录");
            return;
        };
        if item.kind != history_store::ClipboardItemKind::Files {
            return;
        }
        let paths = history_store::parse_file_paths(&item.content);
        if paths.is_empty() {
            self.message = String::from("文件记录不包含有效路径");
            self.status_text = self.status_text();
            cx.notify();
            return;
        }

        let first_actionable = history_store::find_first_actionable_path(&paths);
        match first_actionable {
            Some(target) => match crate::platform::shell::open_path(&target) {
                Ok(()) => {
                    self.message = format!("已打开目录 {}", target.display());
                }
                Err(e) => {
                    self.message = format!("打开失败: {e}");
                }
            },
            None => {
                self.message = String::from("所有文件路径的父目录都已不存在");
            }
        }
        self.status_text = self.status_text();
        cx.notify();
    }

    /// Reveal a single file path in Finder. The path must exist on disk.
    fn reveal_path_in_finder(&mut self, path: &str, cx: &mut Context<Self>) {
        let p = Path::new(path);
        if !p.exists() {
            self.message = format!("文件已不存在: {}", path);
            self.status_text = self.status_text();
            cx.notify();
            return;
        }
        match crate::platform::shell::reveal_in_finder(p) {
            Ok(()) => {
                self.message = format!("已在访达中显示: {}", path);
            }
            Err(e) => {
                self.message = format!("操作失败: {e}");
            }
        }
        self.status_text = self.status_text();
        cx.notify();
    }

    /// Reveal the first existing file path from the selected record in Finder.
    fn reveal_first_existing_in_finder(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(self.selected) else {
            self.message = String::from("没有选中记录");
            self.status_text = self.status_text();
            cx.notify();
            return;
        };
        if item.kind != history_store::ClipboardItemKind::Files {
            return;
        }
        let paths = history_store::parse_file_paths(&item.content);
        if paths.is_empty() {
            self.message = String::from("文件记录不包含有效路径");
            self.status_text = self.status_text();
            cx.notify();
            return;
        }
        match history_store::find_first_existing_path(&paths) {
            Some(existing) => {
                self.reveal_path_in_finder(&existing.to_string_lossy(), cx);
            }
            None => {
                self.message = String::from("所有文件路径都已不存在");
                self.status_text = self.status_text();
                cx.notify();
            }
        }
    }

    fn clear_unpinned(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        self.message = String::from("正在清理未置顶记录...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.clear_unpinned()
                })
                .await;

            let _ = panel.update(async_cx, |panel, cx| {
                let count = result.unwrap_or(0);
                panel.message = format!("已清理 {count} 条未置顶记录");
                panel.status_text = panel.message.clone();
                panel.refresh_async(cx);
            });
        }));
    }

    fn clear_all(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        self.message = String::from("正在清空记录...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.clear_all()
                })
                .await;

            let _ = panel.update(async_cx, |panel, cx| {
                let count = result.unwrap_or(0);
                panel.message = format!("已清空 {count} 条记录");
                panel.status_text = panel.message.clone();
                panel.refresh_async(cx);
            });
        }));
    }

    fn set_tab(&mut self, tab: ClipboardTab) {
        self.tab = tab;
        self.status_text = self.status_text();
    }

    fn settings_snapshot(&self) -> ClipboardConfig {
        self.service
            .lock()
            .map(|service| service.config())
            .unwrap_or_default()
    }

    fn toggle_capture_text(&mut self, cx: &mut Context<Self>) {
        let enabled = !self.settings_snapshot().capture_text;
        let service = Arc::clone(&self.service);
        self.message = String::from("正在保存设置...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.set_capture_text(enabled)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = match result {
                    Ok(_) if enabled => String::from("已开启文本采集"),
                    Ok(_) => String::from("已关闭文本采集"),
                    Err(error) => format!("保存设置失败: {error}"),
                };
                panel.status_text = panel.message.clone();
                cx.notify();
            });
        }));
    }

    fn toggle_capture_image(&mut self, cx: &mut Context<Self>) {
        let enabled = !self.settings_snapshot().capture_image;
        let service = Arc::clone(&self.service);
        self.message = String::from("正在保存设置...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.set_capture_image(enabled)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = match result {
                    Ok(_) if enabled => String::from("已开启图片采集"),
                    Ok(_) => String::from("已关闭图片采集"),
                    Err(error) => format!("保存设置失败: {error}"),
                };
                panel.status_text = panel.message.clone();
                cx.notify();
            });
        }));
    }

    fn toggle_capture_files(&mut self, cx: &mut Context<Self>) {
        let enabled = !self.settings_snapshot().capture_files;
        let service = Arc::clone(&self.service);
        self.message = String::from("正在保存设置...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.set_capture_files(enabled)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = match result {
                    Ok(_) if enabled => String::from("已开启文件采集"),
                    Ok(_) => String::from("已关闭文件采集"),
                    Err(error) => format!("保存设置失败: {error}"),
                };
                panel.status_text = panel.message.clone();
                cx.notify();
            });
        }));
    }

    fn set_max_text_chars(&mut self, next: usize, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        self.message = String::from("正在保存设置...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.set_max_text_chars(next)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = match result {
                    Ok(_) => {
                        panel.sync_max_text_chars_input(next, cx);
                        format!("最大文本长度已调整为 {next}")
                    }
                    Err(error) => format!("保存设置失败: {error}"),
                };
                panel.status_text = panel.message.clone();
                cx.notify();
            });
        }));
    }

    fn save_max_text_chars(&mut self, cx: &mut Context<Self>) {
        let text = self
            .max_text_chars_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default();
        let next = match text.trim().parse::<usize>() {
            Ok(value) => value,
            Err(_) => {
                self.message = String::from("文本长度上限需要是数字");
                return;
            }
        };
        self.set_max_text_chars(next, cx);
    }

    fn save_ignore_patterns(&mut self, cx: &mut Context<Self>) {
        let text = self
            .ignore_patterns_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default();
        let patterns = ClipboardService::parse_ignore_patterns(&text);
        let service = Arc::clone(&self.service);
        self.message = String::from("正在保存规则...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let patterns_for_task = patterns.clone();
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.set_ignore_patterns(patterns_for_task)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = match result {
                    Ok(_) => {
                        panel.sync_ignore_patterns_input(&patterns, cx);
                        String::from("过滤规则已保存")
                    }
                    Err(error) => format!("保存设置失败: {error}"),
                };
                panel.status_text = panel.message.clone();
                cx.notify();
            });
        }));
    }

    fn clear_ignore_patterns(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        self.message = String::from("正在清空规则...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.set_ignore_patterns(Vec::new())
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = match result {
                    Ok(_) => {
                        panel.sync_ignore_patterns_input(&[], cx);
                        String::from("过滤规则已清空")
                    }
                    Err(error) => format!("保存设置失败: {error}"),
                };
                panel.status_text = panel.message.clone();
                cx.notify();
            });
        }));
    }

    fn save_hotkey(&mut self, cx: &mut Context<Self>) {
        let text = self
            .hotkey_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default();
        let normalized = match ClipboardService::normalize_hotkey(&text) {
            Some(value) => value,
            None => {
                self.message = String::from("快捷键格式无效");
                return;
            }
        };
        let service = Arc::clone(&self.service);
        self.message = String::from("正在保存快捷键...");
        self.status_text = self.message.clone();
        self.action_task = Some(cx.spawn(async move |panel, async_cx| {
            let normalized_for_task = normalized.clone();
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let service = service
                        .lock()
                        .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                    service.set_hotkey(normalized_for_task)
                })
                .await;
            let _ = panel.update(async_cx, |panel, cx| {
                panel.message = match result {
                    Ok(_) => {
                        panel.sync_hotkey_input(&normalized, cx);
                        let refresh_result = cx
                            .update_global::<ShortcutService, _>(|service, cx| service.refresh(cx));
                        if let Err(error) = refresh_result {
                            format!("剪贴板快捷键已保存为 {normalized}，刷新注册失败: {error}")
                        } else {
                            format!("剪贴板快捷键已保存为 {normalized}")
                        }
                    }
                    Err(error) => format!("保存设置失败: {error}"),
                };
                panel.status_text = panel.message.clone();
                cx.notify();
            });
        }));
    }

    fn sync_ignore_patterns_input(&self, patterns: &[String], cx: &mut Context<Self>) {
        if let Some(input) = self.ignore_patterns_input.as_ref() {
            input.update(cx, |input, input_cx| {
                input.set_text(patterns.join("\n"), input_cx);
            });
        }
    }

    fn sync_max_text_chars_input(&self, value: usize, cx: &mut Context<Self>) {
        if let Some(input) = self.max_text_chars_input.as_ref() {
            input.update(cx, |input, input_cx| {
                input.set_text(value.to_string(), input_cx);
            });
        }
    }

    fn sync_hotkey_input(&self, value: &str, cx: &mut Context<Self>) {
        if let Some(input) = self.hotkey_input.as_ref() {
            input.update(cx, |input, input_cx| {
                input.set_text(value, input_cx);
            });
        }
    }

    fn status_text(&self) -> String {
        if !self.message.is_empty() {
            return self.message.clone();
        }

        if self.tab == ClipboardTab::Settings {
            let config = self.settings_snapshot();
            return format!(
                "设置 · 文本{} · 图片{} · 文件{} · {} 条过滤规则 · 快捷键 {}",
                if config.capture_text {
                    "开启"
                } else {
                    "关闭"
                },
                if config.capture_image {
                    "开启"
                } else {
                    "关闭"
                },
                if config.capture_files {
                    "开启"
                } else {
                    "关闭"
                },
                config.ignore_patterns.len(),
                config.hotkey
            );
        }

        if self.loading {
            let count = self.items.len();
            if count > 0 {
                return format!("{} · 已加载 {} 条，正在预取...", self.filter.label(), count);
            }
            return format!("{} · 正在加载...", self.filter.label());
        }

        if self.items.is_empty() {
            if self.query.trim().is_empty() {
                return format!("{} · 暂无剪贴板记录", self.filter.label());
            }
            return format!("{} · 没有匹配记录", self.filter.label());
        }

        let count = self.items.len();
        if self.query.trim().is_empty() {
            let more = if self.has_more {
                " · 继续滚动加载更多"
            } else {
                ""
            };
            format!("{} · 已加载 {} 条记录{more}", self.filter.label(), count)
        } else {
            let more = if self.has_more {
                " · 继续滚动加载更多"
            } else {
                ""
            };
            format!(
                "{} · 关键词“{}”匹配到 {} 条记录{more}",
                self.filter.label(),
                self.query,
                count
            )
        }
    }
}

fn render_tab_bar(
    handle: Entity<ClipboardPanel>,
    active: ClipboardTab,
    dark: bool,
) -> impl IntoElement {
    let tabs = [
        (ClipboardTab::History, "历史记录"),
        (ClipboardTab::Settings, "设置"),
    ];

    div()
        .h(px(36.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(theme::semantic(dark).border_default)
        .bg(theme::semantic(dark).bg_page)
        .flex()
        .items_center()
        .gap(px(4.0))
        .children(tabs.into_iter().enumerate().map(|(idx, (tab, label))| {
            let is_active = active == tab;
            let h = handle.clone();
            div()
                .id(("clipboard-tab", idx as u64))
                .h(px(28.0))
                .px(px(12.0))
                .rounded(px(6.0))
                .bg(if is_active {
                    theme::semantic(dark).bg_surface.into()
                } else {
                    hsla(0.0, 0.0, 0.0, 0.0)
                })
                .text_color(if is_active {
                    theme::semantic(dark).text_primary
                } else {
                    theme::semantic(dark).text_secondary
                })
                .font_weight(if is_active {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::NORMAL
                })
                .text_size(px(12.0))
                .cursor_pointer()
                .hover(move |style| {
                    style.bg(if !is_active {
                        theme::semantic(dark).row_hover.into()
                    } else {
                        hsla(0.0, 0.0, 0.0, 0.0)
                    })
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    let _ = cx.update_entity(&h, |panel, cx| {
                        panel.set_tab(tab);
                        cx.notify();
                    });
                })
                .flex()
                .items_center()
                .justify_center()
                .child(label)
        }))
}

impl Render for ClipboardPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_pending {
            if let Some(query_input) = self.query_input.as_ref() {
                window.focus(&query_input.focus_handle(cx));
            }
            self.focus_pending = false;
        } else if let Some(focus_handle) = self.focus_handle.as_ref()
            && !focus_handle.is_focused(window)
            && !self.query_focused(window, cx)
            && !self.settings_input_focused(window, cx)
        {
            window.focus(focus_handle);
        }

        let handle = cx.entity();
        let tab = self.tab;
        let current_filter = self.filter;
        let items = self.items.clone();
        let selected = self.selected;
        let query = self.query.clone();
        let query_input = self.query_input.clone().expect("query input missing");
        let preview_input = self.preview_input.clone().expect("preview input missing");
        let item_count = self.items.len();
        let selected_record = self.items.get(self.selected).cloned();
        let settings_inputs = (
            self.ignore_patterns_input
                .clone()
                .expect("ignore patterns input missing"),
            self.max_text_chars_input
                .clone()
                .expect("max text chars input missing"),
            self.hotkey_input.clone().expect("hotkey input missing"),
        );
        let settings_config = self.settings_snapshot();
        let status_text = self.status_text();
        let dark = crate::app::theme_mode::is_dark();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::semantic(dark).bg_page)
            .text_color(theme::semantic(dark).text_primary)
            .font_family("PingFang SC")
            .track_focus(
                self.focus_handle
                    .as_ref()
                    .expect("panel focus handle missing"),
            )
            .capture_key_down(cx.listener(Self::handle_panel_key))
            .child(if tab == ClipboardTab::History {
                history_page(
                    handle.clone(),
                    items,
                    selected,
                    &query,
                    query_input,
                    selected_record,
                    item_count,
                    current_filter,
                    status_text,
                    self.history_scroll.clone(),
                    preview_input,
                    dark,
                )
                .into_any_element()
            } else {
                settings_page(
                    handle.clone(),
                    status_text,
                    settings_config,
                    settings_inputs,
                    dark,
                )
                .into_any_element()
            })
    }
}

impl Focusable for ClipboardPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle
            .clone()
            .expect("clipboard panel focus handle missing")
    }
}

fn preview_text_for_record_for_panel(item: &ClipboardRecord) -> String {
    match item.kind {
        history_store::ClipboardItemKind::Files => {
            let paths = item.parsed_file_paths();
            if paths.is_empty() {
                item.preview.clone()
            } else {
                paths.join("\n")
            }
        }
        _ => item.content.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{database::DatabaseService, storage::AppPaths};

    #[test]
    fn status_text_reflects_filter_and_query_state() {
        let path = std::env::temp_dir().join("clipboard-status-test.db");
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        let mut panel =
            ClipboardPanel::new(Arc::new(Mutex::new(ClipboardService::new(database, path))));
        panel.filter = ClipboardFilter::Code;
        assert_eq!(panel.status_text(), "代码 · 暂无剪贴板记录");

        panel.filter = ClipboardFilter::Pinned;
        panel.query.clear();
        assert_eq!(panel.status_text(), "置顶 · 暂无剪贴板记录");

        panel.filter = ClipboardFilter::Code;
        panel.query = String::from("json");
        assert_eq!(panel.status_text(), "代码 · 没有匹配记录");

        panel.items = Arc::new(vec![ClipboardRecord {
            id: 1,
            kind: history_store::ClipboardItemKind::Text,
            content: String::from("{\"ok\":true}"),
            preview: String::from("{\"ok\":true}"),
            pinned: false,
            created_at: String::from("05-26 10:00:00"),
            badge: String::from("JSON"),
        }]);
        assert_eq!(panel.status_text(), "代码 · 关键词“json”匹配到 1 条记录");

        panel.query.clear();
        assert_eq!(panel.status_text(), "代码 · 已加载 1 条记录");
    }

    #[test]
    fn tab_switching_and_settings_snapshot_work() {
        let path = std::env::temp_dir().join("clipboard-settings-test.db");
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        let service = Arc::new(Mutex::new(ClipboardService::new(database, path)));
        let mut panel = ClipboardPanel::new(Arc::clone(&service));

        assert_eq!(panel.tab, ClipboardTab::History);
        panel.set_tab(ClipboardTab::Settings);
        assert_eq!(panel.tab, ClipboardTab::Settings);

        let config = panel.settings_snapshot();
        assert!(config.capture_text);
        assert!(config.capture_image);
        assert!(config.capture_files);
        assert_eq!(config.max_text_chars, 20_000);
    }
}
