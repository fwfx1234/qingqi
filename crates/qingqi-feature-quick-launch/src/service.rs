#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    collections::{HashMap, HashSet},
    process::{Command as ProcessCommand, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, ensure};
use time::{OffsetDateTime, macros::format_description};

use crate::{
    manifest::PLUGIN_ID,
    model::{
        ActionKind, FeedbackMode, QuickAction, QuickActionDraft, QuickRun, QuickRunDraft,
        RunStatus, ScriptSource, ScriptType,
    },
    parameters::{
        MissingParameterError, ParameterSpec, split_shell_words, substitute, substitute_mapping,
        substitute_vec,
    },
    store::QuickLaunchStore,
};
use qingqi_plugin::{database::DatabaseService, storage::AppPaths};

pub struct QuickLaunchService {
    store: Mutex<QuickLaunchStore>,
    execution: Mutex<ExecutionState>,
    revision: AtomicU64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickLaunchEvent {
    pub revision: u64,
    pub action_id: i64,
    pub action_name: String,
    pub feedback_mode: FeedbackMode,
    pub message: String,
    pub run: Option<QuickRun>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickLaunchRuntimeSnapshot {
    pub revision: u64,
    pub running_action_ids: Vec<i64>,
    pub last_event: Option<QuickLaunchEvent>,
}

/// Lightweight display-oriented run summary — does not carry stdout/stderr.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunSummary {
    pub status: RunStatus,
    pub exit_code: Option<i64>,
    pub duration_ms: i64,
}

impl RunSummary {
    pub fn from_run(run: &QuickRun) -> Self {
        Self {
            status: run.status,
            exit_code: run.exit_code,
            duration_ms: run.duration_ms,
        }
    }

    /// Compact chip label for action rows, e.g. "成功 · 12ms".
    pub fn chip_label(&self) -> String {
        match self.status {
            RunStatus::Success => format!("上次成功 · {}ms", self.duration_ms),
            RunStatus::Failed => {
                if let Some(code) = self.exit_code {
                    format!("上次失败 · exit {} · {}ms", code, self.duration_ms)
                } else {
                    format!("上次失败 · {}ms", self.duration_ms)
                }
            }
            RunStatus::Timeout => format!("上次超时 · {}ms", self.duration_ms),
            RunStatus::Stopped => format!("已停止 · {}ms", self.duration_ms),
            RunStatus::Error => format!("上次出错 · {}ms", self.duration_ms),
        }
    }
}

#[derive(Default)]
struct ExecutionState {
    running_action_ids: HashSet<i64>,
    stopping_action_ids: HashSet<i64>,
    active_pids: HashMap<i64, u32>,
    last_event: Option<QuickLaunchEvent>,
}

impl QuickLaunchService {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> Result<Self> {
        let _ = paths;
        let store = QuickLaunchStore::open(
            database,
            &qingqi_plugin::database::feature_database_key(PLUGIN_ID, "actions"),
        )?;
        let service = Self {
            store: Mutex::new(store),
            execution: Mutex::new(ExecutionState::default()),
            revision: AtomicU64::new(0),
        };
        service.seed_defaults()?;
        Ok(service)
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub fn runtime_snapshot(&self) -> QuickLaunchRuntimeSnapshot {
        let state = self.execution.lock().expect("quick launch state poisoned");
        let mut running_action_ids = state.running_action_ids.iter().copied().collect::<Vec<_>>();
        running_action_ids.sort_unstable();
        QuickLaunchRuntimeSnapshot {
            revision: self.revision(),
            running_action_ids,
            last_event: state.last_event.clone(),
        }
    }

    pub fn list_actions(&self, query: &str, enabled: Option<bool>) -> Result<Vec<QuickAction>> {
        let store = self.store.lock().expect("quick launch store poisoned");
        let actions = store.list_actions(enabled)?;
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(actions);
        }

        Ok(actions
            .into_iter()
            .filter(|action| matches_query(action, &query))
            .collect())
    }

    pub fn create_action(&self, draft: QuickActionDraft) -> Result<QuickAction> {
        validate_action_draft(&draft)?;
        let store = self.store.lock().expect("quick launch store poisoned");
        let action = store.create_action(&draft)?;
        let message = format!("已创建动作 {}", action.name);
        self.publish_notice(
            action.id,
            action.name.clone(),
            action.feedback_mode,
            message,
        );
        Ok(action)
    }

    pub fn update_action(&self, action_id: i64, draft: QuickActionDraft) -> Result<QuickAction> {
        validate_action_draft(&draft)?;
        let store = self.store.lock().expect("quick launch store poisoned");
        ensure!(
            store.update_action(action_id, &draft)?,
            "未找到动作 {action_id}"
        );
        let action = store
            .get_action(action_id)?
            .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?;
        let message = format!("已保存动作 {}", action.name);
        self.publish_notice(
            action.id,
            action.name.clone(),
            action.feedback_mode,
            message,
        );
        Ok(action)
    }

    pub fn set_action_enabled(&self, action_id: i64, enabled: bool) -> Result<QuickAction> {
        let action = {
            let store = self.store.lock().expect("quick launch store poisoned");
            store
                .get_action(action_id)?
                .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?
        };
        let mut draft = action_to_draft(&action);
        draft.enabled = enabled;
        let updated = self.update_action(action_id, draft)?;
        Ok(updated)
    }

    pub fn duplicate_action(&self, action_id: i64) -> Result<QuickAction> {
        let action = {
            let store = self.store.lock().expect("quick launch store poisoned");
            store
                .get_action(action_id)?
                .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?
        };
        let mut draft = action_to_draft(&action);
        draft.name = format!("{} 副本", action.name);
        draft.sort_order = None;
        let created = self.create_action(draft)?;
        Ok(created)
    }

    pub fn delete_action(&self, action_id: i64) -> Result<String> {
        ensure!(
            !self.is_running(action_id),
            "动作正在执行，请先停止后再删除"
        );
        let action = {
            let store = self.store.lock().expect("quick launch store poisoned");
            store
                .get_action(action_id)?
                .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?
        };
        let store = self.store.lock().expect("quick launch store poisoned");
        ensure!(store.delete_action(action_id)?, "未找到动作 {action_id}");
        let message = format!("已删除动作 {}", action.name);
        self.publish_notice(
            action.id,
            action.name.clone(),
            action.feedback_mode,
            message.clone(),
        );
        Ok(message)
    }

    pub fn is_running(&self, action_id: i64) -> bool {
        let state = self.execution.lock().expect("quick launch state poisoned");
        state.running_action_ids.contains(&action_id)
    }

    #[allow(dead_code)]
    pub fn execute_action(&self, action_id: i64) -> Result<String> {
        self.execute_action_with_values(action_id, &HashMap::new())
    }

    pub fn start_action(self: &Arc<Self>, action_id: i64) -> Result<String> {
        self.start_action_with_values(action_id, HashMap::new())
    }

    pub fn start_action_with_values(
        self: &Arc<Self>,
        action_id: i64,
        values: HashMap<String, String>,
    ) -> Result<String> {
        let action = {
            let store = self.store.lock().expect("quick launch store poisoned");
            store
                .get_action(action_id)?
                .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?
        };
        ensure!(action.enabled, "该动作已停用");
        let prepared = prepare_action(&action, &values)?;
        let start_message = format!("已开始执行 {}", prepared.name);
        self.mark_running(&prepared, start_message.clone())?;

        let service = Arc::clone(self);
        thread::spawn(move || {
            let action_id = prepared.id;
            match service.execute_prepared_and_record(&prepared) {
                Ok(run) => service.mark_finished(&prepared, run),
                Err(error) => service.mark_finished(
                    &prepared,
                    QuickRun {
                        id: 0,
                        action_id,
                        status: RunStatus::Error,
                        exit_code: None,
                        stdout: String::new(),
                        stderr: error.to_string(),
                        duration_ms: 0,
                        started_at: now_label(),
                        finished_at: now_label(),
                        message: format!("执行失败: {error}"),
                    },
                ),
            }
        });

        Ok(start_message)
    }

    pub fn required_parameters(&self, action_id: i64) -> Result<Vec<ParameterSpec>> {
        let store = self.store.lock().expect("quick launch store poisoned");
        let action = store
            .get_action(action_id)?
            .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?;
        Ok(action.parameter_specs())
    }

    #[allow(dead_code)]
    pub fn execute_action_with_values(
        &self,
        action_id: i64,
        values: &HashMap<String, String>,
    ) -> Result<String> {
        let action = {
            let store = self.store.lock().expect("quick launch store poisoned");
            store
                .get_action(action_id)?
                .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?
        };
        ensure!(action.enabled, "该动作已停用");
        let prepared = prepare_action(&action, values)?;
        Ok(self.execute_prepared_and_record(&prepared)?.message)
    }

    pub fn list_runs(&self, action_id: i64, limit: usize) -> Result<Vec<QuickRun>> {
        let store = self.store.lock().expect("quick launch store poisoned");
        store.list_runs(action_id, limit)
    }

    pub fn latest_runs(&self, action_ids: &[i64]) -> Result<HashMap<i64, QuickRun>> {
        let store = self.store.lock().expect("quick launch store poisoned");
        store.latest_run_for_actions(action_ids)
    }

    pub fn latest_run_summaries(&self, action_ids: &[i64]) -> Result<HashMap<i64, RunSummary>> {
        let store = self.store.lock().expect("quick launch store poisoned");
        let runs = store.latest_run_for_actions(action_ids)?;
        Ok(runs
            .into_iter()
            .map(|(id, run)| (id, RunSummary::from_run(&run)))
            .collect())
    }

    pub fn stop_action(&self, action_id: i64) -> Result<String> {
        let action = {
            let store = self.store.lock().expect("quick launch store poisoned");
            store
                .get_action(action_id)?
                .ok_or_else(|| anyhow!("未找到动作 {action_id}"))?
        };
        let pid = {
            let mut state = self.execution.lock().expect("quick launch state poisoned");
            ensure!(
                state.running_action_ids.contains(&action_id),
                "动作当前未在运行"
            );
            state.stopping_action_ids.insert(action_id);
            let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
            state.last_event = Some(QuickLaunchEvent {
                revision,
                action_id,
                action_name: action.name.clone(),
                feedback_mode: action.feedback_mode,
                message: String::from("已请求停止动作"),
                run: None,
            });
            state.active_pids.get(&action_id).copied()
        };

        if let Some(pid) = pid {
            let _ = signal_process(pid, "TERM");
        }

        Ok(String::from("已请求停止动作"))
    }

    fn mark_running(&self, action: &PreparedAction, message: String) -> Result<()> {
        let mut state = self.execution.lock().expect("quick launch state poisoned");
        ensure!(
            !state.running_action_ids.contains(&action.id),
            "动作正在执行，请稍候"
        );
        state.running_action_ids.insert(action.id);
        state.stopping_action_ids.remove(&action.id);
        state.active_pids.remove(&action.id);
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        state.last_event = Some(QuickLaunchEvent {
            revision,
            action_id: action.id,
            action_name: action.name.clone(),
            feedback_mode: action.feedback_mode,
            message,
            run: None,
        });
        Ok(())
    }

    fn mark_finished(&self, action: &PreparedAction, run: QuickRun) {
        let mut state = self.execution.lock().expect("quick launch state poisoned");
        state.running_action_ids.remove(&action.id);
        state.stopping_action_ids.remove(&action.id);
        state.active_pids.remove(&action.id);
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        state.last_event = Some(QuickLaunchEvent {
            revision,
            action_id: action.id,
            action_name: action.name.clone(),
            feedback_mode: action.feedback_mode,
            message: run.message.clone(),
            run: Some(run),
        });
    }

    fn set_active_pid(&self, action_id: i64, pid: u32) {
        let mut state = self.execution.lock().expect("quick launch state poisoned");
        state.active_pids.insert(action_id, pid);
    }

    fn clear_active_pid(&self, action_id: i64) {
        let mut state = self.execution.lock().expect("quick launch state poisoned");
        state.active_pids.remove(&action_id);
    }

    fn publish_notice(
        &self,
        action_id: i64,
        action_name: String,
        feedback_mode: FeedbackMode,
        message: String,
    ) {
        let mut state = self.execution.lock().expect("quick launch state poisoned");
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        state.last_event = Some(QuickLaunchEvent {
            revision,
            action_id,
            action_name,
            feedback_mode,
            message,
            run: None,
        });
    }

    fn stop_requested(&self, action_id: i64) -> bool {
        let state = self.execution.lock().expect("quick launch state poisoned");
        state.stopping_action_ids.contains(&action_id)
    }

    fn execute_prepared_and_record(&self, action: &PreparedAction) -> Result<QuickRun> {
        let started_at = now_label();
        let timer = Instant::now();
        let capture = run_action_capture(self, action);
        let finished_at = now_label();
        let duration_ms = timer.elapsed().as_millis() as i64;

        let run = QuickRunDraft {
            action_id: action.id,
            status: capture.status,
            exit_code: capture.exit_code,
            stdout: capture.stdout,
            stderr: capture.stderr,
            duration_ms,
            started_at,
            finished_at,
            message: capture.message.clone(),
        };
        let store = self.store.lock().expect("quick launch store poisoned");
        store.record_run(&run)
    }

    fn seed_defaults(&self) -> Result<()> {
        let store = self.store.lock().expect("quick launch store poisoned");
        let _ = store.seed_defaults(&default_actions())?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreparedAction {
    id: i64,
    name: String,
    kind: ActionKind,
    script_type: ScriptType,
    script_source: ScriptSource,
    script_body: String,
    interpreter: String,
    path: String,
    url: String,
    args: Vec<String>,
    cwd: String,
    env: HashMap<String, String>,
    timeout_sec: i64,
    feedback_mode: FeedbackMode,
    enabled: bool,
}

struct RunCapture {
    status: RunStatus,
    exit_code: Option<i64>,
    stdout: String,
    stderr: String,
    message: String,
}

fn prepare_action(
    action: &QuickAction,
    values: &HashMap<String, String>,
) -> Result<PreparedAction> {
    Ok(PreparedAction {
        id: action.id,
        name: action.name.clone(),
        kind: action.kind,
        script_type: action.script_type,
        script_source: action.script_source,
        script_body: substitute(&action.script_body, values, false, true)
            .map_err(parameter_error)?,
        interpreter: substitute(&action.interpreter, values, false, true)
            .map_err(parameter_error)?,
        path: substitute(&action.path, values, false, true).map_err(parameter_error)?,
        url: substitute(&action.url, values, false, true).map_err(parameter_error)?,
        args: substitute_vec(&action.args, values, false, true).map_err(parameter_error)?,
        cwd: substitute(&action.cwd, values, false, true).map_err(parameter_error)?,
        env: substitute_mapping(&action.env, values, false, true).map_err(parameter_error)?,
        timeout_sec: action.timeout_sec,
        feedback_mode: action.feedback_mode,
        enabled: action.enabled,
    })
}

fn action_to_draft(action: &QuickAction) -> QuickActionDraft {
    QuickActionDraft {
        name: action.name.clone(),
        description: action.description.clone(),
        kind: action.kind,
        script_type: action.script_type,
        script_source: action.script_source,
        script_body: action.script_body.clone(),
        interpreter: action.interpreter.clone(),
        path: action.path.clone(),
        url: action.url.clone(),
        args: action.args.clone(),
        cwd: action.cwd.clone(),
        env: action.env.clone(),
        keywords: action.keywords.clone(),
        prefixes: action.prefixes.clone(),
        icon: action.icon.clone(),
        feedback_mode: action.feedback_mode,
        timeout_sec: action.timeout_sec,
        enabled: action.enabled,
        sort_order: Some(action.sort_order),
    }
}

fn validate_action_draft(draft: &QuickActionDraft) -> Result<()> {
    ensure!(!draft.name.trim().is_empty(), "动作名称不能为空");
    ensure!(draft.timeout_sec >= 0, "超时必须大于或等于 0");
    match draft.kind {
        ActionKind::Script => match draft.script_source {
            ScriptSource::Inline => {
                ensure!(!draft.script_body.trim().is_empty(), "脚本内容不能为空");
            }
            ScriptSource::Path => {
                ensure!(!draft.path.trim().is_empty(), "脚本路径不能为空");
            }
        },
        ActionKind::OpenPath => ensure!(!draft.path.trim().is_empty(), "目标路径不能为空"),
        ActionKind::OpenUrl => ensure!(!draft.url.trim().is_empty(), "URL 不能为空"),
    }
    Ok(())
}

fn parameter_error(error: MissingParameterError) -> anyhow::Error {
    anyhow!(error.to_string())
}

fn run_action_capture(service: &QuickLaunchService, action: &PreparedAction) -> RunCapture {
    match action.kind {
        ActionKind::Script => run_script_action(service, action),
        ActionKind::OpenPath => {
            if action.path.trim().is_empty() {
                return RunCapture {
                    status: RunStatus::Error,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::from("路径不能为空"),
                    message: String::from("执行失败: 路径不能为空"),
                };
            }
            let mut command = ProcessCommand::new("open");
            command.arg(&action.path);
            command.args(&action.args);
            match command
                .output()
                .with_context(|| format!("打开路径失败: {}", action.path))
            {
                Ok(output) => capture_from_output(action, output),
                Err(error) => error_capture(&error.to_string()),
            }
        }
        ActionKind::OpenUrl => {
            if action.url.trim().is_empty() {
                return RunCapture {
                    status: RunStatus::Error,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::from("URL 不能为空"),
                    message: String::from("执行失败: URL 不能为空"),
                };
            }
            match ProcessCommand::new("open")
                .arg(&action.url)
                .output()
                .with_context(|| format!("打开 URL 失败: {}", action.url))
            {
                Ok(output) => capture_from_output(action, output),
                Err(error) => error_capture(&error.to_string()),
            }
        }
    }
}

fn run_script_action(service: &QuickLaunchService, action: &PreparedAction) -> RunCapture {
    let Some((program, arguments)) = build_script_command(action) else {
        return error_capture("脚本配置不完整");
    };

    let mut command = ProcessCommand::new(&program);
    command
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    if !action.cwd.trim().is_empty() {
        command.current_dir(&action.cwd);
    }
    if !action.env.is_empty() {
        command.envs(
            action
                .env
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
    }

    let child = match command
        .spawn()
        .with_context(|| format!("执行脚本失败: {}", action.name))
    {
        Ok(child) => child,
        Err(error) => return error_capture(&error.to_string()),
    };

    let pid = child.id();
    service.set_active_pid(action.id, pid);

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    let started = Instant::now();
    let mut timed_out = false;
    let mut stop_sent = false;
    let mut forced_kill = false;
    let mut signal_deadline = None;

    loop {
        match rx.try_recv() {
            Ok(result) => {
                service.clear_active_pid(action.id);
                return match result {
                    Ok(output) => {
                        let mut capture = capture_from_output(action, output);
                        let stop_requested = stop_sent || service.stop_requested(action.id);
                        if timed_out {
                            capture.status = RunStatus::Timeout;
                            capture.message = format!("{} 执行超时", action.name);
                        } else if stop_requested {
                            capture.status = RunStatus::Stopped;
                            capture.message = format!("已停止 {}", action.name);
                        }
                        capture
                    }
                    Err(error) => error_capture(&error.to_string()),
                };
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                service.clear_active_pid(action.id);
                return error_capture("执行线程已断开");
            }
        }

        if !timed_out && action.timeout_sec > 0 {
            let timeout = Duration::from_secs(action.timeout_sec as u64);
            if started.elapsed() >= timeout {
                timed_out = true;
                let _ = signal_process(pid, "TERM");
                signal_deadline = Some(Instant::now() + Duration::from_millis(250));
            }
        }

        if !stop_sent && service.stop_requested(action.id) {
            stop_sent = true;
            let _ = signal_process(pid, "TERM");
            signal_deadline = Some(Instant::now() + Duration::from_millis(250));
        }

        if !forced_kill
            && let Some(deadline) = signal_deadline
            && Instant::now() >= deadline
        {
            forced_kill = true;
            let _ = signal_process(pid, "KILL");
        }

        thread::sleep(Duration::from_millis(25));
    }
}

fn build_script_command(action: &PreparedAction) -> Option<(String, Vec<String>)> {
    match action.script_source {
        ScriptSource::Inline => build_inline_script_command(action),
        ScriptSource::Path => build_path_script_command(action),
    }
}

fn build_inline_script_command(action: &PreparedAction) -> Option<(String, Vec<String>)> {
    if action.script_body.trim().is_empty() {
        return None;
    }

    if !action.interpreter.trim().is_empty() {
        let mut argv = split_shell_words(&action.interpreter).ok()?;
        if argv.is_empty() {
            return None;
        }
        let program = argv.remove(0);
        argv.push(action.script_body.clone());
        argv.extend(action.args.clone());
        return Some((program, argv));
    }

    let (program, flag) = match action.script_type {
        ScriptType::Shell => (String::from("/bin/zsh"), String::from("-lc")),
        ScriptType::Node => (String::from("node"), String::from("-e")),
        ScriptType::Python => (String::from("python3"), String::from("-c")),
        ScriptType::Other => (String::from("/bin/zsh"), String::from("-c")),
    };

    let mut argv = vec![flag, action.script_body.clone()];
    if action.script_type == ScriptType::Shell {
        argv.push(String::from("quick-launch"));
    }
    argv.extend(action.args.clone());
    Some((program, argv))
}

fn build_path_script_command(action: &PreparedAction) -> Option<(String, Vec<String>)> {
    if action.path.trim().is_empty() {
        return None;
    }

    if !action.interpreter.trim().is_empty() {
        let mut argv = split_shell_words(&action.interpreter).ok()?;
        if argv.is_empty() {
            return None;
        }
        let program = argv.remove(0);
        argv.push(action.path.clone());
        argv.extend(action.args.clone());
        return Some((program, argv));
    }

    match action.script_type {
        ScriptType::Shell => Some((
            String::from("/bin/zsh"),
            vec![action.path.clone()]
                .into_iter()
                .chain(action.args.clone())
                .collect(),
        )),
        ScriptType::Node => Some((
            String::from("node"),
            vec![action.path.clone()]
                .into_iter()
                .chain(action.args.clone())
                .collect(),
        )),
        ScriptType::Python => Some((
            String::from("python3"),
            vec![action.path.clone()]
                .into_iter()
                .chain(action.args.clone())
                .collect(),
        )),
        ScriptType::Other => Some((
            String::from("/bin/zsh"),
            vec![action.path.clone()]
                .into_iter()
                .chain(action.args.clone())
                .collect(),
        )),
    }
}

fn capture_from_output(action: &PreparedAction, output: std::process::Output) -> RunCapture {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let exit_code = output.status.code().map(i64::from);
    let message = if output.status.success() {
        format!("已执行 {}", action.name)
    } else if stderr.is_empty() {
        format!("{} 返回码 {}", action.name, exit_code.unwrap_or(-1))
    } else {
        format!("{} 失败: {}", action.name, stderr)
    };

    RunCapture {
        status: if output.status.success() {
            RunStatus::Success
        } else {
            RunStatus::Failed
        },
        exit_code,
        stdout,
        stderr,
        message,
    }
}

fn error_capture(message: &str) -> RunCapture {
    RunCapture {
        status: RunStatus::Error,
        exit_code: None,
        stdout: String::new(),
        stderr: message.to_string(),
        message: format!("执行失败: {message}"),
    }
}

fn signal_process(pid: u32, signal: &str) -> Result<()> {
    #[cfg(unix)]
    {
        let sig = match signal {
            "KILL" => libc::SIGKILL,
            _ => libc::SIGTERM,
        };
        let rc = unsafe { libc::kill(-(pid as i32), sig) };
        if rc != 0 {
            let error = std::io::Error::last_os_error();
            ensure!(false, "发送 {signal} 信号失败: {error}");
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix platforms, try to kill the process directly
        let _ = (pid, signal);
        ensure!(false, "进程信号发送仅在 Unix 平台支持");
    }
    Ok(())
}

fn matches_query(action: &QuickAction, query: &str) -> bool {
    action.name.to_lowercase().contains(query)
        || action.description.to_lowercase().contains(query)
        || action.kind.as_str().contains(query)
        || action.script_type.as_str().contains(query)
        || action.interpreter.to_lowercase().contains(query)
        || action.path.to_lowercase().contains(query)
        || action.url.to_lowercase().contains(query)
        || action.script_body.to_lowercase().contains(query)
        || action
            .args
            .iter()
            .any(|arg| arg.to_lowercase().contains(query))
        || action.env.iter().any(|(key, value)| {
            key.to_lowercase().contains(query) || value.to_lowercase().contains(query)
        })
        || action
            .command_keywords()
            .into_iter()
            .any(|keyword| keyword.to_lowercase().contains(query))
}

fn default_actions() -> Vec<QuickActionDraft> {
    vec![
        seed_script(
            "锁定屏幕",
            "立即锁定 Mac 屏幕",
            r#""/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession" -suspend"#,
            "锁",
            &["锁定", "屏幕", "lock"],
        ),
        seed_script(
            "启动屏保",
            "立即启动屏幕保护程序",
            "open -a ScreenSaverEngine",
            "屏",
            &["屏保", "screensaver"],
        ),
        seed_script(
            "清空废纸篓",
            "安全清空废纸篓",
            r#"osascript -e 'tell application "Finder" to empty trash'"#,
            "废",
            &["废纸篓", "trash"],
        ),
        seed_script(
            "休眠",
            "将 Mac 置于休眠状态",
            "pmset sleepnow",
            "休",
            &["睡眠", "休眠", "sleep"],
        ),
        seed_script(
            "截屏",
            "启动 macOS 截屏工具",
            "open -b com.apple.screenshot",
            "截",
            &["截图", "截屏", "screenshot"],
        ),
        seed_script(
            "打开访达",
            "打开 Finder 文件管理器",
            "open -a Finder",
            "访",
            &["finder", "文件", "访达"],
        ),
        seed_script(
            "打开终端",
            "打开 Terminal.app",
            "open -a Terminal",
            "终",
            &["terminal", "终端"],
        ),
        seed_script(
            "打开浏览器",
            "打开 Safari 浏览器",
            "open -a Safari",
            "网",
            &["browser", "safari", "浏览器"],
        ),
        seed_script(
            "VS Code",
            "打开 Visual Studio Code",
            "open -a 'Visual Studio Code'",
            "C",
            &["code", "vscode", "编辑器"],
        ),
        seed_script(
            "活动监视器",
            "打开活动监视器",
            "open -a 'Activity Monitor'",
            "监",
            &["监视器", "activity", "性能"],
        ),
        seed_script(
            "计算器",
            "打开计算器",
            "open -a Calculator",
            "算",
            &["计算器"],
        ),
        seed_script(
            "备忘录",
            "打开系统备忘录",
            "open -a Notes",
            "备",
            &["备忘录", "notes"],
        ),
    ]
}

fn seed_script(
    name: &str,
    description: &str,
    script_body: &str,
    icon: &str,
    keywords: &[&str],
) -> QuickActionDraft {
    let mut draft = QuickActionDraft::script(name, description, script_body);
    draft.icon = icon.to_string();
    draft.keywords = keywords.iter().map(|value| value.to_string()).collect();
    draft.prefixes = vec![String::from("ql"), String::from("quick")];
    draft.feedback_mode = FeedbackMode::Notification;
    draft
}

fn now_label() -> String {
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&fmt)
        .unwrap_or_else(|_| String::from("1970-01-01 00:00:00"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};

    fn sample_action() -> QuickAction {
        QuickAction {
            id: 1,
            name: String::from("参数动作"),
            description: String::new(),
            kind: ActionKind::Script,
            script_type: ScriptType::Shell,
            script_source: ScriptSource::Inline,
            script_body: String::from("echo ${name}"),
            interpreter: String::new(),
            path: String::new(),
            url: String::new(),
            args: vec![String::from("--team=${team}")],
            cwd: String::from("/tmp/${team}"),
            env: HashMap::from([(String::from("TEAM_NAME"), String::from("${team}"))]),
            keywords: Vec::new(),
            prefixes: Vec::new(),
            icon: String::from("参"),
            feedback_mode: FeedbackMode::Notification,
            timeout_sec: 300,
            enabled: true,
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn temp_db(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-quick-launch-service-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    fn test_service(name: &str) -> Arc<QuickLaunchService> {
        let path = temp_db(name);
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                qingqi_plugin::database::feature_database_key("quick-launch", "actions"),
                path,
            ))
            .unwrap();
        let store = QuickLaunchStore::open(
            database,
            &qingqi_plugin::database::feature_database_key("quick-launch", "actions"),
        )
        .expect("store should open");
        Arc::new(QuickLaunchService {
            store: Mutex::new(store),
            execution: Mutex::new(ExecutionState::default()),
            revision: AtomicU64::new(0),
        })
    }

    #[test]
    fn prepares_action_with_parameter_values() {
        let action = sample_action();
        let values = HashMap::from([
            (String::from("name"), String::from("Jane Doe")),
            (String::from("team"), String::from("infra")),
        ]);

        let prepared = prepare_action(&action, &values).expect("prepare should succeed");
        assert_eq!(prepared.script_body, "echo Jane Doe");
        assert_eq!(prepared.args, vec![String::from("--team=infra")]);
        assert_eq!(prepared.cwd, "/tmp/infra");
        assert_eq!(prepared.env.get("TEAM_NAME"), Some(&String::from("infra")));
    }

    #[test]
    fn rejects_missing_parameters() {
        let action = sample_action();
        let error = prepare_action(&action, &HashMap::new()).expect_err("prepare should fail");
        assert!(error.to_string().contains("缺少参数"));
        assert!(error.to_string().contains("name"));
    }

    #[test]
    #[cfg_attr(target_os = "windows", ignore = "requires Unix shell (/bin/zsh)")]
    fn start_action_updates_running_state_and_records_run() {
        let service = test_service("async.db");
        let draft = QuickActionDraft::script("异步动作", "sleep then echo", "sleep 0.1; echo done");
        let action = {
            let store = service.store.lock().expect("store lock");
            store.create_action(&draft).expect("action should create")
        };

        let started = service
            .start_action(action.id)
            .expect("action should start");
        assert!(started.contains("已开始执行"));
        assert!(
            service
                .runtime_snapshot()
                .running_action_ids
                .contains(&action.id)
        );

        let mut completed = false;
        for _ in 0..40 {
            if !service
                .runtime_snapshot()
                .running_action_ids
                .contains(&action.id)
            {
                completed = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(completed, "background action should finish");

        let snapshot = service.runtime_snapshot();
        assert!(!snapshot.running_action_ids.contains(&action.id));
        assert!(
            snapshot
                .last_event
                .as_ref()
                .map(|event| event.message.contains("已执行"))
                .unwrap_or(false)
        );

        let runs = service.list_runs(action.id, 5).expect("runs should load");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Success);
        assert_eq!(runs[0].stdout, "done");
    }

    #[test]
    #[cfg_attr(target_os = "windows", ignore = "requires Unix shell (/bin/zsh)")]
    fn stop_action_records_stopped_status() {
        let service = test_service("stop.db");
        let draft = QuickActionDraft::script("可停止动作", "sleep long", "sleep 2; echo done");
        let action = {
            let store = service.store.lock().expect("store lock");
            store.create_action(&draft).expect("action should create")
        };

        let _ = service
            .start_action(action.id)
            .expect("action should start");
        thread::sleep(Duration::from_millis(120));
        let stopped = service.stop_action(action.id).expect("stop should succeed");
        assert!(stopped.contains("已请求停止"));

        let mut completed = false;
        for _ in 0..40 {
            if !service
                .runtime_snapshot()
                .running_action_ids
                .contains(&action.id)
            {
                completed = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(completed, "stopped action should finish");

        let runs = service.list_runs(action.id, 5).expect("runs should load");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Stopped);
    }

    #[test]
    #[cfg_attr(target_os = "windows", ignore = "requires Unix shell (/bin/zsh)")]
    fn timeout_records_timeout_status() {
        let service = test_service("timeout.db");
        let mut draft =
            QuickActionDraft::script("超时动作", "sleep too long", "sleep 2; echo late");
        draft.timeout_sec = 1;
        let action = {
            let store = service.store.lock().expect("store lock");
            store.create_action(&draft).expect("action should create")
        };

        let _ = service
            .start_action(action.id)
            .expect("action should start");

        let mut completed = false;
        for _ in 0..80 {
            if !service
                .runtime_snapshot()
                .running_action_ids
                .contains(&action.id)
            {
                completed = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(completed, "timed out action should finish");

        let runs = service.list_runs(action.id, 5).expect("runs should load");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Timeout);
    }

    #[test]
    fn management_crud_updates_store_and_revision() {
        let service = test_service("manage.db");

        let created = service
            .create_action(QuickActionDraft::script("创建动作", "demo", "echo first"))
            .expect("action should create");
        assert_eq!(created.name, "创建动作");
        assert!(
            service
                .runtime_snapshot()
                .last_event
                .as_ref()
                .map(|event| event.message.contains("已创建动作"))
                .unwrap_or(false)
        );

        let mut updated_draft = action_to_draft(&created);
        updated_draft.name = String::from("更新动作");
        updated_draft.script_body = String::from("echo second");
        let updated = service
            .update_action(created.id, updated_draft)
            .expect("action should update");
        assert_eq!(updated.name, "更新动作");

        let disabled = service
            .set_action_enabled(created.id, false)
            .expect("action should disable");
        assert!(!disabled.enabled);
        assert!(
            service
                .list_actions("", Some(true))
                .expect("enabled actions should load")
                .into_iter()
                .all(|action| action.id != created.id)
        );

        let duplicated = service
            .duplicate_action(created.id)
            .expect("action should duplicate");
        assert!(duplicated.name.contains("副本"));

        let deleted = service
            .delete_action(duplicated.id)
            .expect("action should delete");
        assert!(deleted.contains("已删除动作"));
        assert!(
            service
                .list_actions("", None)
                .expect("actions should load")
                .into_iter()
                .all(|action| action.id != duplicated.id)
        );
    }

    #[test]
    fn create_action_rejects_invalid_draft() {
        let service = test_service("invalid.db");
        let draft = QuickActionDraft::script("", "demo", "");
        let error = service
            .create_action(draft)
            .expect_err("invalid action should fail");
        assert!(error.to_string().contains("动作名称不能为空"));
    }

    #[test]
    fn builds_inline_node_command_with_override_interpreter() {
        let mut action = sample_action();
        action.script_type = ScriptType::Node;
        action.interpreter = String::from("/usr/bin/env node --no-warnings");
        action.script_body = String::from("console.log('hi')");
        action.args = vec![String::from("--trace-warnings")];
        action.env.clear();
        action.cwd.clear();

        let prepared = prepare_action(&action, &HashMap::new()).expect("prepare should succeed");
        let (program, args) = build_script_command(&prepared).expect("command should build");
        assert_eq!(program, "/usr/bin/env");
        assert_eq!(
            args,
            vec![
                String::from("node"),
                String::from("--no-warnings"),
                String::from("console.log('hi')"),
                String::from("--trace-warnings"),
            ]
        );
    }

    #[test]
    fn builds_path_python_command_with_env() {
        let mut action = sample_action();
        action.script_type = ScriptType::Python;
        action.script_source = ScriptSource::Path;
        action.script_body.clear();
        action.path = String::from("/tmp/demo.py");
        action.args = vec![String::from("--fast")];
        action.env = HashMap::from([(String::from("PROFILE"), String::from("${team}"))]);

        let prepared = prepare_action(
            &action,
            &HashMap::from([(String::from("team"), String::from("prod"))]),
        )
        .expect("prepare should succeed");
        let (program, args) = build_script_command(&prepared).expect("command should build");
        assert_eq!(program, "python3");
        assert_eq!(
            args,
            vec![String::from("/tmp/demo.py"), String::from("--fast")]
        );
        assert_eq!(prepared.env.get("PROFILE"), Some(&String::from("prod")));
    }

    #[test]
    fn latest_runs_returns_most_recent_run_per_action() {
        let service = test_service("latest_runs.db");
        let a1 = {
            let store = service.store.lock().expect("store lock");
            store
                .create_action(&QuickActionDraft::script("动作一", "desc", "echo one"))
                .expect("action should create")
        };
        let a2 = {
            let store = service.store.lock().expect("store lock");
            store
                .create_action(&QuickActionDraft::script("动作二", "desc", "echo two"))
                .expect("action should create")
        };

        // Record runs via store directly to avoid spawning threads
        {
            let store = service.store.lock().expect("store lock");
            store
                .record_run(&QuickRunDraft {
                    action_id: a1.id,
                    status: RunStatus::Success,
                    exit_code: Some(0),
                    stdout: String::from("ok"),
                    stderr: String::new(),
                    duration_ms: 12,
                    started_at: String::from("2026-05-28 10:00:00"),
                    finished_at: String::from("2026-05-28 10:00:01"),
                    message: String::from("已执行"),
                })
                .expect("run should record");
            store
                .record_run(&QuickRunDraft {
                    action_id: a1.id,
                    status: RunStatus::Failed,
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: String::from("error"),
                    duration_ms: 34,
                    started_at: String::from("2026-05-28 10:01:00"),
                    finished_at: String::from("2026-05-28 10:01:01"),
                    message: String::from("失败"),
                })
                .expect("run should record");
        }

        let latest = service
            .latest_runs(&[a1.id, a2.id])
            .expect("latest runs should load");
        // a1 has runs, a2 has none
        assert_eq!(latest.len(), 1);
        let run = latest.get(&a1.id).expect("a1 should have latest run");
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.stderr, "error");
        assert_eq!(run.duration_ms, 34);
    }

    #[test]
    fn run_summary_chip_label_per_status() {
        let summary = |status, exit_code, duration_ms| RunSummary {
            status,
            exit_code,
            duration_ms,
        };

        assert!(
            summary(RunStatus::Success, Some(0), 12)
                .chip_label()
                .contains("上次成功")
        );
        assert!(
            summary(RunStatus::Success, Some(0), 12)
                .chip_label()
                .contains("12ms")
        );

        assert!(
            summary(RunStatus::Failed, Some(1), 34)
                .chip_label()
                .contains("上次失败")
        );
        assert!(
            summary(RunStatus::Failed, Some(1), 34)
                .chip_label()
                .contains("exit 1")
        );
        assert!(
            summary(RunStatus::Failed, Some(1), 34)
                .chip_label()
                .contains("34ms")
        );

        assert!(
            summary(RunStatus::Timeout, None, 5000)
                .chip_label()
                .contains("上次超时")
        );

        assert!(
            summary(RunStatus::Stopped, None, 42)
                .chip_label()
                .contains("已停止")
        );

        assert!(
            summary(RunStatus::Error, None, 0)
                .chip_label()
                .contains("上次出错")
        );
    }

    #[test]
    fn run_summary_chip_label_failed_no_exit_code() {
        let summary = RunSummary {
            status: RunStatus::Failed,
            exit_code: None,
            duration_ms: 100,
        };
        let label = summary.chip_label();
        assert!(label.contains("上次失败"));
        assert!(label.contains("100ms"));
        assert!(!label.contains("exit"));
    }

    #[test]
    fn latest_run_summaries_returns_summary_map() {
        let service = test_service("summaries.db");
        let a1 = {
            let store = service.store.lock().expect("store lock");
            store
                .create_action(&QuickActionDraft::script("A1", "desc", "echo one"))
                .expect("action should create")
        };
        let a2 = {
            let store = service.store.lock().expect("store lock");
            store
                .create_action(&QuickActionDraft::script("A2", "desc", "echo two"))
                .expect("action should create")
        };

        {
            let store = service.store.lock().expect("store lock");
            store
                .record_run(&QuickRunDraft {
                    action_id: a1.id,
                    status: RunStatus::Success,
                    exit_code: Some(0),
                    stdout: String::from("ok"),
                    stderr: String::new(),
                    duration_ms: 12,
                    started_at: String::from("2026-05-28 10:00:00"),
                    finished_at: String::from("2026-05-28 10:00:01"),
                    message: String::from("done"),
                })
                .expect("run should record");
        }

        let summaries = service
            .latest_run_summaries(&[a1.id, a2.id])
            .expect("summaries should load");
        assert_eq!(summaries.len(), 1);
        let s = summaries.get(&a1.id).expect("a1 should have summary");
        assert_eq!(s.status, RunStatus::Success);
        assert_eq!(s.exit_code, Some(0));
        assert_eq!(s.duration_ms, 12);
        assert!(s.chip_label().contains("上次成功"));
    }
}
