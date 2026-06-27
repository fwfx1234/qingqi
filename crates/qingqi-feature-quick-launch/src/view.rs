use std::{collections::HashMap, sync::Arc};

use gpui::{
    App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, Point, Render, ScrollStrategy, StatefulInteractiveElement, Styled,
    Subscription, Task, UniformListScrollHandle, Window, div, hsla, point, px, uniform_list,
};

use crate::{
    model::{
        ActionKind, FeedbackMode, QuickAction, QuickActionDraft, QuickRun, RunStatus, ScriptSource,
        ScriptType,
    },
    parameters::{join_shell_words, split_shell_words},
    service::{QuickLaunchService, RunSummary},
};
use gpui_component::theme::Theme;
use gpui_component::{
    Selectable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use qingqi_ui::{
    theme,
    ui::{self, components},
};

/// Pre-computed theme colors used for uniform_list rows (must be Copy/'static for lifetime compatibility)
#[derive(Clone, Copy)]
struct RowTheme {
    hover_bg: gpui::Hsla,
    bg_surface: gpui::Hsla,
    border: gpui::Hsla,
    bg_subtle: gpui::Hsla,
    text_secondary: gpui::Hsla,
    accent: gpui::Rgba,
    success: gpui::Rgba,
    warning: gpui::Rgba,
    danger: gpui::Rgba,
    primary: gpui::Hsla,
    background: gpui::Hsla,
    is_dark: bool,
}

impl RowTheme {
    fn from_app(cx: &App) -> Self {
        let theme = Theme::global(cx);
        Self {
            hover_bg: ui::row_hover(cx),
            bg_surface: ui::bg_surface(cx),
            border: ui::border_light(cx),
            bg_subtle: ui::bg_subtle(cx),
            text_secondary: ui::text_secondary(cx),
            accent: ui::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Blue),
            success: gpui::Rgba::from(ui::success(cx)),
            warning: gpui::Rgba::from(ui::warning(cx)),
            danger: gpui::Rgba::from(ui::danger(cx)),
            primary: theme.primary,
            background: theme.background,
            is_dark: theme.is_dark(),
        }
    }
}

const HISTORY_LIMIT: usize = 8;

#[derive(Clone)]
struct PendingParameterField {
    name: String,
    input: Option<Entity<InputState>>,
}

#[derive(Clone)]
struct PendingExecution {
    action_id: i64,
    action_name: String,
    fields: Vec<PendingParameterField>,
}

#[derive(Clone)]
struct HistorySheetState {
    action_id: i64,
    action_name: String,
    runs: Vec<QuickRun>,
}

#[derive(Clone)]
struct ResultSheetState {
    action_name: String,
    run: QuickRun,
}

#[derive(Clone)]
struct ActionMenuState {
    action: QuickAction,
    position: Point<Pixels>,
}

#[derive(Clone)]
struct DeleteConfirmState {
    action_id: i64,
    action_name: String,
}

#[derive(Clone, Copy)]
enum ActionEditorMode {
    Create,
    Edit(i64),
}

#[derive(Clone)]
struct ActionEditorState {
    mode: ActionEditorMode,
    name_input: Entity<InputState>,
    description_input: Entity<InputState>,
    target_input: Entity<InputState>,
    args_input: Entity<InputState>,
    cwd_input: Entity<InputState>,
    interpreter_input: Entity<InputState>,
    env_input: Entity<InputState>,
    keywords_input: Entity<InputState>,
    prefixes_input: Entity<InputState>,
    icon_input: Entity<InputState>,
    timeout_input: Entity<InputState>,
    kind: ActionKind,
    script_type: ScriptType,
    script_source: ScriptSource,
    feedback_mode: FeedbackMode,
    enabled: bool,
}

#[derive(Clone, Copy)]
enum FocusTarget {
    Query,
    Pending(usize),
    EditorName,
}

pub struct QuickLaunchView {
    service: Arc<QuickLaunchService>,
    query_input: Option<Entity<InputState>>,
    actions: Vec<QuickAction>,
    running_action_ids: Vec<i64>,
    latest_run_summaries: HashMap<i64, RunSummary>,
    query: String,
    selected: usize,
    notice: Option<String>,
    pending: Option<PendingExecution>,
    history: Option<HistorySheetState>,
    result: Option<ResultSheetState>,
    action_menu: Option<ActionMenuState>,
    delete_confirm: Option<DeleteConfirmState>,
    editor: Option<ActionEditorState>,
    focus_target: Option<FocusTarget>,
    last_runtime_revision: u64,
    loading: bool,
    reload_generation: u64,
    pending_selected_action_id: Option<i64>,
    reload_task: Option<Task<()>>,
    history_task: Option<Task<()>>,
    action_task: Option<Task<()>>,
    list_scroll: UniformListScrollHandle,
    subscriptions: Vec<Subscription>,
}

impl QuickLaunchView {
    pub fn new(service: Arc<QuickLaunchService>, cx: &mut Context<Self>) -> Self {
        let snapshot = service.runtime_snapshot();
        let mut this = Self {
            service,
            query_input: None,
            actions: Vec::new(),
            running_action_ids: snapshot.running_action_ids,
            latest_run_summaries: HashMap::new(),
            query: String::new(),
            selected: 0,
            notice: None,
            pending: None,
            history: None,
            result: None,
            action_menu: None,
            delete_confirm: None,
            editor: None,
            focus_target: Some(FocusTarget::Query),
            last_runtime_revision: snapshot.revision,
            loading: false,
            reload_generation: 0,
            pending_selected_action_id: None,
            reload_task: None,
            history_task: None,
            action_task: None,
            list_scroll: UniformListScrollHandle::new(),
            subscriptions: Vec::new(),
        };
        this.reload_actions(cx);
        this
    }

    fn observe_query_input(&mut self, cx: &mut Context<Self>) {
        if !self.subscriptions.is_empty() {
            return;
        }
        let Some(query_input) = self.query_input.clone() else {
            return;
        };
        let subscription = cx.observe(&query_input, |view, _, cx| {
            view.sync_query(cx);
        });
        self.subscriptions.push(subscription);
    }

    fn sync_query(&mut self, cx: &mut Context<Self>) {
        let Some(query_input) = self.query_input.as_ref() else {
            return;
        };
        self.query = query_input.read(cx).value().to_string();
        self.selected = 0;
        self.notice = None;
        self.pending_selected_action_id = None;
        self.reload_actions(cx);
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.query_input.is_none() {
            let query = self.query.clone();
            let input = cx.new(|cx| {
                let mut input = InputState::new(window, cx);
                input.set_placeholder("搜索动作", window, cx);
                input.reset_value(query, cx);
                input
            });
            self.query_input = Some(input);
        }

        if let Some(pending) = self.pending.as_mut() {
            for field in &mut pending.fields {
                if field.input.is_none() {
                    let placeholder = format!("输入 {}", field.name);
                    field.input = Some(sheet_input(window, cx, placeholder, ""));
                }
            }
        }
    }

    fn reload_actions(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.reload_generation = self.reload_generation.wrapping_add(1);
        let generation = self.reload_generation;
        let query = self.query.clone();
        let service = Arc::clone(&self.service);

        self.reload_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let actions = service.list_actions(&query, None)?;
                    let ids: Vec<i64> = actions.iter().map(|action| action.id).collect();
                    let latest_run_summaries =
                        service.latest_run_summaries(&ids).unwrap_or_default();
                    anyhow::Ok((actions, latest_run_summaries))
                })
                .await;

            let _ = view.update(async_cx, |view, cx| {
                if view.reload_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok((actions, latest_run_summaries)) => {
                        view.actions = actions;
                        view.latest_run_summaries = latest_run_summaries;
                        if let Some(action_id) = view.pending_selected_action_id.take() {
                            view.select_action_id(action_id);
                        } else {
                            view.selected = view.selected.min(view.actions.len().saturating_sub(1));
                        }
                    }
                    Err(error) => {
                        view.actions.clear();
                        view.latest_run_summaries.clear();
                        view.selected = 0;
                        view.notice = Some(format!("读取动作失败: {error}"));
                    }
                }
                cx.notify();
            });
        }));
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.actions.is_empty() {
            self.selected = 0;
            cx.notify();
            return;
        }

        let len = self.actions.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        self.list_scroll
            .scroll_to_item(self.selected, ScrollStrategy::Top);
        self.notice = None;
        cx.notify();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index.min(self.actions.len().saturating_sub(1));
        self.notice = None;
        cx.notify();
    }

    fn selected_action(&self) -> Option<QuickAction> {
        self.actions.get(self.selected).cloned()
    }

    fn run_selected(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("没有可执行的动作"));
            cx.notify();
            return;
        };
        self.run_action(action, cx);
    }

    fn run_action(&mut self, action: QuickAction, cx: &mut Context<Self>) {
        self.notice = Some(format!("正在准备执行 {}", action.name));
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let action_for_task = action.clone();
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    let specs = service.required_parameters(action_for_task.id)?;
                    if !specs.is_empty() {
                        anyhow::Ok((Some(specs), None))
                    } else {
                        let message = service.start_action(action_for_task.id)?;
                        anyhow::Ok((None, Some(message)))
                    }
                })
                .await;

            let _ = view.update(async_cx, |view, cx| match result {
                Ok((Some(specs), _)) => view.open_pending(action.clone(), specs, cx),
                Ok((None, Some(message))) => {
                    view.notice = Some(message);
                    cx.notify();
                }
                Ok(_) => {}
                Err(error) => {
                    view.notice = Some(format!("执行失败: {error}"));
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn rerun_action_by_id(&mut self, action_id: i64, cx: &mut Context<Self>) {
        if let Some(action) = self.actions.iter().find(|a| a.id == action_id).cloned() {
            self.run_action(action, cx);
        } else {
            self.notice = Some(format!("动作 {action_id} 不存在或已被删除"));
            cx.notify();
        }
    }

    fn stop_selected(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("没有可停止的动作"));
            cx.notify();
            return;
        };
        self.stop_action(action.id, cx);
    }

    fn stop_action(&mut self, action_id: i64, cx: &mut Context<Self>) {
        self.notice = Some(String::from("正在请求停止..."));
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.stop_action(action_id) })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                view.notice = Some(result.unwrap_or_else(|error| format!("停止失败: {error}")));
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn open_selected_history(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("当前没有可查看历史的动作"));
            cx.notify();
            return;
        };
        self.open_history(action, cx);
    }

    fn open_history(&mut self, action: QuickAction, cx: &mut Context<Self>) {
        self.notice = Some(format!("正在加载 {} 的最近记录...", action.name));
        let service = Arc::clone(&self.service);
        self.history_task = Some(cx.spawn(async move |view, async_cx| {
            let action_for_task = action.clone();
            let result = async_cx
                .background_executor()
                .spawn(async move { service.list_runs(action_for_task.id, HISTORY_LIMIT) })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                match result {
                    Ok(runs) => {
                        view.history = Some(HistorySheetState {
                            action_id: action.id,
                            action_name: action.name.clone(),
                            runs,
                        });
                        view.notice = Some(format!("已加载 {} 的最近记录", action.name));
                    }
                    Err(error) => {
                        view.history = None;
                        view.notice = Some(format!("读取历史失败: {error}"));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn open_create_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor = Some(build_editor_state(None, window, cx));
        self.pending = None;
        self.history = None;
        self.result = None;
        self.action_menu = None;
        self.delete_confirm = None;
        self.notice = Some(String::from("新建动作"));
        self.focus_target = Some(FocusTarget::EditorName);
        cx.notify();
    }

    fn open_selected_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("当前没有可编辑的动作"));
            cx.notify();
            return;
        };
        self.open_edit_editor(action, window, cx);
    }

    fn open_edit_editor(
        &mut self,
        action: QuickAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor = Some(build_editor_state(Some(action.clone()), window, cx));
        self.pending = None;
        self.history = None;
        self.result = None;
        self.action_menu = None;
        self.delete_confirm = None;
        self.notice = Some(format!("编辑 {}", action.name));
        self.focus_target = Some(FocusTarget::EditorName);
        cx.notify();
    }

    fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        if self.pending.is_none() && self.history.is_none() && self.result.is_none() {
            self.focus_target = Some(FocusTarget::Query);
        }
        cx.notify();
    }

    fn open_action_menu(
        &mut self,
        action: QuickAction,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.select_action_id(action.id);
        self.action_menu = Some(ActionMenuState { action, position });
        self.delete_confirm = None;
        self.notice = None;
        cx.notify();
    }

    fn close_action_menu(&mut self, cx: &mut Context<Self>) {
        self.action_menu = None;
        cx.notify();
    }

    fn open_delete_confirm(&mut self, action_id: i64, action_name: String, cx: &mut Context<Self>) {
        self.action_menu = None;
        self.delete_confirm = Some(DeleteConfirmState {
            action_id,
            action_name,
        });
        cx.notify();
    }

    fn close_delete_confirm(&mut self, cx: &mut Context<Self>) {
        self.delete_confirm = None;
        cx.notify();
    }

    fn set_editor_kind(&mut self, kind: ActionKind, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.as_mut() {
            editor.kind = kind;
        }
        cx.notify();
    }

    fn set_editor_script_type(&mut self, script_type: ScriptType, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.as_mut() {
            editor.script_type = script_type;
        }
        cx.notify();
    }

    fn set_editor_script_source(&mut self, script_source: ScriptSource, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.as_mut() {
            editor.script_source = script_source;
        }
        cx.notify();
    }

    fn set_editor_feedback_mode(&mut self, mode: FeedbackMode, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.as_mut() {
            editor.feedback_mode = mode;
        }
        cx.notify();
    }

    fn toggle_editor_enabled(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.as_mut() {
            editor.enabled = !editor.enabled;
        }
        cx.notify();
    }

    fn pick_editor_target(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let prompt = match editor.kind {
            ActionKind::Script => match editor.script_source {
                ScriptSource::Path => "选择脚本文件",
                ScriptSource::Inline => return,
            },
            ActionKind::OpenPath => "选择目标路径",
            ActionKind::OpenUrl => return,
        };

        match qingqi_platform::shell::choose_file(prompt) {
            Ok(Some(path)) => {
                if let Some(editor) = self.editor.as_mut() {
                    editor.target_input.update(cx, |input, input_cx| {
                        input.reset_value(path.display().to_string(), input_cx);
                    });
                }
                self.notice = Some(format!("已选择 {}", path.display()));
            }
            Ok(None) => {
                self.notice = Some(String::from("已取消选择"));
            }
            Err(error) => {
                self.notice = Some(format!("选择失败: {error}"));
            }
        }
        cx.notify();
    }

    fn pick_editor_cwd(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };

        match qingqi_platform::shell::choose_directory("选择工作目录") {
            Ok(Some(path)) => {
                editor.cwd_input.update(cx, |input, input_cx| {
                    input.reset_value(path.display().to_string(), input_cx);
                });
                self.notice = Some(format!("已选择目录 {}", path.display()));
            }
            Ok(None) => {
                self.notice = Some(String::from("已取消选择"));
            }
            Err(error) => {
                self.notice = Some(format!("选择目录失败: {error}"));
            }
        }
        cx.notify();
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let draft = match self.collect_editor_draft(cx) {
            Ok(draft) => draft,
            Err(error) => {
                self.notice = Some(format!("保存失败: {error}"));
                cx.notify();
                return;
            }
        };

        self.notice = Some(String::from("正在保存动作..."));
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let mode = editor.mode;
            let draft_for_task = draft.clone();
            let result = async_cx
                .background_executor()
                .spawn(async move {
                    match mode {
                        ActionEditorMode::Create => service.create_action(draft_for_task),
                        ActionEditorMode::Edit(action_id) => {
                            service.update_action(action_id, draft_for_task)
                        }
                    }
                })
                .await;
            let _ = view.update(async_cx, |view, cx| match result {
                Ok(action) => {
                    view.editor = None;
                    view.notice = Some(match mode {
                        ActionEditorMode::Create => format!("已创建动作 {}", action.name),
                        ActionEditorMode::Edit(_) => format!("已保存动作 {}", action.name),
                    });
                    view.focus_target = Some(FocusTarget::Query);
                    view.pending_selected_action_id = Some(action.id);
                    view.reload_actions(cx);
                }
                Err(error) => {
                    view.notice = Some(format!("保存失败: {error}"));
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn duplicate_selected(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("当前没有可复制的动作"));
            cx.notify();
            return;
        };
        self.duplicate_action(action, cx);
    }

    fn duplicate_action(&mut self, action: QuickAction, cx: &mut Context<Self>) {
        self.notice = Some(format!("正在复制 {}...", action.name));
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.duplicate_action(action.id) })
                .await;
            let _ = view.update(async_cx, |view, cx| match result {
                Ok(created) => {
                    view.action_menu = None;
                    view.notice = Some(format!("已复制为 {}", created.name));
                    view.pending_selected_action_id = Some(created.id);
                    view.reload_actions(cx);
                }
                Err(error) => {
                    view.notice = Some(format!("复制失败: {error}"));
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn toggle_selected_enabled(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("当前没有可切换的动作"));
            cx.notify();
            return;
        };
        let enabled = !action.enabled;
        self.set_action_enabled(action, enabled, cx);
    }

    fn set_action_enabled(&mut self, action: QuickAction, enabled: bool, cx: &mut Context<Self>) {
        self.notice = Some(format!("正在更新 {}...", action.name));
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.set_action_enabled(action.id, enabled) })
                .await;
            let _ = view.update(async_cx, |view, cx| match result {
                Ok(updated) => {
                    view.action_menu = None;
                    view.notice = Some(if updated.enabled {
                        format!("已启用 {}", updated.name)
                    } else {
                        format!("已停用 {}", updated.name)
                    });
                    view.pending_selected_action_id = Some(updated.id);
                    view.reload_actions(cx);
                }
                Err(error) => {
                    view.notice = Some(format!("切换失败: {error}"));
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("当前没有可删除的动作"));
            cx.notify();
            return;
        };
        self.request_delete_action(action, cx);
    }

    fn request_delete_action(&mut self, action: QuickAction, cx: &mut Context<Self>) {
        self.select_action_id(action.id);
        self.open_delete_confirm(action.id, action.name.clone(), cx);
    }

    fn open_action_history(&mut self, action: QuickAction, cx: &mut Context<Self>) {
        self.action_menu = None;
        self.open_history(action, cx);
    }

    fn confirm_delete_action(&mut self, action_id: i64, cx: &mut Context<Self>) {
        let deleted_index = self
            .actions
            .iter()
            .position(|action| action.id == action_id);

        self.notice = Some(String::from("正在删除动作..."));
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.delete_action(action_id) })
                .await;
            let _ = view.update(async_cx, |view, cx| match result {
                Ok(message) => {
                    view.delete_confirm = None;
                    if view
                        .history
                        .as_ref()
                        .map(|history| history.action_id == action_id)
                        .unwrap_or(false)
                    {
                        view.history = None;
                    }
                    if let Some(index) = deleted_index {
                        view.selected = index.min(view.actions.len().saturating_sub(1));
                    }
                    view.pending_selected_action_id = None;
                    view.notice = Some(message);
                    view.reload_actions(cx);
                }
                Err(error) => {
                    view.notice = Some(format!("删除失败: {error}"));
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn open_selected_result(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.selected_action() else {
            self.notice = Some(String::from("当前没有可查看的结果"));
            cx.notify();
            return;
        };
        self.open_latest_result(action.id, action.name.clone(), cx);
    }

    fn open_latest_result(&mut self, action_id: i64, action_name: String, cx: &mut Context<Self>) {
        self.notice = Some(format!("正在读取 {action_name} 的最新结果..."));
        let service = Arc::clone(&self.service);
        self.history_task = Some(cx.spawn(async move |view, async_cx| {
            let action_name_for_task = action_name.clone();
            let result = async_cx
                .background_executor()
                .spawn(async move { service.list_runs(action_id, 1) })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                match result {
                    Ok(runs) => {
                        if let Some(run) = runs.into_iter().next() {
                            view.open_result(action_name.clone(), run, cx);
                        } else {
                            view.notice = Some(format!("{action_name} 还没有运行记录"));
                            cx.notify();
                        }
                    }
                    Err(error) => {
                        view.notice = Some(format!("读取最新结果失败: {error}"));
                        cx.notify();
                    }
                }
                let _ = action_name_for_task;
            });
        }));
    }

    fn set_result(&mut self, action_name: String, run: QuickRun) {
        self.result = Some(ResultSheetState { action_name, run });
    }

    fn open_result(&mut self, action_name: String, run: QuickRun, cx: &mut Context<Self>) {
        self.set_result(action_name, run);
        self.notice = Some(String::from("已打开结果详情"));
        cx.notify();
    }

    fn refresh_history_panel(&mut self, cx: &mut Context<Self>) {
        let Some(history) = self.history.clone() else {
            return;
        };
        self.notice = Some(String::from("正在刷新运行历史..."));
        let service = Arc::clone(&self.service);
        self.history_task = Some(cx.spawn(async move |view, async_cx| {
            let history_for_task = history.clone();
            let result = async_cx
                .background_executor()
                .spawn(async move { service.list_runs(history_for_task.action_id, HISTORY_LIMIT) })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                match result {
                    Ok(runs) => {
                        view.history = Some(HistorySheetState { runs, ..history });
                        view.notice = Some(String::from("运行历史已刷新"));
                    }
                    Err(error) => {
                        view.notice = Some(format!("刷新历史失败: {error}"));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn refresh_history_for(&mut self, action_id: i64) {
        let Some(history) = self.history.as_mut() else {
            return;
        };
        if history.action_id != action_id {
            return;
        }

        if let Ok(runs) = self.service.list_runs(action_id, HISTORY_LIMIT) {
            history.runs = runs;
        }
    }

    fn sync_runtime_state(&mut self) {
        let snapshot = self.service.runtime_snapshot();
        if snapshot.revision == self.last_runtime_revision {
            return;
        }

        if let Some(event) = snapshot.last_event.as_ref()
            && event.revision > self.last_runtime_revision
        {
            self.notice = Some(event.message.clone());
            self.refresh_history_for(event.action_id);
            if let Some(run) = event.run.clone()
                && (event.feedback_mode == FeedbackMode::Popup || run.status != RunStatus::Success)
            {
                self.set_result(event.action_name.clone(), run);
            }
            // Refresh the latest-run status for the affected action
            if let Some(run) = event.run.as_ref() {
                self.latest_run_summaries
                    .insert(run.action_id, RunSummary::from_run(run));
            }
        }

        self.running_action_ids = snapshot.running_action_ids;
        self.last_runtime_revision = snapshot.revision;
    }

    fn close_history(&mut self, cx: &mut Context<Self>) {
        self.history = None;
        if self.result.is_none() {
            self.focus_target = Some(FocusTarget::Query);
        }
        cx.notify();
    }

    fn close_result(&mut self, cx: &mut Context<Self>) {
        self.result = None;
        if self.pending.is_none() && self.history.is_none() {
            self.focus_target = Some(FocusTarget::Query);
        }
        cx.notify();
    }

    fn copy_result_stdout(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.result.as_ref() else {
            return;
        };
        if result.run.stdout.trim().is_empty() {
            self.notice = Some(String::from("stdout 为空"));
        } else {
            qingqi_platform::clipboard::write_text(cx, result.run.stdout.clone());
            self.notice = Some(String::from("已复制 stdout"));
        }
        cx.notify();
    }

    fn copy_result_stderr(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.result.as_ref() else {
            return;
        };
        if result.run.stderr.trim().is_empty() {
            self.notice = Some(String::from("stderr 为空"));
        } else {
            qingqi_platform::clipboard::write_text(cx, result.run.stderr.clone());
            self.notice = Some(String::from("已复制 stderr"));
        }
        cx.notify();
    }

    fn open_pending(
        &mut self,
        action: QuickAction,
        specs: Vec<crate::parameters::ParameterSpec>,
        cx: &mut Context<Self>,
    ) {
        let fields = specs
            .into_iter()
            .map(|spec| PendingParameterField {
                name: spec.name.clone(),
                input: None,
            })
            .collect::<Vec<_>>();
        let count = fields.len();

        self.pending = Some(PendingExecution {
            action_id: action.id,
            action_name: action.name.clone(),
            fields,
        });
        self.notice = Some(format!("{} 需要 {} 个参数", action.name, count));
        self.focus_target = Some(FocusTarget::Pending(0));
        cx.notify();
    }

    fn submit_pending(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending.clone() else {
            return;
        };

        let mut values = HashMap::new();
        for field in &pending.fields {
            values.insert(
                field.name.clone(),
                field
                    .input
                    .as_ref()
                    .map(|input| input.read(cx).value().to_string())
                    .unwrap_or_default(),
            );
        }

        match self
            .service
            .start_action_with_values(pending.action_id, values)
        {
            Ok(message) => {
                self.notice = Some(message);
                self.pending = None;
                self.focus_target = Some(FocusTarget::Query);
            }
            Err(error) => {
                self.notice = Some(format!("执行失败: {error}"));
            }
        }
        cx.notify();
    }

    fn close_pending(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.focus_target = Some(FocusTarget::Query);
        cx.notify();
    }

    fn clear_query(&mut self, cx: &mut Context<Self>) {
        if let Some(query_input) = self.query_input.as_ref() {
            query_input.update(cx, |input, input_cx| input.reset_value("", input_cx));
        }
        self.query.clear();
        self.selected = 0;
        self.notice = None;
        self.pending_selected_action_id = None;
        self.reload_actions(cx);
        self.focus_target = Some(FocusTarget::Query);
        cx.notify();
    }

    fn collect_editor_draft(&self, cx: &App) -> anyhow::Result<QuickActionDraft> {
        let editor = self
            .editor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("编辑器尚未打开"))?;
        let name = editor.name_input.read(cx).value().trim().to_string();
        let description = editor.description_input.read(cx).value().trim().to_string();
        let target = editor.target_input.read(cx).value().trim().to_string();
        let args = split_shell_words(editor.args_input.read(cx).value().as_ref())
            .map_err(|error| anyhow::anyhow!("运行参数格式错误: {error}"))?;
        let cwd = editor.cwd_input.read(cx).value().trim().to_string();
        let interpreter = editor.interpreter_input.read(cx).value().trim().to_string();
        let env = parse_env_lines(editor.env_input.read(cx).value().as_ref());
        let keywords = split_csv(editor.keywords_input.read(cx).value().as_ref());
        let prefixes = split_csv(editor.prefixes_input.read(cx).value().as_ref());
        let icon = editor.icon_input.read(cx).value().trim().to_string();
        let timeout_text = editor.timeout_input.read(cx).value().to_string();
        let timeout_sec = if timeout_text.trim().is_empty() {
            300
        } else {
            timeout_text
                .trim()
                .parse::<i64>()
                .map_err(|_| anyhow::anyhow!("超时必须是整数"))?
        };

        let mut draft = QuickActionDraft {
            name,
            description,
            kind: editor.kind,
            script_type: editor.script_type,
            script_source: editor.script_source,
            script_body: String::new(),
            interpreter,
            path: String::new(),
            url: String::new(),
            args,
            cwd,
            env,
            keywords,
            prefixes,
            icon,
            feedback_mode: editor.feedback_mode,
            timeout_sec,
            enabled: editor.enabled,
            sort_order: None,
        };

        match editor.kind {
            ActionKind::Script => match editor.script_source {
                ScriptSource::Inline => draft.script_body = target,
                ScriptSource::Path => draft.path = target,
            },
            ActionKind::OpenPath => draft.path = target,
            ActionKind::OpenUrl => draft.url = target,
        }

        Ok(draft)
    }

    fn select_action_id(&mut self, action_id: i64) {
        if let Some(index) = self
            .actions
            .iter()
            .position(|action| action.id == action_id)
        {
            self.selected = index;
        } else {
            self.selected = self.selected.min(self.actions.len().saturating_sub(1));
        }
    }

    fn status_text(&self) -> String {
        if let Some(notice) = self.notice.as_ref() {
            return notice.clone();
        }
        if self.loading {
            return String::from("正在加载动作...");
        }

        if self.actions.is_empty() {
            if self.query.trim().is_empty() {
                return String::from("还没有动作，当前先展示 seed data 动作仓库");
            }
            return String::from("没有匹配的动作");
        }

        if self.query.trim().is_empty() {
            format!("共 {} 个动作", self.actions.len())
        } else {
            format!("匹配到 {} 个动作", self.actions.len())
        }
    }
}

impl Render for QuickLaunchView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(window, cx);
        self.observe_query_input(cx);
        self.sync_runtime_state();

        if let Some(target) = self.focus_target.take() {
            match target {
                FocusTarget::Query => {
                    if let Some(query_input) = self.query_input.as_ref() {
                        window.focus(&query_input.focus_handle(cx));
                    }
                }
                FocusTarget::Pending(index) => {
                    if let Some(pending) = self.pending.as_ref()
                        && let Some(field) = pending.fields.get(index)
                        && let Some(input) = field.input.as_ref()
                    {
                        window.focus(&input.focus_handle(cx));
                    }
                }
                FocusTarget::EditorName => {
                    if let Some(editor) = self.editor.as_ref() {
                        window.focus(&editor.name_input.focus_handle(cx));
                    }
                }
            }
        }

        let handle = cx.entity();
        let actions = self.actions.clone();
        let running_action_ids = self.running_action_ids.clone();
        let selected = self.selected.min(actions.len().saturating_sub(1));
        let selected_running = actions
            .get(selected)
            .map(|action| running_action_ids.contains(&action.id))
            .unwrap_or(false);
        let query_input = self.query_input.clone().expect("query input missing");
        let message = self.status_text();
        let has_query = !self.query.trim().is_empty();
        let pending = self.pending.clone();
        let history = self.history.clone();
        let result = self.result.clone();
        let action_menu = self.action_menu.clone();
        let delete_confirm = self.delete_confirm.clone();
        let editor = self.editor.clone();
        let selected_action = actions.get(selected).cloned();

        div()
            .size_full()
            .bg(Theme::global(cx).background)
            .text_color(ui::text_primary(cx))
            .font_family(ui::font_ui())
            .p_2p5()
            .flex()
            .flex_col()
            .gap_1p5()
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        if view.pending.is_some() {
                            view.close_pending(cx);
                        } else if view.editor.is_some() {
                            view.close_editor(cx);
                        } else if view.delete_confirm.is_some() {
                            view.close_delete_confirm(cx);
                        } else if view.action_menu.is_some() {
                            view.close_action_menu(cx);
                        } else if view.result.is_some() {
                            view.close_result(cx);
                        } else if view.history.is_some() {
                            view.close_history(cx);
                        }
                    }
                    "enter" if view.pending.is_some() => view.submit_pending(cx),
                    "up" if view.pending.is_none()
                        && view.editor.is_none()
                        && view.history.is_none()
                        && view.result.is_none() =>
                    {
                        view.move_selection(-1, cx)
                    }
                    "down"
                        if view.pending.is_none()
                            && view.editor.is_none()
                            && view.history.is_none()
                            && view.result.is_none() =>
                    {
                        view.move_selection(1, cx)
                    }
                    "enter" if view.editor.is_some() => view.save_editor(cx),
                    "enter"
                        if view.editor.is_none()
                            && view.history.is_none()
                            && view.result.is_none() =>
                    {
                        view.run_selected(cx)
                    }
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
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("快速启动"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(ui::text_secondary(cx))
                            .child(message),
                    ),
            )
            .child(search_row(
                handle.clone(),
                query_input,
                selected_running,
                cx,
            ))
            .child(
                selected_action
                    .map(|action| management_row(handle.clone(), action, cx).into_any_element())
                    .unwrap_or_else(|| div().into_any_element()),
            )
            .child({
                let list_container = div()
                    .id("quick-launch-list")
                    .flex_1()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(ui::border_light(cx))
                    .bg(ui::bg_surface(cx));
                if actions.is_empty() {
                    list_container
                        .overflow_y_scroll()
                        .child(empty_state(has_query, cx))
                        .into_any_element()
                } else {
                    let scroll = self.list_scroll.clone();
                    let handle = handle.clone();
                    let running_ids = running_action_ids.clone();
                    let latest_summaries = self.latest_run_summaries.clone();
                    let total = actions.len();
                    let actions_for_list = actions.clone();
                    list_container
                        .child(
                            uniform_list("quick-launch-rows", total, move |range, _window, cx| {
                                let theme = RowTheme::from_app(cx);
                                range
                                    .map(|index| {
                                        let action = actions_for_list[index].clone();
                                        let running = running_ids.contains(&action.id);
                                        action_row(
                                            handle.clone(),
                                            action,
                                            index,
                                            index == selected,
                                            running,
                                            latest_summaries.clone(),
                                            theme,
                                        )
                                    })
                                    .collect()
                            })
                            .track_scroll(scroll)
                            .size_full(),
                        )
                        .into_any_element()
                }
            })
            .child(if let Some(action_menu) = action_menu {
                let running = running_action_ids.contains(&action_menu.action.id);
                menu_overlay_shell(handle.clone(), action_menu, running, cx).into_any_element()
            } else if let Some(delete_confirm) = delete_confirm {
                overlay_shell(
                    cx,
                    "quick-launch-delete-overlay",
                    {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| {
                                view.close_delete_confirm(cx);
                            });
                        }
                    },
                    delete_confirm_sheet(handle.clone(), delete_confirm, cx),
                )
                .into_any_element()
            } else if let Some(pending) = pending {
                overlay_shell(
                    cx,
                    "quick-launch-pending-overlay",
                    {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| view.close_pending(cx));
                        }
                    },
                    pending_sheet(handle.clone(), pending, cx),
                )
                .into_any_element()
            } else if let Some(editor) = editor {
                overlay_shell(
                    cx,
                    "quick-launch-editor-overlay",
                    {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| view.close_editor(cx));
                        }
                    },
                    action_editor_sheet(handle.clone(), editor, cx),
                )
                .into_any_element()
            } else if let Some(result) = result {
                overlay_shell(
                    cx,
                    "quick-launch-result-overlay",
                    {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| view.close_result(cx));
                        }
                    },
                    result_sheet(handle.clone(), result, cx),
                )
                .into_any_element()
            } else if let Some(history) = history {
                overlay_shell(
                    cx,
                    "quick-launch-history-overlay",
                    {
                        let handle = handle.clone();
                        move |_, cx| {
                            let _ = cx.update_entity(&handle, |view, cx| view.close_history(cx));
                        }
                    },
                    history_sheet(handle, history, cx),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
    }
}

fn search_row(
    handle: Entity<QuickLaunchView>,
    query_input: Entity<InputState>,
    selected_running: bool,
    cx: &App,
) -> impl IntoElement {
    let sr_theme = RowTheme::from_app(cx);
    div()
        .h(px(30.0))
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .flex_1()
                .h(px(28.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(ui::border_light(cx))
                .bg(ui::bg_surface(cx))
                .child(
                    Input::new(&query_input)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .h(px(28.0))
                        .text_size(px(12.0)),
                ),
        )
        .child(if selected_running {
            action_button("停止选中项", sr_theme, {
                let handle = handle.clone();
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| view.stop_selected(cx));
                }
            })
            .into_any_element()
        } else {
            primary_action_button("运行选中项", sr_theme, {
                let handle = handle.clone();
                move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| view.run_selected(cx));
                }
            })
            .into_any_element()
        })
        .child(action_button("查看历史", sr_theme, {
            let handle = handle.clone();
            move |_, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.open_selected_history(cx));
            }
        }))
        .child(action_button("最新结果", sr_theme, {
            let handle = handle.clone();
            move |_, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.open_selected_result(cx));
            }
        }))
        .child(window_action_button("新建动作", sr_theme, {
            let handle = handle.clone();
            move |_, window, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.open_create_editor(window, cx));
            }
        }))
        .child(action_button("清空", sr_theme, move |_, cx| {
            let _ = cx.update_entity(&handle, |view, cx| view.clear_query(cx));
        }))
}

fn management_row(
    handle: Entity<QuickLaunchView>,
    action: QuickAction,
    cx: &App,
) -> impl IntoElement {
    let mgmt_theme = RowTheme::from_app(cx);
    let enabled_label = if action.enabled { "停用" } else { "启用" };

    div()
        .rounded(px(6.0))
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .px(px(8.0))
        .py_1()
        .flex()
        .items_center()
        .gap_1p5()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(format!("已选中: {}", action.name)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(kind_chip(action.kind.label().to_string(), mgmt_theme))
                        .child(if action.feedback_mode != FeedbackMode::Notification {
                            subtle_chip(
                                feedback_label(action.feedback_mode).to_string(),
                                mgmt_theme,
                            )
                            .into_any_element()
                        } else {
                            div().into_any_element()
                        })
                        .child(if action.enabled {
                            status_chip(
                                String::from("已启用"),
                                gpui::Rgba::from(ui::success(cx)),
                                mgmt_theme,
                            )
                            .into_any_element()
                        } else {
                            status_chip(
                                String::from("已停用"),
                                gpui::Rgba::from(ui::warning(cx)),
                                mgmt_theme,
                            )
                            .into_any_element()
                        })
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ui::text_secondary(cx))
                                .child(subtitle_for(&action)),
                        ),
                ),
        )
        .child(window_action_button("编辑", mgmt_theme, {
            let handle = handle.clone();
            move |_, window, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.open_selected_editor(window, cx));
            }
        }))
        .child(action_button("复制", mgmt_theme, {
            let handle = handle.clone();
            move |_, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.duplicate_selected(cx));
            }
        }))
        .child(action_button(enabled_label, mgmt_theme, {
            let handle = handle.clone();
            move |_, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.toggle_selected_enabled(cx));
            }
        }))
        .child(action_button("删除", mgmt_theme, move |_, cx| {
            let _ = cx.update_entity(&handle, |view, cx| view.delete_selected(cx));
        }))
}

fn empty_state(has_query: bool, cx: &App) -> impl IntoElement {
    let (title, subtitle) = if has_query {
        ("没有匹配的动作", "换个关键词，或者清空当前搜索")
    } else {
        (
            "动作仓库已就绪",
            "当前显示的是 SQLite 动作仓库，支持新建、编辑和运行",
        )
    };

    div()
        .w_full()
        .min_h(px(160.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_1()
        .child(
            div()
                .size(px(36.0))
                .rounded(px(8.0))
                .bg(ui::bg_subtle(cx))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .text_color(ui::accent_color(
                    qingqi_plugin::plugin_spec::PluginAccent::Blue,
                ))
                .child("Q"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_secondary(cx))
                .child(subtitle),
        )
}

fn action_row(
    handle: Entity<QuickLaunchView>,
    action: QuickAction,
    index: usize,
    selected: bool,
    running: bool,
    latest_run_summaries: HashMap<i64, RunSummary>,
    theme: RowTheme,
) -> impl IntoElement + 'static {
    let row_bg = if selected {
        theme.hover_bg
    } else {
        theme.bg_surface
    };
    let parameter_count = action.parameter_specs().len();
    let action_for_run = action.clone();
    let action_for_history = action.clone();
    let action_for_edit = action.clone();
    let action_for_menu = action.clone();
    let menu_handle = handle.clone();
    let run_handle = handle.clone();
    let row_hover_bg = theme.hover_bg;

    div()
        .id(("quick-launch-row", index))
        .h(px(48.0))
        .px(px(8.0))
        .border_b_1()
        .border_color(theme.border)
        .bg(row_bg)
        .hover(move |style| style.bg(row_hover_bg).cursor_pointer())
        .on_click({
            let handle = handle.clone();
            move |_, _, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.select(index, cx));
            }
        })
        .flex()
        .items_center()
        .gap_1p5()
        .child(
            div()
                .size(px(30.0))
                .rounded(px(6.0))
                .bg(theme.bg_subtle)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.0))
                .text_color(theme.accent)
                .child(icon_for_action(&action)),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(div().text_size(px(12.0)).child(action.name.clone()))
                        .child(kind_chip(action.kind.label().to_string(), theme))
                        .child(if parameter_count > 0 {
                            subtle_chip(format!("{parameter_count} 参数"), theme).into_any_element()
                        } else {
                            div().into_any_element()
                        })
                        .child(if action.feedback_mode != FeedbackMode::Notification {
                            subtle_chip(feedback_label(action.feedback_mode).to_string(), theme)
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        })
                        .child(if running {
                            status_chip(String::from("运行中"), theme.success, theme)
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        })
                        .child(if !running {
                            latest_run_status_chip(action.id, &latest_run_summaries, theme)
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        })
                        .child(if action.enabled {
                            div().into_any_element()
                        } else {
                            status_chip("已停用".to_string(), theme.warning, theme)
                                .into_any_element()
                        }),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(theme.text_secondary)
                        .child(subtitle_for(&action)),
                ),
        )
        .child(action_button("历史", theme, {
            let handle = handle.clone();
            move |_, cx| {
                let action = action_for_history.clone();
                let _ = cx.update_entity(&handle, |view, cx| view.open_history(action, cx));
            }
        }))
        .child(window_action_button("编辑", theme, {
            let handle = handle.clone();
            move |_, window, cx| {
                let action = action_for_edit.clone();
                let _ = cx.update_entity(&handle, |view, cx| {
                    view.open_edit_editor(action, window, cx);
                });
            }
        }))
        .child(icon_action_button(
            "⋯",
            theme,
            move |event, window, cx| {
                let action = action_for_menu.clone();
                let position = menu_position(event.position(), window);
                let _ = cx.update_entity(&menu_handle, |view, cx| {
                    view.open_action_menu(action, position, cx);
                });
            },
        ))
        .child(if running {
            action_button("停止", theme, move |_, cx| {
                let _ = cx.update_entity(&handle, |view, cx| view.stop_action(action.id, cx));
            })
            .into_any_element()
        } else {
            primary_action_button("运行", theme, move |_, cx| {
                let action = action_for_run.clone();
                let _ = cx.update_entity(&run_handle, |view, cx| view.run_action(action, cx));
            })
            .into_any_element()
        })
}

fn action_editor_sheet(
    handle: Entity<QuickLaunchView>,
    editor: ActionEditorState,
    cx: &App,
) -> impl IntoElement {
    let ed_theme = RowTheme::from_app(cx);
    let title = match editor.mode {
        ActionEditorMode::Create => "新建动作",
        ActionEditorMode::Edit(_) => "编辑动作",
    };
    let target_label = match editor.kind {
        ActionKind::Script => match editor.script_source {
            ScriptSource::Inline => "脚本内容",
            ScriptSource::Path => "脚本路径",
        },
        ActionKind::OpenPath => "目标路径",
        ActionKind::OpenUrl => "URL",
    };
    let target_hint = match editor.kind {
        ActionKind::Script => match editor.script_source {
            ScriptSource::Inline => "如：open -a Safari; echo done",
            ScriptSource::Path => "/path/to/script.sh",
        },
        ActionKind::OpenPath => "/Applications/Safari.app",
        ActionKind::OpenUrl => "https://example.com",
    };

    div()
        .w(px(480.0))
        .rounded(theme::radius_sheet())
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(ui::border_light(cx))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(title),
                )
                .child(action_button("关闭", ed_theme, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.close_editor(cx));
                    }
                })),
        )
        .child(
            div().px_3().py_2().max_h(px(480.0)).child(
                div()
                    .id("quick-launch-editor-scroll")
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(editor_field("名称", editor.name_input, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child("类型"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(segment_button(
                                        "脚本",
                                        editor.kind == ActionKind::Script,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_kind(ActionKind::Script, cx);
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "打开路径",
                                        editor.kind == ActionKind::OpenPath,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_kind(ActionKind::OpenPath, cx);
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "打开链接",
                                        editor.kind == ActionKind::OpenUrl,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_kind(ActionKind::OpenUrl, cx);
                                                });
                                            }
                                        },
                                    )),
                            ),
                    )
                    .child(if editor.kind == ActionKind::Script {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child("脚本类型"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(segment_button(
                                        "Shell",
                                        editor.script_type == ScriptType::Shell,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_script_type(
                                                        ScriptType::Shell,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "Node",
                                        editor.script_type == ScriptType::Node,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_script_type(
                                                        ScriptType::Node,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "Python",
                                        editor.script_type == ScriptType::Python,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_script_type(
                                                        ScriptType::Python,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "其他",
                                        editor.script_type == ScriptType::Other,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_script_type(
                                                        ScriptType::Other,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    )),
                            )
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if editor.kind == ActionKind::Script {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child("脚本来源"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(segment_button(
                                        "文件",
                                        editor.script_source == ScriptSource::Path,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_script_source(
                                                        ScriptSource::Path,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "内联",
                                        editor.script_source == ScriptSource::Inline,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_script_source(
                                                        ScriptSource::Inline,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    )),
                            )
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if editor.kind == ActionKind::Script {
                        editor_field("解释器（可选覆盖）", editor.interpreter_input, cx)
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if matches!(editor.kind, ActionKind::OpenPath)
                        || (editor.kind == ActionKind::Script
                            && editor.script_source == ScriptSource::Path)
                    {
                        editor_picker_field(
                            handle.clone(),
                            target_label,
                            "选择…",
                            editor.target_input,
                            cx,
                            QuickLaunchView::pick_editor_target,
                        )
                        .into_any_element()
                    } else {
                        editor_field(target_label, editor.target_input, cx).into_any_element()
                    })
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(ui::text_secondary(cx))
                            .child(target_hint),
                    )
                    .child(if editor.kind != ActionKind::OpenUrl {
                        editor_field("运行参数", editor.args_input, cx).into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if editor.kind == ActionKind::Script {
                        editor_picker_field(
                            handle.clone(),
                            "工作目录",
                            "选择…",
                            editor.cwd_input,
                            cx,
                            QuickLaunchView::pick_editor_cwd,
                        )
                        .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if editor.kind == ActionKind::Script {
                        editor_field("环境变量（每行 KEY=VALUE）", editor.env_input, cx)
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if editor.kind == ActionKind::Script {
                        editor_field("超时（秒）", editor.timeout_input, cx).into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(ui::text_secondary(cx))
                                    .child("反馈方式"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(segment_button(
                                        "静默",
                                        editor.feedback_mode == FeedbackMode::Silent,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_feedback_mode(
                                                        FeedbackMode::Silent,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "弹窗",
                                        editor.feedback_mode == FeedbackMode::Popup,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_feedback_mode(
                                                        FeedbackMode::Popup,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    ))
                                    .child(segment_button(
                                        "通知",
                                        editor.feedback_mode == FeedbackMode::Notification,
                                        cx,
                                        {
                                            let handle = handle.clone();
                                            move |_, cx| {
                                                let _ = cx.update_entity(&handle, |view, cx| {
                                                    view.set_editor_feedback_mode(
                                                        FeedbackMode::Notification,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    )),
                            ),
                    )
                    .child(editor_field(
                        "关键词（逗号分隔）",
                        editor.keywords_input,
                        cx,
                    ))
                    .child(editor_field(
                        "前缀（逗号分隔）",
                        editor.prefixes_input,
                        cx,
                    ))
                    .child(editor_field("图标字符（可选）", editor.icon_input, cx))
                    .child(editor_field("描述", editor.description_input, cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(ui::border_light(cx))
                            .bg(ui::bg_surface(cx))
                            .px(px(8.0))
                            .py_1()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.0))
                                    .child(div().text_size(px(10.0)).child("启用"))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(ui::text_secondary(cx))
                                            .child(if editor.enabled {
                                                "创建后会注册到启动器"
                                            } else {
                                                "保存在动作仓库，但不会出现在启动器"
                                            }),
                                    ),
                            )
                            .child(action_button(
                                if editor.enabled {
                                    "已启用"
                                } else {
                                    "已停用"
                                },
                                ed_theme,
                                {
                                    let handle = handle.clone();
                                    move |_, cx| {
                                        let _ = cx.update_entity(&handle, |view, cx| {
                                            view.toggle_editor_enabled(cx);
                                        });
                                    }
                                },
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary(cx))
                            .child(
                                "提示：在脚本、解释器、路径、参数、cwd、env 中使用 ${name} 可声明运行时参数。",
                            ),
                    ),
            ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(ui::border_light(cx))
                .flex()
                .justify_end()
                .gap_1()
                .child(action_button("取消", ed_theme, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.close_editor(cx));
                    }
                }))
                .child(primary_action_button("保存", ed_theme, move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| view.save_editor(cx));
                })),
        )
}

fn editor_target_placeholder(kind: ActionKind, script_source: ScriptSource) -> &'static str {
    match kind {
        ActionKind::Script => match script_source {
            ScriptSource::Inline => "脚本内容 / 支持多行",
            ScriptSource::Path => "脚本路径",
        },
        ActionKind::OpenPath => "目标路径",
        ActionKind::OpenUrl => "URL",
    }
}

fn editor_field(label: &'static str, input: Entity<InputState>, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_secondary(cx))
                .child(label),
        )
        .child(
            div()
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light(cx))
                .bg(ui::bg_surface(cx))
                .child(sheet_input_element(input, px(28.0))),
        )
}

fn editor_picker_field(
    handle: Entity<QuickLaunchView>,
    label: &'static str,
    button_label: &'static str,
    input: Entity<InputState>,
    cx: &App,
    on_pick: fn(&mut QuickLaunchView, &mut Context<QuickLaunchView>),
) -> impl IntoElement {
    let epf_theme = RowTheme::from_app(cx);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_secondary(cx))
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(ui::border_light(cx))
                        .bg(ui::bg_surface(cx))
                        .child(sheet_input_element(input, px(28.0))),
                )
                .child(action_button(button_label, epf_theme, move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| on_pick(view, cx));
                })),
        )
}

fn pending_sheet(
    handle: Entity<QuickLaunchView>,
    pending: PendingExecution,
    cx: &App,
) -> impl IntoElement {
    let pd_theme = RowTheme::from_app(cx);
    div()
        .w(px(440.0))
        .rounded(theme::radius_sheet())
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(ui::border_light(cx))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(format!("执行 {}", pending.action_name)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ui::text_secondary(cx))
                                .child("该动作包含参数占位符，先填写再执行"),
                        ),
                )
                .child(action_button("取消", pd_theme, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.close_pending(cx));
                    }
                })),
        )
        .child(
            div().px_3().py_2().flex().flex_col().gap_1p5().children(
                pending
                    .fields
                    .into_iter()
                    .enumerate()
                    .map(|(index, field)| parameter_row(field, index, cx)),
            ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(ui::border_light(cx))
                .flex()
                .justify_end()
                .gap_1()
                .child(action_button("稍后", pd_theme, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.close_pending(cx));
                    }
                }))
                .child(primary_action_button("执行", pd_theme, move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| view.submit_pending(cx));
                })),
        )
}

fn parameter_row(field: PendingParameterField, index: usize, cx: &App) -> impl IntoElement {
    let input = field.input;
    div()
        .id(("quick-launch-parameter", index))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_secondary(cx))
                .child(field.name),
        )
        .child(
            div()
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light(cx))
                .bg(ui::bg_surface(cx))
                .children(input.map(|input| sheet_input_element(input, px(28.0)))),
        )
}

fn sheet_input_element(state: Entity<InputState>, height: Pixels) -> Input {
    Input::new(&state)
        .appearance(false)
        .bordered(false)
        .focus_bordered(false)
        .h(height)
        .text_size(px(11.0))
}

fn history_sheet(
    handle: Entity<QuickLaunchView>,
    history: HistorySheetState,
    cx: &App,
) -> impl IntoElement {
    let hs_theme = RowTheme::from_app(cx);
    div()
        .w(px(560.0))
        .rounded(theme::radius_sheet())
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(ui::border_light(cx))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(format!("{} 的运行历史", history.action_name)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ui::text_secondary(cx))
                                .child(format!("最近 {} 条记录", history.runs.len())),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(action_button("刷新", hs_theme, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ = cx.update_entity(&handle, |view, cx| {
                                    view.refresh_history_panel(cx);
                                });
                            }
                        }))
                        .child(action_button("关闭", hs_theme, {
                            let handle = handle.clone();
                            move |_, cx| {
                                let _ =
                                    cx.update_entity(&handle, |view, cx| view.close_history(cx));
                            }
                        })),
                ),
        )
        .child(if history.runs.is_empty() {
            div()
                .min_h(px(140.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .text_color(ui::text_secondary(cx))
                .child("还没有运行记录")
                .into_any_element()
        } else {
            div()
                .px_2()
                .py_2()
                .flex()
                .flex_col()
                .gap_1p5()
                .children(history.runs.into_iter().enumerate().map(|(index, run)| {
                    history_row(handle.clone(), history.action_name.clone(), run, index, cx)
                }))
                .into_any_element()
        })
}

fn history_row(
    handle: Entity<QuickLaunchView>,
    action_name: String,
    run: QuickRun,
    index: usize,
    cx: &App,
) -> impl IntoElement {
    let hr_theme = RowTheme::from_app(cx);
    let tone = run_status_color(run.status, &hr_theme);
    let preview = if !run.stderr.trim().is_empty() {
        preview_text(&run.stderr)
    } else if !run.stdout.trim().is_empty() {
        preview_text(&run.stdout)
    } else {
        run.message.clone()
    };
    let run_for_result = run.clone();
    let rerun_action_id = run.action_id;
    let rerun_handle = handle.clone();

    div()
        .id(("quick-launch-history-row", index))
        .rounded(px(6.0))
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .px_2()
        .py_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(status_chip(
                    run_status_label(run.status).to_string(),
                    gpui::Rgba::from(tone),
                    hr_theme,
                ))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_secondary(cx))
                        .child(run.started_at.clone()),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_secondary(cx))
                        .child(format!("{} ms", run.duration_ms)),
                )
                .child(if let Some(code) = run.exit_code {
                    subtle_chip(format!("exit {code}"), hr_theme).into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
        .child(div().text_size(px(10.0)).child(run.message.clone()))
        .child(
            div()
                .flex()
                .items_end()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(if run.stderr.trim().is_empty() {
                            ui::text_secondary(cx)
                        } else {
                            tone
                        })
                        .child(preview),
                )
                .child(action_button("重新运行", hr_theme, move |_, cx| {
                    let _ = cx.update_entity(&rerun_handle, |view, cx| {
                        view.rerun_action_by_id(rerun_action_id, cx);
                    });
                }))
                .child(action_button("详情", hr_theme, move |_, cx| {
                    let action_name = action_name.clone();
                    let run = run_for_result.clone();
                    let _ = cx.update_entity(&handle, |view, cx| {
                        view.open_result(action_name, run, cx);
                    });
                })),
        )
}

fn result_sheet(
    handle: Entity<QuickLaunchView>,
    result: ResultSheetState,
    cx: &App,
) -> impl IntoElement {
    let result_theme = RowTheme::from_app(cx);
    let tone = run_status_color(result.run.status, &result_theme);
    let ok = result.run.status == RunStatus::Success;
    let status_line = result_meta_text(&result.run);
    let stderr_color = if result.run.stderr.trim().is_empty() {
        ui::text_secondary(cx)
    } else {
        tone
    };
    let rerun_action_id = result.run.action_id;
    let rerun_handle = handle.clone();

    div()
        .w(px(500.0))
        .rounded(theme::radius_sheet())
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(ui::border_light(cx))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("执行结果"),
                )
                .child(action_button("关闭", result_theme, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.close_result(cx));
                    }
                })),
        )
        .child(
            div().px_3().py_2().max_h(px(420.0)).child(
                div()
                    .id("quick-launch-result-scroll")
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(24.0))
                                    .rounded(px(12.0))
                                    .bg(if ok {
                                        hsla(0.36, 0.72, 0.42, 0.16)
                                    } else {
                                        hsla(0.0, 0.82, 0.58, 0.16)
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(tone)
                                    .child(if ok { "✓" } else { "!" }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child(result.action_name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(ui::text_secondary(cx))
                                            .child(status_line),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(ui::text_secondary(cx))
                            .child(result.run.message.clone()),
                    )
                    .child(result_block(
                        handle.clone(),
                        "stdout",
                        result.run.stdout,
                        ui::text_primary(cx),
                        "复制 stdout",
                        QuickLaunchView::copy_result_stdout,
                        cx,
                    ))
                    .child(result_block(
                        handle.clone(),
                        "stderr",
                        result.run.stderr,
                        stderr_color,
                        "复制 stderr",
                        QuickLaunchView::copy_result_stderr,
                        cx,
                    )),
            ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(ui::border_light(cx))
                .flex()
                .justify_between()
                .child(action_button("重新运行", result_theme, move |_, cx| {
                    let _ = cx.update_entity(&rerun_handle, |view, cx| {
                        view.rerun_action_by_id(rerun_action_id, cx);
                    });
                }))
                .child(primary_action_button(
                    "完成",
                    result_theme,
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.close_result(cx));
                    },
                )),
        )
}

fn result_block(
    handle: Entity<QuickLaunchView>,
    title: &'static str,
    content: String,
    text_color: gpui::Hsla,
    copy_label: &'static str,
    on_copy: fn(&mut QuickLaunchView, &mut Context<QuickLaunchView>),
    cx: &App,
) -> impl IntoElement {
    let rb_theme = RowTheme::from_app(cx);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_secondary(cx))
                        .child(title),
                )
                .child(action_button(copy_label, rb_theme, move |_, cx| {
                    let _ = cx.update_entity(&handle, |view, cx| on_copy(view, cx));
                })),
        )
        .child(
            div()
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light(cx))
                .bg(ui::bg_surface(cx))
                .max_h(px(140.0))
                .child(
                    div()
                        .id(title)
                        .overflow_y_scroll()
                        .px_2()
                        .py_1()
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(text_color)
                        .child(result_block_text(&content)),
                ),
        )
}

fn overlay_shell(
    cx: &App,
    backdrop_id: &'static str,
    on_close: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
    content: impl IntoElement,
) -> impl IntoElement {
    components::overlay_host(
        backdrop_id,
        move |event, _window, cx| on_close(event, cx),
        content,
        cx,
    )
}

fn menu_overlay_shell(
    handle: Entity<QuickLaunchView>,
    menu: ActionMenuState,
    running: bool,
    cx: &App,
) -> impl IntoElement {
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.001))
                .id("quick-launch-menu-backdrop")
                .on_click({
                    let handle = handle.clone();
                    move |_, _window, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| view.close_action_menu(cx));
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top(menu.position.y)
                .left(menu.position.x)
                .w(px(160.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light(cx))
                .bg(ui::bg_surface(cx))
                .shadow_lg()
                .p_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .children(action_menu_items(handle, menu.action, running, cx)),
        )
}

fn primary_action_button(
    label: &'static str,
    _theme: RowTheme,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement + 'static {
    Button::new(label)
        .label(label)
        .small()
        .primary()
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn action_button(
    label: &'static str,
    _theme: RowTheme,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement + 'static {
    Button::new(label)
        .label(label)
        .small()
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn window_action_button(
    label: &'static str,
    _theme: RowTheme,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement + 'static {
    Button::new(label)
        .label(label)
        .small()
        .on_click(move |event, window, cx| on_click(event, window, cx))
}

fn icon_action_button(
    label: &'static str,
    _theme: RowTheme,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement + 'static {
    Button::new(label)
        .label(label)
        .small()
        .with_size(gpui_component::Size::Size(px(26.0)))
        .compact()
        .on_click(move |event, window, cx| on_click(event, window, cx))
}

fn action_menu_items(
    handle: Entity<QuickLaunchView>,
    action: QuickAction,
    running: bool,
    cx: &App,
) -> Vec<gpui::AnyElement> {
    let mut items = Vec::new();

    if running {
        items.push(
            action_menu_item("停止", true, cx, {
                let handle = handle.clone();
                let action = action.clone();
                move |_, cx| {
                    let action = action.clone();
                    let _ = cx.update_entity(&handle, |view, cx| {
                        view.stop_action(action.id, cx);
                        view.close_action_menu(cx);
                    });
                }
            })
            .into_any_element(),
        );
    }

    items.push(
        action_menu_item(
            if action.enabled { "停用" } else { "启用" },
            false,
            cx,
            {
                let handle = handle.clone();
                let action = action.clone();
                move |_, cx| {
                    let action = action.clone();
                    let _ = cx.update_entity(&handle, |view, cx| {
                        view.set_action_enabled(action.clone(), !action.enabled, cx);
                    });
                }
            },
        )
        .into_any_element(),
    );
    items.push(
        action_menu_item("复制", false, cx, {
            let handle = handle.clone();
            let action = action.clone();
            move |_, cx| {
                let action = action.clone();
                let _ = cx.update_entity(&handle, |view, cx| {
                    view.duplicate_action(action, cx);
                });
            }
        })
        .into_any_element(),
    );
    items.push(
        action_menu_item("查看运行历史", false, cx, {
            let handle = handle.clone();
            let action = action.clone();
            move |_, cx| {
                let action = action.clone();
                let _ = cx.update_entity(&handle, |view, cx| {
                    view.open_action_history(action, cx);
                });
            }
        })
        .into_any_element(),
    );
    items.push(action_menu_separator(cx).into_any_element());
    items.push(
        action_menu_item("删除", true, cx, move |_, cx| {
            let action = action.clone();
            let _ = cx.update_entity(&handle, |view, cx| {
                view.request_delete_action(action, cx);
            });
        })
        .into_any_element(),
    );

    items
}

fn action_menu_item(
    label: &'static str,
    destructive: bool,
    cx: &App,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    let hover_bg = ui::row_hover(cx);
    div()
        .id(label)
        .h(px(26.0))
        .w_full()
        .px(px(8.0))
        .rounded(px(4.0))
        .hover(move |style| style.bg(hover_bg).cursor_pointer())
        .flex()
        .items_center()
        .text_size(px(10.0))
        .text_color(if destructive {
            ui::danger(cx)
        } else {
            ui::text_primary(cx)
        })
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn action_menu_separator(cx: &App) -> impl IntoElement {
    div()
        .w_full()
        .h(px(1.0))
        .my(px(2.0))
        .bg(ui::border_light(cx))
}

fn menu_position(click: Point<Pixels>, window: &Window) -> Point<Pixels> {
    let viewport = window.bounds().size;
    let width = px(160.0);
    let height = px(144.0);
    let margin = px(12.0);
    let max_x = (viewport.width - width - margin).max(margin);
    let max_y = (viewport.height - height - margin).max(margin);
    point(
        click.x.min(max_x).max(margin),
        click.y.min(max_y).max(margin),
    )
}

fn delete_confirm_sheet(
    handle: Entity<QuickLaunchView>,
    delete_confirm: DeleteConfirmState,
    cx: &App,
) -> impl IntoElement {
    let dc_theme = RowTheme::from_app(cx);
    div()
        .w(px(380.0))
        .rounded(theme::radius_sheet())
        .border_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .px_3()
                .py_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("删除动作"),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(14.0))
                        .text_color(ui::text_secondary(cx))
                        .child(format!(
                            "确定要删除动作 “{}” 吗？此操作不可撤销。",
                            delete_confirm.action_name
                        )),
                ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(ui::border_light(cx))
                .flex()
                .justify_end()
                .gap_1()
                .child(action_button("取消", dc_theme, {
                    let handle = handle.clone();
                    move |_, cx| {
                        let _ = cx.update_entity(&handle, |view, cx| {
                            view.close_delete_confirm(cx);
                        });
                    }
                }))
                .child(destructive_action_button("删除", cx, move |_, cx| {
                    let action_id = delete_confirm.action_id;
                    let _ = cx.update_entity(&handle, |view, cx| {
                        view.confirm_delete_action(action_id, cx);
                    });
                })),
        )
}

fn destructive_action_button(
    label: &'static str,
    _cx: &App,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    Button::new(label)
        .label(label)
        .small()
        .danger()
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn kind_chip(label: String, theme: RowTheme) -> impl IntoElement + 'static {
    div()
        .h(px(18.0))
        .px(px(6.0))
        .rounded(px(3.0))
        .bg(theme.primary)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(ui::accent_color(
            qingqi_plugin::plugin_spec::PluginAccent::Blue,
        ))
        .child(label)
}

fn subtle_chip(label: String, theme: RowTheme) -> impl IntoElement + 'static {
    div()
        .h(px(18.0))
        .px(px(6.0))
        .rounded(px(3.0))
        .bg(theme.background)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(theme.text_secondary)
        .child(label)
}

fn status_chip(label: String, tone: gpui::Rgba, theme: RowTheme) -> impl IntoElement + 'static {
    div()
        .h(px(18.0))
        .px(px(6.0))
        .rounded(px(3.0))
        .bg(if theme.is_dark {
            hsla(0.0, 0.0, 1.0, 0.08)
        } else {
            hsla(0.0, 0.0, 0.0, 0.05)
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(tone)
        .child(label)
}

fn latest_run_status_chip(
    action_id: i64,
    latest_run_summaries: &HashMap<i64, RunSummary>,
    theme: RowTheme,
) -> impl IntoElement + 'static {
    let Some(summary) = latest_run_summaries.get(&action_id) else {
        return div().into_any_element();
    };
    let tone = run_status_color(summary.status, &theme);
    div()
        .h(px(18.0))
        .px(px(6.0))
        .rounded(px(3.0))
        .bg(hsla(0.0, 0.0, if theme.is_dark { 1.0 } else { 0.0 }, 0.06))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(tone)
        .child(summary.chip_label())
        .into_any_element()
}

fn segment_button(
    label: &'static str,
    active: bool,
    _cx: &App,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    Button::new(label)
        .label(label)
        .small()
        .selected(active)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn build_editor_state(
    action: Option<QuickAction>,
    window: &mut Window,
    cx: &mut Context<QuickLaunchView>,
) -> ActionEditorState {
    let name_input = sheet_input(window, cx, "为该动作起一个名字", "");
    let description_input = sheet_input(window, cx, "可选描述", "");
    let args_input = sheet_input(window, cx, "--flag value", "");
    let cwd_input = sheet_input(window, cx, "可选，默认当前目录", "");
    let interpreter_input = sheet_input(window, cx, "如 /opt/homebrew/bin/ruby", "");
    let env_input = multiline_sheet_input(window, cx, "每行 KEY=VALUE", "");
    let keywords_input = sheet_input(window, cx, "逗号分隔，便于搜索", "");
    let prefixes_input = sheet_input(window, cx, "逗号分隔，如 ql, git", "");
    let icon_input = sheet_input(window, cx, "如：⌘ / 🌐 / 📁", "");
    let timeout_input = sheet_input(window, cx, "300", "300");

    if let Some(action) = action {
        let placeholder = editor_target_placeholder(action.kind, action.script_source);
        let target_input = editor_target_input(
            window,
            cx,
            action.kind,
            action.script_source,
            placeholder,
            "",
        );
        name_input.update(cx, |input, cx| input.reset_value(action.name.clone(), cx));
        description_input.update(cx, |input, cx| {
            input.reset_value(action.description.clone(), cx)
        });
        target_input.update(cx, |input, cx| {
            input.reset_value(
                match action.kind {
                    ActionKind::Script => match action.script_source {
                        ScriptSource::Inline => action.script_body.clone(),
                        ScriptSource::Path => action.path.clone(),
                    },
                    ActionKind::OpenPath => action.path.clone(),
                    ActionKind::OpenUrl => action.url.clone(),
                },
                cx,
            )
        });
        args_input.update(cx, |input, cx| {
            input.reset_value(join_shell_words(&action.args), cx)
        });
        cwd_input.update(cx, |input, cx| input.reset_value(action.cwd.clone(), cx));
        interpreter_input.update(cx, |input, cx| {
            input.reset_value(action.interpreter.clone(), cx)
        });
        env_input.update(cx, |input, cx| {
            input.reset_value(format_env_lines(&action.env), cx)
        });
        keywords_input.update(cx, |input, cx| {
            input.reset_value(action.keywords.join(", "), cx)
        });
        prefixes_input.update(cx, |input, cx| {
            input.reset_value(action.prefixes.join(", "), cx)
        });
        icon_input.update(cx, |input, cx| input.reset_value(action.icon.clone(), cx));
        timeout_input.update(cx, |input, cx| {
            input.reset_value(action.timeout_sec.to_string(), cx);
        });

        ActionEditorState {
            mode: ActionEditorMode::Edit(action.id),
            name_input,
            description_input,
            target_input,
            args_input,
            cwd_input,
            interpreter_input,
            env_input,
            keywords_input,
            prefixes_input,
            icon_input,
            timeout_input,
            kind: action.kind,
            script_type: action.script_type,
            script_source: action.script_source,
            feedback_mode: action.feedback_mode,
            enabled: action.enabled,
        }
    } else {
        let placeholder = editor_target_placeholder(ActionKind::Script, ScriptSource::Inline);
        let target_input = editor_target_input(
            window,
            cx,
            ActionKind::Script,
            ScriptSource::Inline,
            placeholder,
            "",
        );
        ActionEditorState {
            mode: ActionEditorMode::Create,
            name_input,
            description_input,
            target_input,
            args_input,
            cwd_input,
            interpreter_input,
            env_input,
            keywords_input,
            prefixes_input,
            icon_input,
            timeout_input,
            kind: ActionKind::Script,
            script_type: ScriptType::Shell,
            script_source: ScriptSource::Inline,
            feedback_mode: FeedbackMode::Notification,
            enabled: true,
        }
    }
}

fn sheet_input(
    window: &mut Window,
    cx: &mut Context<QuickLaunchView>,
    placeholder: impl Into<String>,
    value: impl Into<String>,
) -> Entity<InputState> {
    let placeholder = placeholder.into();
    let value = value.into();
    cx.new(move |cx| {
        let mut input = InputState::new(window, cx);
        input.set_placeholder(placeholder.clone(), window, cx);
        input.reset_value(value.clone(), cx);
        input
    })
}

fn editor_target_input(
    window: &mut Window,
    cx: &mut Context<QuickLaunchView>,
    kind: ActionKind,
    script_source: ScriptSource,
    placeholder: &str,
    value: impl Into<String>,
) -> Entity<InputState> {
    let placeholder = placeholder.to_string();
    let value = value.into();
    let is_inline_script = kind == ActionKind::Script && script_source == ScriptSource::Inline;
    cx.new(move |cx| {
        let mut input = if is_inline_script {
            InputState::new(window, cx)
                .multi_line(true)
                .searchable(true)
                .soft_wrap(true)
        } else {
            InputState::new(window, cx)
        };
        input.set_placeholder(placeholder.clone(), window, cx);
        input.reset_value(value.clone(), cx);
        input
    })
}

fn multiline_sheet_input(
    window: &mut Window,
    cx: &mut Context<QuickLaunchView>,
    placeholder: impl Into<String>,
    value: impl Into<String>,
) -> Entity<InputState> {
    let placeholder = placeholder.into();
    let value = value.into();
    cx.new(move |cx| {
        let mut input = InputState::new(window, cx)
            .multi_line(true)
            .searchable(true)
            .soft_wrap(true);
        input.set_placeholder(placeholder.clone(), window, cx);
        input.reset_value(value.clone(), cx);
        input
    })
}

fn split_csv(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_env_lines(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let (key, value) = trimmed.split_once('=')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), value.trim().to_string()))
        })
        .collect()
}

fn format_env_lines(env: &HashMap<String, String>) -> String {
    let mut items = env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>();
    items.sort();
    items.join("\n")
}

fn subtitle_for(action: &QuickAction) -> String {
    if !action.description.trim().is_empty() {
        return preview_text(&action.description);
    }
    if action.kind == ActionKind::Script && !action.interpreter.trim().is_empty() {
        return format!(
            "{} · {}",
            action.script_type.label(),
            preview_text(&action.interpreter)
        );
    }
    if !action.path.trim().is_empty() {
        return preview_text(&action.path);
    }
    if !action.url.trim().is_empty() {
        return preview_text(&action.url);
    }
    if !action.script_body.trim().is_empty() {
        return preview_text(&action.script_body);
    }
    String::from("未配置描述")
}

fn preview_text(text: &str) -> String {
    let compact = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if compact.len() > 120 {
        format!("{}…", &compact[..120])
    } else {
        compact
    }
}

fn result_block_text(text: &str) -> String {
    if text.trim().is_empty() {
        String::from("(空)")
    } else {
        text.to_string()
    }
}

fn icon_for_action(action: &QuickAction) -> String {
    if !action.icon.trim().is_empty() {
        return action.icon.clone();
    }
    match action.kind {
        crate::model::ActionKind::Script => String::from("⌘"),
        crate::model::ActionKind::OpenPath => String::from("📁"),
        crate::model::ActionKind::OpenUrl => String::from("🌐"),
    }
}

fn feedback_label(mode: FeedbackMode) -> &'static str {
    match mode {
        FeedbackMode::Silent => "静默",
        FeedbackMode::Popup => "弹窗",
        FeedbackMode::Notification => "通知",
    }
}

fn run_status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Success => "成功",
        RunStatus::Failed => "失败",
        RunStatus::Timeout => "超时",
        RunStatus::Stopped => "已停止",
        RunStatus::Error => "错误",
    }
}

fn result_meta_text(run: &QuickRun) -> String {
    let mut parts = vec![run_status_label(run.status).to_string()];
    if let Some(code) = run.exit_code {
        parts.push(format!("exit {code}"));
    }
    parts.push(format!("{} ms", run.duration_ms));
    parts.push(run.started_at.clone());
    parts.join("  ·  ")
}

fn run_status_color(status: RunStatus, theme: &RowTheme) -> gpui::Hsla {
    match status {
        RunStatus::Success => theme.success.into(),
        RunStatus::Timeout => theme.warning.into(),
        RunStatus::Stopped => theme.text_secondary,
        RunStatus::Failed | RunStatus::Error => theme.danger.into(),
    }
}
