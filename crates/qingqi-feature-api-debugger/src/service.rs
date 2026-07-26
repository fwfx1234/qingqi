use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread,
    time::Instant,
};

use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use crate::{
    data_source::ApiDebuggerDataSource,
    model::{ApiVariable, CollectionNode, NodeKind, RequestSnapshot, VariableScope},
    script_service,
    store::ApiWorkspace,
    variable_service,
};
use qingqi_plugin::{database::DatabaseService, log_error, storage::AppPaths};
#[cfg(not(test))]
use qingqi_plugin::dict_store::PluginDictStore;

// Re-export model types for backward compatibility
pub use crate::model::{
    ApiEnvironment, ApiGroup, ApiRequest, ApiScenario, AuthType, BodyMode, EnvHeader, EnvVariable,
    EnvironmentFull, HttpHistory, HttpMethod, KeyValueRow, RequestBody,
};

#[cfg(not(test))]
const UI_PREFS_NAMESPACE: &str = "api_debugger.ui";
#[cfg(not(test))]
const PREF_REQUEST_PANEL_RATIO: &str = "request_panel_ratio";
#[cfg(not(test))]
const PREF_RESPONSE_TAB: &str = "response_tab";

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct EnvironmentExport {
    pub version: u8,
    pub environments: Vec<ApiEnvironment>,
}

// ── UI-specific enums (not persisted) ──

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTab {
    Params,
    Path,
    Body,
    Headers,
    Cookies,
    Auth,
    PreOps,
    PostOps,
}

impl EditorTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Params => "Params",
            Self::Path => "Path",
            Self::Body => "Body",
            Self::Headers => "Headers",
            Self::Cookies => "Cookies",
            Self::Auth => "Auth",
            Self::PreOps => "Pre-ops",
            Self::PostOps => "Post-ops",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Params => "params",
            Self::Path => "path",
            Self::Body => "body",
            Self::Headers => "headers",
            Self::Cookies => "cookies",
            Self::Auth => "auth",
            Self::PreOps => "pre_ops",
            Self::PostOps => "post_ops",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "path" => Self::Path,
            "body" => Self::Body,
            "headers" => Self::Headers,
            "cookies" => Self::Cookies,
            "auth" => Self::Auth,
            "pre_ops" => Self::PreOps,
            "post_ops" => Self::PostOps,
            _ => Self::Params,
        }
    }
}

pub fn editor_tab_index(tab: EditorTab) -> i64 {
    match tab {
        EditorTab::Params => 0,
        EditorTab::Path => 1,
        EditorTab::Body => 2,
        EditorTab::Headers => 3,
        EditorTab::Cookies => 4,
        EditorTab::Auth => 5,
        EditorTab::PreOps => 6,
        EditorTab::PostOps => 7,
    }
}

pub fn index_to_editor_tab(index: i64) -> Option<EditorTab> {
    match index {
        0 => Some(EditorTab::Params),
        1 => Some(EditorTab::Path),
        2 => Some(EditorTab::Body),
        3 => Some(EditorTab::Headers),
        4 => Some(EditorTab::Cookies),
        5 => Some(EditorTab::Auth),
        6 => Some(EditorTab::PreOps),
        7 => Some(EditorTab::PostOps),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseTab {
    Body,
    Cookies,
    Headers,
    Request,
    Curl,
    Logs,
    History,
    Code,
}

impl ResponseTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Body => "Body",
            Self::Cookies => "Cookies",
            Self::Headers => "Headers",
            Self::Request => "Request",
            Self::Curl => "cURL",
            Self::Logs => "日志",
            Self::History => "历史",
            Self::Code => "代码",
        }
    }

    pub fn all() -> [Self; 8] {
        [
            Self::Body,
            Self::Cookies,
            Self::Headers,
            Self::Request,
            Self::Curl,
            Self::Logs,
            Self::History,
            Self::Code,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Cookies => "cookies",
            Self::Headers => "headers",
            Self::Request => "request",
            Self::Curl => "curl",
            Self::Logs => "logs",
            Self::History => "history",
            Self::Code => "code",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "cookies" => Self::Cookies,
            "headers" => Self::Headers,
            "request" => Self::Request,
            "curl" => Self::Curl,
            "logs" => Self::Logs,
            "history" => Self::History,
            "code" => Self::Code,
            _ => Self::Body,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvDetailTab {
    Variables,
    Headers,
}

impl EnvDetailTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Variables => "变量",
            Self::Headers => "公共 Headers",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ApiResponse {
    pub status_line: String,
    pub status_code: u16,
    pub duration_ms: u128,
    pub size_bytes: usize,
    pub body: String,
    pub headers: String,
    /// `Set-Cookie` response headers, one per line (empty when none).
    pub cookies: String,
    /// Raw `Content-Type` of the response (used to flag binary/image bodies).
    pub content_type: String,
    pub request_dump: String,
    pub curl: String,
    pub logs: Vec<String>,
    pub assertion_results: Vec<(String, bool)>,
    /// Raw response bytes for binary responses (images, PDFs, etc.).
    /// When `Some`, `body` is a lossy UTF-8 preview and `body_bytes` holds the
    /// original data for accurate save / image preview.
    pub body_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
struct ApiServiceState {
    in_flight: bool,
    pending_response: Option<ApiResponse>,
    pending_error: Option<String>,
    pending_notice: Option<String>,
    pending_groups: Option<Vec<ApiGroup>>,
    pending_environments: Option<Vec<ApiEnvironment>>,
}

pub struct ApiService {
    /// Monotonic request generation. Each send claims the next value; a cancel
    /// (or a superseding send) advances it so the in-flight worker discards its
    /// result instead of clobbering newer state.
    generation: AtomicU64,
    state: Mutex<ApiServiceState>,
    update_subscribers: Mutex<Vec<Sender<()>>>,
    data_source: ApiDebuggerDataSource,
    #[cfg(not(test))]
    preferences: PluginDictStore,
}

impl ApiService {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> Self {
        let _ = paths;
        #[cfg(not(test))]
        let preferences =
            PluginDictStore::for_database(database.as_ref().clone(), "plugin-dict.db");
        let data_source = ApiDebuggerDataSource::open(database, "api_debugger/main")
            .expect("无法打开 API 调试器数据库");
        Self {
            generation: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                pending_groups: None,
                pending_environments: None,
            }),
            update_subscribers: Mutex::new(Vec::new()),
            data_source,
            #[cfg(not(test))]
            preferences,
        }
    }

    pub fn load_request_panel_ratio(&self) -> f32 {
        #[cfg(test)]
        return 0.4;
        #[cfg(not(test))]
        self.preferences
            .get_u64(UI_PREFS_NAMESPACE, PREF_REQUEST_PANEL_RATIO)
            .ok()
            .flatten()
            .map(|value| (value.clamp(2000, 8000) as f32) / 10_000.0)
            .unwrap_or(0.4)
    }

    pub fn save_request_panel_ratio(&self, ratio: f32) {
        #[cfg(test)]
        let _ = ratio;
        #[cfg(not(test))]
        {
        let value = (ratio.clamp(0.2, 0.8) * 10_000.0).round() as u64;
        if let Err(error) = self.preferences.set_u64(
            UI_PREFS_NAMESPACE,
            PREF_REQUEST_PANEL_RATIO,
            value,
        ) {
            tracing::warn!("保存 API 调试器分栏比例失败: {error}");
        }
        }
    }

    pub fn load_response_tab(&self) -> ResponseTab {
        #[cfg(test)]
        return ResponseTab::Body;
        #[cfg(not(test))]
        self.preferences
            .get_string(UI_PREFS_NAMESPACE, PREF_RESPONSE_TAB)
            .ok()
            .flatten()
            .map(|value| ResponseTab::from_db(&value))
            .unwrap_or(ResponseTab::Body)
    }

    pub fn save_response_tab(&self, tab: ResponseTab) {
        #[cfg(test)]
        let _ = tab;
        #[cfg(not(test))]
        {
        if let Err(error) = self.preferences.set_string(
            UI_PREFS_NAMESPACE,
            PREF_RESPONSE_TAB,
            tab.as_str(),
        ) {
            tracing::warn!("保存 API 调试器响应标签失败: {error}");
        }
        }
    }

    pub fn subscribe_updates(&self) -> Receiver<()> {
        let (sender, receiver) = channel();
        if let Ok(mut subscribers) = self.update_subscribers.lock() {
            subscribers.push(sender);
        }
        receiver
    }

    fn notify_updated(&self) {
        if let Ok(mut subscribers) = self.update_subscribers.lock() {
            subscribers.retain(|subscriber| subscriber.send(()).is_ok());
        }
    }

    pub fn is_in_flight(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.in_flight)
            .unwrap_or(false)
    }

    pub fn cancel_request(&self) {
        // Invalidate the in-flight request so its result is discarded, and
        // unlock the UI immediately. A blocking request already sent cannot be
        // aborted mid-flight, so the server may still process it — we only stop
        // waiting for / applying the response.
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut state) = self.state.lock() {
            state.in_flight = false;
            state.pending_notice = Some(String::from("请求已取消"));
        }
        self.notify_updated();
    }

    pub fn take_pending_response(&self) -> Option<ApiResponse> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.pending_response.take())
    }

    pub fn take_pending_error(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.pending_error.take())
    }

    pub fn take_pending_notice(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.pending_notice.take())
    }

    pub fn take_pending_groups(&self) -> Option<Vec<ApiGroup>> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.pending_groups.take())
    }

    pub fn take_pending_environments(&self) -> Option<Vec<ApiEnvironment>> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.pending_environments.take())
    }

    fn reload_workspace_with_notice(&self, notice: String) {
        let groups = self.build_collection_tree().unwrap_or_default();
        let environments = self.list_environments_ui();
        if let Ok(mut state) = self.state.lock() {
            state.pending_groups = Some(groups);
            state.pending_environments = Some(environments);
            state.pending_notice = Some(notice);
        }
        self.notify_updated();
    }

    fn publish_notice(&self, notice: String) {
        if let Ok(mut state) = self.state.lock() {
            state.pending_notice = Some(notice);
        }
        self.notify_updated();
    }

    pub fn load_workspace(&self) -> Result<ApiWorkspace> {
        let groups = self.build_collection_tree()?;
        let environments = self.list_environments_ui();
        Ok(ApiWorkspace::new(groups, environments))
    }

    pub fn list_environments_ui(&self) -> Vec<ApiEnvironment> {
        match self.data_source.list_environments() {
            Ok(envs_full) if !envs_full.is_empty() => {
                envs_full.iter().map(env_full_to_ui).collect()
            }
            _ => default_environments(),
        }
    }

    pub fn list_global_variables(&self) -> Vec<KeyValueRow> {
        self.data_source
            .list_variables(VariableScope::Global, "")
            .unwrap_or_default()
            .into_iter()
            .map(|variable| KeyValueRow::new(variable.var_key, variable.var_value))
            .collect()
    }

    pub fn save_global_variables(&self, rows: &[KeyValueRow]) -> Result<()> {
        let desired = rows
            .iter()
            .filter_map(|row| {
                let key = row.key.trim();
                (!key.is_empty()).then(|| (key.to_string(), row.value.clone()))
            })
            .collect::<HashMap<_, _>>();

        for (key, value) in &desired {
            self.data_source
                .upsert_variable(VariableScope::Global, "", key, value)?;
        }
        for variable in self.data_source.list_variables(VariableScope::Global, "")? {
            if !desired.contains_key(&variable.var_key) {
                self.data_source
                    .delete_variable(VariableScope::Global, "", &variable.var_key)?;
            }
        }
        Ok(())
    }

    pub fn save_global_variables_async(self: &Arc<Self>, rows: Vec<KeyValueRow>) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.save_global_variables(&rows) {
            Ok(()) => service.publish_notice(String::from("已保存全局变量")),
            Err(error) => service.publish_notice(format!("保存全局变量失败: {error}")),
        });
    }

    pub fn export_environments_json(&self) -> Result<String> {
        let export = EnvironmentExport {
            version: 1,
            environments: self.list_environments_ui(),
        };
        Ok(serde_json::to_string_pretty(&export)?)
    }

    pub fn import_environments_json(&self, content: &str) -> Result<usize> {
        let environments: Vec<ApiEnvironment> =
            if let Ok(export) = serde_json::from_str::<EnvironmentExport>(content) {
                export.environments
            } else {
                serde_json::from_str(content)?
            };
        if environments.is_empty() {
            bail!("导入文件中没有环境");
        }
        let envs_full = environments
            .iter()
            .enumerate()
            .map(|(index, env)| {
                let suffix = Uuid::new_v4().simple();
                env_ui_to_full(env, &format!("env-import-{index}-{suffix}"))
            })
            .collect::<Vec<_>>();
        self.data_source.save_environments_full(&envs_full)?;
        Ok(environments.len())
    }

    pub fn persist_endpoint_snapshot(
        &self,
        title: &str,
        method: &str,
        url: &str,
        request: &ApiRequest,
    ) -> Result<()> {
        let snapshot = request_to_snapshot(method, url, request);
        // Prefer the stable node_id so endpoints that share a title don't clobber
        // each other; fall back to title matching only for legacy requests that
        // have not been assigned a node_id yet.
        if !request.node_id.is_empty() {
            self.data_source.update_collection_node(
                &request.node_id,
                title,
                method,
                url,
                &snapshot,
            )?;
            return Ok(());
        }
        let nodes = self.data_source.list_collection_nodes()?;
        if let Some(node) = nodes
            .iter()
            .find(|n| n.name == title && n.kind == NodeKind::Endpoint)
        {
            self.data_source
                .update_collection_node(&node.id, title, method, url, &snapshot)?;
        }
        Ok(())
    }

    fn build_collection_tree(&self) -> Result<Vec<ApiGroup>> {
        let nodes = self.data_source.list_collection_nodes()?;
        Ok(build_groups_from_nodes(&nodes))
    }

    pub fn save_workspace(
        &self,
        _groups: &[ApiGroup],
        environments: &[ApiEnvironment],
    ) -> Result<()> {
        let envs_full: Vec<EnvironmentFull> = environments
            .iter()
            .enumerate()
            .map(|(i, env)| env_ui_to_full(env, &format!("env-{i}")))
            .collect();
        self.data_source.save_environments_full(&envs_full)?;
        Ok(())
    }

    pub fn save_workspace_async(self: &Arc<Self>, environments: Vec<ApiEnvironment>) {
        let service = Arc::clone(self);
        thread::spawn(move || {
            if let Err(error) = service.save_workspace(&[], &environments) {
                service.publish_notice(format!("工作区保存失败: {error}"));
            } else {
                service.reload_workspace_with_notice(String::from("工作区已保存"));
            }
        });
    }

    // ── Collection CRUD ──

    pub fn create_endpoint(
        &self,
        parent_id: Option<&str>,
        name: &str,
        method: &str,
        url: &str,
    ) -> Result<CollectionNode> {
        let id = format!("node-{}", Uuid::new_v4().simple());
        let snapshot = RequestSnapshot {
            method: method.to_string(),
            url: url.to_string(),
            ..Default::default()
        };
        let node = self.data_source.create_collection_node(
            &id,
            parent_id,
            NodeKind::Endpoint,
            name,
            method,
            url,
            &snapshot,
        )?;
        Ok(node)
    }

    pub fn create_endpoint_async(
        self: &Arc<Self>,
        parent_id: Option<String>,
        name: String,
        method: String,
        url: String,
    ) {
        let service = Arc::clone(self);
        thread::spawn(move || {
            match service.create_endpoint(parent_id.as_deref(), &name, &method, &url) {
                Ok(_) => service.reload_workspace_with_notice(format!("已创建端点 {}", name)),
                Err(e) => service.publish_notice(format!("创建端点失败: {e}")),
            }
        });
    }

    /// Create a test case as a child of an endpoint. Cases surface as scenarios
    /// under their parent request in the collection tree.
    pub fn create_case(&self, parent_id: &str, name: &str) -> Result<CollectionNode> {
        let parent = self
            .data_source
            .get_collection_node(parent_id)?
            .ok_or_else(|| anyhow!("父端点不存在"))?;
        if parent.kind != NodeKind::Endpoint {
            bail!("用例只能创建在端点下");
        }
        let parent_snapshot = parent.request.clone();
        let id = format!("case-{}", Uuid::new_v4().simple());
        let node = self.data_source.create_collection_node(
            &id,
            Some(parent_id),
            NodeKind::Case,
            name,
            &parent.method,
            &parent.url,
            &parent_snapshot,
        )?;
        Ok(node)
    }

    pub fn create_case_async(self: &Arc<Self>, parent_id: String, name: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.create_case(&parent_id, &name) {
            Ok(_) => service.reload_workspace_with_notice(format!("已创建用例 {}", name)),
            Err(e) => service.publish_notice(format!("创建用例失败: {e}")),
        });
    }

    pub fn create_folder(&self, parent_id: Option<&str>, name: &str) -> Result<CollectionNode> {
        let id = format!("folder-{}", Uuid::new_v4().simple());
        let node = self.data_source.create_collection_node(
            &id,
            parent_id,
            NodeKind::Folder,
            name,
            "",
            "",
            &RequestSnapshot::default(),
        )?;
        Ok(node)
    }

    pub fn create_folder_async(self: &Arc<Self>, parent_id: Option<String>, name: String) {
        let service = Arc::clone(self);
        thread::spawn(
            move || match service.create_folder(parent_id.as_deref(), &name) {
                Ok(_) => service.reload_workspace_with_notice(format!("已创建分组 {}", name)),
                Err(e) => service.publish_notice(format!("创建分组失败: {e}")),
            },
        );
    }

    pub fn delete_collection_item(&self, node_id: &str) -> Result<usize> {
        let count = self.data_source.delete_collection_node_recursive(node_id)?;
        Ok(count)
    }

    pub fn delete_collection_item_async(self: &Arc<Self>, node_id: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.delete_collection_item(&node_id) {
            Ok(count) => service.reload_workspace_with_notice(format!("已删除 {} 项", count)),
            Err(e) => service.publish_notice(format!("删除失败: {e}")),
        });
    }

    pub fn rename_collection_item(&self, node_id: &str, new_name: &str) -> Result<()> {
        let operation_id = Uuid::new_v4().simple().to_string();
        self.rename_collection_item_traced(node_id, new_name, &operation_id, Instant::now())
    }

    fn rename_collection_item_traced(
        &self,
        node_id: &str,
        new_name: &str,
        operation_id: &str,
        operation_started: Instant,
    ) -> Result<()> {
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            new_name_len = new_name.chars().count(),
            queue_duration_ms = operation_started.elapsed().as_millis(),
            step = "rename_started",
            "API 调试器重命名：开始执行"
        );

        let step_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step = "load_node_started",
            "API 调试器重命名：开始读取节点"
        );
        let node = match self.data_source.get_collection_node(node_id) {
            Ok(Some(node)) => {
                tracing::info!(
                    target: "qingqi::api_debugger::rename",
                    operation_id,
                    node_id,
                    node_kind = ?node.kind,
                    step_duration_ms = step_started.elapsed().as_millis(),
                    total_duration_ms = operation_started.elapsed().as_millis(),
                    step = "load_node_completed",
                    "API 调试器重命名：节点读取完成"
                );
                node
            }
            Ok(None) => {
                tracing::warn!(
                    target: "qingqi::api_debugger::rename",
                    operation_id,
                    node_id,
                    step_duration_ms = step_started.elapsed().as_millis(),
                    total_duration_ms = operation_started.elapsed().as_millis(),
                    step = "load_node_failed",
                    "API 调试器重命名：节点不存在"
                );
                return Err(anyhow!("节点不存在"));
            }
            Err(error) => {
                tracing::warn!(
                    target: "qingqi::api_debugger::rename",
                    operation_id,
                    node_id,
                    error = %error,
                    step_duration_ms = step_started.elapsed().as_millis(),
                    total_duration_ms = operation_started.elapsed().as_millis(),
                    step = "load_node_failed",
                    "API 调试器重命名：读取节点失败"
                );
                return Err(error);
            }
        };

        let step_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step = "clone_snapshot_started",
            "API 调试器重命名：开始复制请求快照"
        );
        let snapshot = node.request.clone();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step_duration_ms = step_started.elapsed().as_millis(),
            total_duration_ms = operation_started.elapsed().as_millis(),
            step = "clone_snapshot_completed",
            "API 调试器重命名：请求快照复制完成"
        );

        let step_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step = "database_update_started",
            "API 调试器重命名：开始更新数据库"
        );
        if let Err(error) = self.data_source.update_collection_node(
            node_id,
            new_name,
            &node.method,
            &node.url,
            &snapshot,
        ) {
            tracing::warn!(
                target: "qingqi::api_debugger::rename",
                operation_id,
                node_id,
                error = %error,
                step_duration_ms = step_started.elapsed().as_millis(),
                total_duration_ms = operation_started.elapsed().as_millis(),
                step = "database_update_failed",
                "API 调试器重命名：数据库更新失败"
            );
            return Err(error);
        }
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step_duration_ms = step_started.elapsed().as_millis(),
            total_duration_ms = operation_started.elapsed().as_millis(),
            step = "database_update_completed",
            "API 调试器重命名：数据库更新完成"
        );

        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            total_duration_ms = operation_started.elapsed().as_millis(),
            step = "rename_persisted",
            "API 调试器重命名：持久化阶段完成"
        );
        Ok(())
    }

    pub fn rename_collection_item_async(self: &Arc<Self>, node_id: String, new_name: String) {
        let service = Arc::clone(self);
        let operation_id = Uuid::new_v4().simple().to_string();
        let operation_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            new_name_len = new_name.chars().count(),
            step = "worker_queued",
            "API 调试器重命名：后台任务已排队"
        );
        let worker_operation_id = operation_id.clone();
        let worker_node_id = node_id.clone();
        let worker_new_name = new_name.clone();
        let spawn_started = Instant::now();
        match thread::Builder::new()
            .name(String::from("api-debugger-rename"))
            .spawn(move || {
                tracing::info!(
                    target: "qingqi::api_debugger::rename",
                    operation_id = worker_operation_id,
                    node_id = worker_node_id,
                    queue_duration_ms = operation_started.elapsed().as_millis(),
                    step = "worker_started",
                    "API 调试器重命名：后台任务开始执行"
                );
                match service.rename_collection_item_traced(
                    &worker_node_id,
                    &worker_new_name,
                    &worker_operation_id,
                    operation_started,
                ) {
                    Ok(()) => service.reload_workspace_after_rename(
                        format!("已重命名为 {}", worker_new_name),
                        &worker_operation_id,
                        &worker_node_id,
                        operation_started,
                    ),
                    Err(error) => {
                        tracing::warn!(
                            target: "qingqi::api_debugger::rename",
                            operation_id = worker_operation_id,
                            node_id = worker_node_id,
                            error = %error,
                            total_duration_ms = operation_started.elapsed().as_millis(),
                            step = "rename_failed",
                            "API 调试器重命名：执行失败，开始发布错误通知"
                        );
                        let step_started = Instant::now();
                        service.publish_notice(format!("重命名失败: {error}"));
                        tracing::info!(
                            target: "qingqi::api_debugger::rename",
                            operation_id = worker_operation_id,
                            node_id = worker_node_id,
                            step_duration_ms = step_started.elapsed().as_millis(),
                            total_duration_ms = operation_started.elapsed().as_millis(),
                            step = "failure_notice_published",
                            "API 调试器重命名：错误通知发布完成"
                        );
                    }
                }
            }) {
            Ok(_) => tracing::info!(
                target: "qingqi::api_debugger::rename",
                operation_id,
                spawn_duration_ms = spawn_started.elapsed().as_millis(),
                step = "worker_spawned",
                "API 调试器重命名：后台线程创建完成"
            ),
            Err(error) => {
                tracing::warn!(
                    target: "qingqi::api_debugger::rename",
                    operation_id,
                    node_id,
                    error = %error,
                    spawn_duration_ms = spawn_started.elapsed().as_millis(),
                    total_duration_ms = operation_started.elapsed().as_millis(),
                    step = "worker_spawn_failed",
                    "API 调试器重命名：后台线程创建失败"
                );
                self.publish_notice(format!("重命名失败: 无法创建后台任务: {error}"));
            }
        }
    }

    fn reload_workspace_after_rename(
        &self,
        notice: String,
        operation_id: &str,
        node_id: &str,
        operation_started: Instant,
    ) {
        let step_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step = "collection_tree_reload_started",
            "API 调试器重命名：开始重载集合树"
        );
        let groups = match self.build_collection_tree() {
            Ok(groups) => {
                tracing::info!(
                    target: "qingqi::api_debugger::rename",
                    operation_id,
                    node_id,
                    group_count = groups.len(),
                    step_duration_ms = step_started.elapsed().as_millis(),
                    total_duration_ms = operation_started.elapsed().as_millis(),
                    step = "collection_tree_reload_completed",
                    "API 调试器重命名：集合树重载完成"
                );
                groups
            }
            Err(error) => {
                tracing::warn!(
                    target: "qingqi::api_debugger::rename",
                    operation_id,
                    node_id,
                    error = %error,
                    step_duration_ms = step_started.elapsed().as_millis(),
                    total_duration_ms = operation_started.elapsed().as_millis(),
                    step = "collection_tree_reload_failed",
                    "API 调试器重命名：集合树重载失败，使用空列表"
                );
                Vec::new()
            }
        };

        let step_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step = "environments_reload_started",
            "API 调试器重命名：开始重载环境列表"
        );
        let environments = self.list_environments_ui();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            environment_count = environments.len(),
            step_duration_ms = step_started.elapsed().as_millis(),
            total_duration_ms = operation_started.elapsed().as_millis(),
            step = "environments_reload_completed",
            "API 调试器重命名：环境列表重载完成"
        );

        let step_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step = "state_lock_wait_started",
            "API 调试器重命名：开始等待状态锁"
        );
        match self.state.lock() {
            Ok(mut state) => {
                let lock_wait_duration_ms = step_started.elapsed().as_millis();
                state.pending_groups = Some(groups);
                state.pending_environments = Some(environments);
                state.pending_notice = Some(notice);
                tracing::info!(
                    target: "qingqi::api_debugger::rename",
                    operation_id,
                    node_id,
                    lock_wait_duration_ms,
                    step_duration_ms = step_started.elapsed().as_millis(),
                    total_duration_ms = operation_started.elapsed().as_millis(),
                    step = "state_published",
                    "API 调试器重命名：工作区状态发布完成"
                );
            }
            Err(error) => tracing::warn!(
                target: "qingqi::api_debugger::rename",
                operation_id,
                node_id,
                error = %error,
                step_duration_ms = step_started.elapsed().as_millis(),
                total_duration_ms = operation_started.elapsed().as_millis(),
                step = "state_publish_failed",
                "API 调试器重命名：状态锁已损坏，工作区状态发布失败"
            ),
        }

        let notify_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            total_duration_ms = operation_started.elapsed().as_millis(),
            step = "render_event_publish_started",
            "API 调试器重命名：开始发布状态变更事件"
        );
        self.notify_updated();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            step_duration_ms = notify_started.elapsed().as_millis(),
            total_duration_ms = operation_started.elapsed().as_millis(),
            step = "render_event_publish_completed",
            "API 调试器重命名：状态变更事件发布完成"
        );
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            operation_id,
            node_id,
            total_duration_ms = operation_started.elapsed().as_millis(),
            step = "rename_completed",
            "API 调试器重命名：全部步骤完成"
        );
    }

    // ── Import ──

    pub fn import_from_curl(&self, curl_text: &str) -> Result<CollectionNode> {
        let parsed =
            crate::curl_parser::parse_curl(curl_text).map_err(|e| anyhow!("cURL 解析失败: {e}"))?;
        let (url, query_rows) = split_request_url(&parsed.url);
        let path_rows = extract_path_parameter_names(&url)
            .into_iter()
            .map(|name| KeyValueRow::new(name, ""))
            .collect::<Vec<_>>();
        let mut body_payloads = RequestBody::migrate_legacy(parsed.body_mode, &parsed.body);
        if !parsed.form_data.is_empty() {
            body_payloads.form_data = parsed.form_data.clone();
        }
        let id = format!("node-{}", Uuid::new_v4().simple());
        let snapshot = RequestSnapshot {
            method: parsed.method.clone(),
            url: url.clone(),
            params_text: format_kv_rows(&query_rows),
            path_params_text: format_kv_rows(&path_rows),
            headers_text: parsed
                .headers
                .iter()
                .map(|h| format!("{}={}", h.key, h.value))
                .collect::<Vec<_>>()
                .join("\n"),
            body_text: parsed.body.clone(),
            body_mode: parsed.body_mode.as_str().to_string(),
            body_payloads,
            auth_type: parsed.auth_type.clone(),
            auth_value: parsed.auth_value.clone(),
            ..Default::default()
        };
        self.data_source.create_collection_node(
            &id,
            None,
            NodeKind::Endpoint,
            &parsed.url,
            &parsed.method,
            &url,
            &snapshot,
        )
    }

    pub fn import_from_curl_async(self: &Arc<Self>, curl_text: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.import_from_curl(&curl_text) {
            Ok(node) => {
                service.reload_workspace_with_notice(format!("已导入 cURL 请求: {}", node.name))
            }
            Err(e) => service.publish_notice(format!("cURL 导入失败: {e}")),
        });
    }

    pub fn import_from_openapi(&self, content: &str) -> Result<Vec<CollectionNode>> {
        let collection = crate::import_openapi::parse_openapi(content)
            .map_err(|e| anyhow!("OpenAPI 解析失败: {e}"))?;
        self.persist_imported_collection(collection)
    }

    pub fn import_from_postman(&self, content: &str) -> Result<Vec<CollectionNode>> {
        let collection = crate::import_postman::parse_postman(content)
            .map_err(|e| anyhow!("Postman 解析失败: {e}"))?;
        self.persist_imported_collection(collection)
    }

    fn persist_imported_collection(
        &self,
        collection: crate::import_openapi::ImportedCollection,
    ) -> Result<Vec<CollectionNode>> {
        let mut nodes = Vec::new();
        let mut folder_ids = HashMap::<String, String>::new();
        for endpoint in &collection.endpoints {
            let id = format!("node-{}", Uuid::new_v4().simple());
            let parent_id = if let Some(folder_path) = endpoint.parent_folder.as_deref() {
                let mut parent_id = None;
                let mut current_path = String::new();
                for folder_name in folder_path.split('/').filter(|name| !name.is_empty()) {
                    if !current_path.is_empty() {
                        current_path.push('/');
                    }
                    current_path.push_str(folder_name);
                    let folder_id = if let Some(existing_id) = folder_ids.get(&current_path) {
                        existing_id.clone()
                    } else {
                        let folder_id = format!("folder-{}", Uuid::new_v4().simple());
                        self.data_source.create_collection_node(
                            &folder_id,
                            parent_id.as_deref(),
                            NodeKind::Folder,
                            folder_name,
                            "",
                            "",
                            &RequestSnapshot::default(),
                        )?;
                        folder_ids.insert(current_path.clone(), folder_id.clone());
                        folder_id
                    };
                    parent_id = Some(folder_id);
                }
                parent_id
            } else {
                None
            };
            let node = self.data_source.create_collection_node(
                &id,
                parent_id.as_deref(),
                NodeKind::Endpoint,
                &endpoint.name,
                &endpoint.method,
                &endpoint.url,
                &endpoint.snapshot,
            )?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    pub fn import_from_openapi_async(self: &Arc<Self>, content: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.import_from_openapi(&content) {
            Ok(nodes) => service
                .reload_workspace_with_notice(format!("已从 OpenAPI 导入 {} 个端点", nodes.len())),
            Err(e) => service.publish_notice(format!("OpenAPI 导入失败: {e}")),
        });
    }

    pub fn import_from_postman_async(self: &Arc<Self>, content: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.import_from_postman(&content) {
            Ok(nodes) => service
                .reload_workspace_with_notice(format!("已从 Postman 导入 {} 个端点", nodes.len())),
            Err(e) => service.publish_notice(format!("Postman 导入失败: {e}")),
        });
    }

    // ── Export ──

    pub fn export_collection_as_openapi(&self) -> Result<String> {
        let nodes = self.data_source.list_collection_nodes()?;
        let mut paths = serde_json::Map::new();
        for node in &nodes {
            if node.kind != NodeKind::Endpoint {
                continue;
            }
            let snapshot = &node.request;
            let method = snapshot.method.to_lowercase();
            let path_key = if snapshot.url.is_empty() {
                node.url.clone()
            } else {
                snapshot.url.clone()
            };
            let path_item = serde_json::json!({
                method: {
                    "summary": node.name,
                    "responses": { "200": { "description": "OK" } }
                }
            });
            if let Some(obj) = path_item.as_object() {
                if let Some(existing) = paths.get_mut(&path_key) {
                    if let Some(existing_obj) = existing.as_object_mut() {
                        for (k, v) in obj {
                            existing_obj.insert(k.clone(), v.clone());
                        }
                    }
                } else {
                    paths.insert(path_key, path_item);
                }
            }
        }
        let doc = serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "API", "version": "1.0.0" },
            "paths": serde_json::Value::Object(paths),
        });
        Ok(serde_json::to_string_pretty(&doc)?)
    }

    // ── History ──

    pub fn list_history(&self, tab_id: &str, limit: i64) -> Result<Vec<crate::model::HttpHistory>> {
        self.data_source.list_history(tab_id, limit)
    }

    pub fn clear_history(&self, tab_id: &str) -> Result<usize> {
        self.data_source.clear_history(tab_id)
    }

    // ── Collection node lookup ──

    pub fn get_collection_node(&self, id: &str) -> Result<Option<CollectionNode>> {
        self.data_source.get_collection_node(id)
    }

    pub fn send_request(
        self: &Arc<Self>,
        environment: ApiEnvironment,
        request: ApiRequest,
        pre_ops_text: &str,
        post_ops_text: &str,
    ) -> Result<()> {
        let my_gen = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow!("api state poisoned"))?;
            if state.in_flight {
                bail!("已有请求正在执行");
            }
            state.in_flight = true;
            state.pending_response = None;
            state.pending_error = None;
            self.generation
                .fetch_add(1, Ordering::SeqCst)
                .wrapping_add(1)
        };
        self.notify_updated();

        let service = Arc::clone(self);
        let pre_ops = pre_ops_text.to_string();
        let post_ops = post_ops_text.to_string();
        thread::spawn(move || {
            // Load the persisted variable stores so chained values (extracted by
            // earlier requests) resolve in this one. Module scope is not yet keyed
            // from the UI, so it stays empty for now.
            let env_store = service
                .data_source
                .list_variables(VariableScope::Environment, &environment.name)
                .unwrap_or_default();
            let global_store = service
                .data_source
                .list_variables(VariableScope::Global, "")
                .unwrap_or_default();
            let result = perform_request(
                &environment,
                &request,
                &pre_ops,
                &env_store,
                &[],
                &global_store,
            );

            // Discard if a newer request started or the user cancelled.
            if service.generation.load(Ordering::SeqCst) != my_gen {
                return;
            }

            // Post-process a successful response before taking the state lock so
            // the (potentially I/O-bound) variable persistence does not hold it:
            //   1. `extract` rules pull values from the body and are written back
            //      to the environment-scoped store, so the next request in the
            //      chain can reference them (request chaining).
            //   2. assertions are evaluated and summarised into the logs.
            let result = match result {
                Ok(mut resp) => {
                    if !post_ops.is_empty() {
                        let extracted = script_service::extract_variables(&post_ops, &resp.body);
                        if !extracted.is_empty() {
                            let mut names: Vec<String> = extracted.keys().cloned().collect();
                            names.sort();
                            for (key, value) in &extracted {
                                log_error!(
                                    service.data_source.upsert_variable(
                                        VariableScope::Environment,
                                        &environment.name,
                                        key,
                                        value,
                                    ),
                                    warn,
                                    "保存提取变量失败"
                                );
                            }
                            resp.logs.push(format!("提取变量: {}", names.join(", ")));
                        }
                        let assertion_results =
                            script_service::run_assertions(&post_ops, resp.status_code, &resp.body);
                        if !assertion_results.is_empty() {
                            let summary =
                                script_service::format_assertion_results(&assertion_results);
                            resp.logs.push(format!("断言结果:\n{summary}"));
                            resp.assertion_results = assertion_results;
                        }
                    }
                    Ok(resp)
                }
                Err(error) => Err(error),
            };

            let recorded = {
                let mut state = match service.state.lock() {
                    Ok(state) => state,
                    Err(_) => return,
                };
                if service.generation.load(Ordering::SeqCst) != my_gen {
                    return;
                }
                state.in_flight = false;
                match result {
                    Ok(resp) => {
                        let snapshot = resp.clone();
                        state.pending_response = Some(resp);
                        state.pending_error = None;
                        Some(snapshot)
                    }
                    Err(error) => {
                        state.pending_error = Some(error.to_string());
                        let resp = ApiResponse {
                            status_line: String::from("请求失败"),
                            status_code: 0,
                            duration_ms: 0,
                            size_bytes: 0,
                            body: format!("{{\n  \"error\": {:?}\n}}", error.to_string()),
                            headers: String::new(),
                            cookies: String::new(),
                            content_type: String::new(),
                            request_dump: String::new(),
                            curl: String::new(),
                            logs: vec![format!("请求失败: {error}")],
                            assertion_results: Vec::new(),
                            body_bytes: None,
                        };
                        let snapshot = resp.clone();
                        state.pending_response = Some(resp);
                        Some(snapshot)
                    }
                }
            };

            // Persist history from the recorded response snapshot.
            if let Some(resp) = recorded {
                let title = resp.status_line.clone();
                let method = request.method.label().to_string();
                let url_str = build_final_url(&environment, &request);
                let node_id = request.node_id.clone();
                log_error!(
                    service.data_source.insert_history(
                        &node_id,
                        &method,
                        &url_str,
                        resp.status_code as i64,
                        &title,
                        &resp.body,
                    ),
                    warn,
                    "保存请求历史失败"
                );
            }

            service.notify_updated();
        });

        Ok(())
    }

    // ── Environment CRUD (real actions) ──

    pub fn create_environment(&self, name: &str, base_url: &str) -> Result<ApiEnvironment> {
        let id = format!("env-{}", Uuid::new_v4().simple());
        let env = self.data_source.create_environment(&id, name, base_url)?;
        let full = self.data_source.list_environments()?;
        let result = full
            .iter()
            .find(|f| f.env.id == id)
            .map(env_full_to_ui)
            .unwrap_or_else(|| ApiEnvironment {
                name: env.name,
                badge: name
                    .chars()
                    .next()
                    .map(|c| c.to_string())
                    .unwrap_or_default(),
                color: 0x338855,
                base_url: env.base_url,
                variables: Vec::new(),
                headers: Vec::new(),
            });
        Ok(result)
    }

    pub fn create_environment_async(self: &Arc<Self>, name: String, base_url: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.create_environment(&name, &base_url) {
            Ok(env) => service.reload_workspace_with_notice(format!("已创建环境 {}", env.name)),
            Err(error) => service.publish_notice(format!("创建环境失败: {error}")),
        });
    }

    pub fn duplicate_environment(&self, source_index: usize) -> Result<ApiEnvironment> {
        let envs_full = self.data_source.list_environments()?;
        let source = envs_full
            .get(source_index)
            .ok_or_else(|| anyhow!("环境索引 {source_index} 超出范围"))?;
        let new_id = format!("env-{}", Uuid::new_v4().simple());
        let new_name = format!("{} 副本", source.env.name);
        let new_env =
            self.data_source
                .create_environment(&new_id, &new_name, &source.env.base_url)?;
        // Copy variables
        for var in &source.variables {
            self.data_source.upsert_env_variable(
                &new_id,
                var.enabled,
                &var.var_key,
                &var.var_value,
            )?;
        }
        // Copy headers
        let header_rows: Vec<(bool, String, String)> = source
            .headers
            .iter()
            .map(|h| (h.enabled, h.header_key.clone(), h.header_value.clone()))
            .collect();
        if !header_rows.is_empty() {
            self.data_source
                .replace_env_headers(&new_id, &header_rows)?;
        }
        Ok(ApiEnvironment {
            name: new_env.name,
            badge: new_name
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_default(),
            color: 0x338855,
            base_url: new_env.base_url,
            variables: source
                .variables
                .iter()
                .map(|v| KeyValueRow {
                    enabled: v.enabled,
                    key: v.var_key.clone(),
                    value: v.var_value.clone(),
                    value_type: String::new(),
                    description: String::new(),
                })
                .collect(),
            headers: source
                .headers
                .iter()
                .map(|h| KeyValueRow {
                    enabled: h.enabled,
                    key: h.header_key.clone(),
                    value: h.header_value.clone(),
                    value_type: String::new(),
                    description: String::new(),
                })
                .collect(),
        })
    }

    pub fn duplicate_environment_async(self: &Arc<Self>, source_index: usize) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.duplicate_environment(source_index) {
            Ok(env) => service.reload_workspace_with_notice(format!("已复制为 {}", env.name)),
            Err(error) => service.publish_notice(format!("复制环境失败: {error}")),
        });
    }

    pub fn delete_environment_by_index(&self, index: usize) -> Result<bool> {
        let envs_full = self.data_source.list_environments()?;
        if envs_full.len() <= 1 {
            bail!("至少保留一个环境");
        }
        let target = envs_full
            .get(index)
            .ok_or_else(|| anyhow!("环境索引 {index} 超出范围"))?;
        let deleted = self.data_source.delete_environment(&target.env.id)?;
        Ok(deleted)
    }

    pub fn delete_environment_by_index_async(self: &Arc<Self>, index: usize) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.delete_environment_by_index(index) {
            Ok(true) => service.reload_workspace_with_notice(String::from("已删除环境")),
            Ok(false) => service.publish_notice(String::from("环境未删除")),
            Err(error) => service.publish_notice(format!("删除环境失败: {error}")),
        });
    }

    pub fn save_environment_fields(
        &self,
        index: usize,
        name: &str,
        base_url: &str,
        variables_kv: &str,
        headers_kv: &str,
    ) -> Result<()> {
        let envs_full = self.data_source.list_environments()?;
        let target = envs_full
            .get(index)
            .ok_or_else(|| anyhow!("环境索引 {index} 超出范围"))?;
        let env_id = target.env.id.clone();
        self.data_source
            .update_environment(&env_id, name, base_url)?;
        let var_rows: Vec<(bool, String, String)> = parse_kv_lines(variables_kv);
        self.data_source.replace_env_variables(&env_id, &var_rows)?;
        let hdr_rows: Vec<(bool, String, String)> = parse_kv_lines(headers_kv);
        self.data_source.replace_env_headers(&env_id, &hdr_rows)?;
        Ok(())
    }

    pub fn save_environment_fields_async(
        self: &Arc<Self>,
        index: usize,
        name: String,
        base_url: String,
        variables_kv: String,
        headers_kv: String,
    ) {
        let service = Arc::clone(self);
        thread::spawn(move || {
            match service.save_environment_fields(
                index,
                &name,
                &base_url,
                &variables_kv,
                &headers_kv,
            ) {
                Ok(()) => service.reload_workspace_with_notice(format!("已保存环境 {}", name)),
                Err(error) => service.publish_notice(format!("保存环境失败: {error}")),
            }
        });
    }

    pub fn import_environments_json_async(self: &Arc<Self>, content: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.import_environments_json(&content) {
            Ok(count) => service.reload_workspace_with_notice(format!("已导入 {count} 个环境")),
            Err(error) => service.publish_notice(format!("环境导入失败: {error}")),
        });
    }
}

pub fn format_auth_for_input(auth_type: &str, auth_value: &str) -> String {
    let t = auth_type.trim().to_lowercase();
    let v = auth_value.trim();
    if t.is_empty() || v.is_empty() {
        return String::new();
    }
    match t.as_str() {
        "bearer" => format!("Authorization=Bearer {v}"),
        "basic" => format!("Authorization=Basic {v}"),
        "apikey" => format!("X-API-Key={v}"),
        _ => format!("{}={}", t.to_uppercase(), v),
    }
}

impl Default for ApiService {
    fn default() -> Self {
        let paths = AppPaths::resolve().expect("failed to resolve qingqi data path");
        let database = Arc::new(DatabaseService::new(paths.clone()));
        Self::new(database, paths)
    }
}

fn perform_request(
    environment: &ApiEnvironment,
    request: &ApiRequest,
    pre_ops_text: &str,
    env_store: &[ApiVariable],
    module_store: &[ApiVariable],
    global_store: &[ApiVariable],
) -> Result<ApiResponse> {
    let request_path = request_path_with_segments(request, |value| value.to_string())?;
    let mut draft = script_service::RequestDraft {
        method: request.method.label().to_string(),
        url: request_path,
        params: request
            .params
            .iter()
            .filter(|r| r.enabled && !r.key.is_empty())
            .map(|r| (r.key.clone(), r.value.clone()))
            .collect(),
        headers: request
            .headers
            .iter()
            .filter(|r| r.enabled && !r.key.is_empty())
            .map(|r| (r.key.clone(), r.value.clone()))
            .collect(),
        body: request.body.clone(),
    };
    let temporary = script_service::apply_pre_ops(&mut draft, pre_ops_text);

    let env_vars: HashMap<String, String> = environment
        .variables
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| (row.key.trim().to_string(), row.value.trim().to_string()))
        .collect();

    // Resolve `{{var}}` / `${var}` through the full precedence chain: pre-ops
    // temporaries → inline env vars → env/module/global persisted stores. The
    // persisted stores are what make request chaining work (a value `extract`ed
    // by an earlier request is read back here).
    let resolve = |text: &str| -> String {
        variable_service::resolve_text(
            text,
            &temporary,
            &env_vars,
            env_store,
            &HashMap::new(),
            module_store,
            &HashMap::new(),
            global_store,
        )
    };

    // Split auth into header- and query-based contributions. API keys placed
    // in the query string are appended to the params; everything else (Bearer,
    // Basic, header API keys) becomes a request header.
    let mut auth_headers: Vec<(String, String)> = Vec::new();
    let mut auth_query: Vec<(String, String)> = Vec::new();
    for r in &request.auth {
        if !r.enabled || r.key.trim().is_empty() {
            continue;
        }
        let k = r.key.trim().to_string();
        let v = resolve(r.value.trim());
        if r.description.trim().eq_ignore_ascii_case("query") {
            auth_query.push((k, v));
        } else {
            auth_headers.push((k, v));
        }
    }

    let resolved_url = resolve(&draft.url);
    let mut resolved_params: Vec<(String, String)> = draft
        .params
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (resolve(k), resolve(v)))
        .collect();
    for (k, v) in &auth_query {
        resolved_params.push((k.clone(), v.clone()));
    }

    let base_url = if resolved_url.starts_with("http://") || resolved_url.starts_with("https://") {
        resolved_url.clone()
    } else {
        let base = environment.base_url.trim().trim_end_matches('/');
        let route = if resolved_url.starts_with('/') {
            resolved_url.clone()
        } else {
            format!("/{resolved_url}")
        };
        format!("{base}{route}")
    };
    let url = if resolved_params.is_empty() {
        base_url
    } else {
        append_query_params(&base_url, &resolved_params)
    };

    let allows_body = request.method.allows_body();
    let raw_body = if allows_body {
        resolve(&draft.body)
    } else {
        String::new()
    };

    // Collect the effective request headers (resolved) once, so the real
    // request, the cURL preview and the request dump all stay in sync.
    let mut headers: Vec<(String, String)> = Vec::new();
    for (k, v) in &draft.headers {
        if k.trim().is_empty() {
            continue;
        }
        headers.push((k.trim().to_string(), resolve(v)));
    }
    for (k, v) in &auth_headers {
        headers.push((k.clone(), v.clone()));
    }
    let cookie_pairs: Vec<String> = request
        .cookies
        .iter()
        .filter(|r| r.enabled && !r.key.trim().is_empty())
        .map(|r| format!("{}={}", r.key.trim(), resolve(r.value.trim())))
        .collect();
    if !cookie_pairs.is_empty() {
        headers.push(("Cookie".to_string(), cookie_pairs.join("; ")));
    }
    for r in &environment.headers {
        if !r.enabled || r.key.trim().is_empty() {
            continue;
        }
        headers.push((r.key.trim().to_string(), resolve(&r.value)));
    }

    // Auto Content-Type based on the body mode, unless the user set one.
    let has_content_type = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
    if allows_body && !raw_body.is_empty() && !has_content_type {
        if request.body_mode == BodyMode::FormData {
            headers.push((
                "Content-Type".to_string(),
                format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
            ));
        } else if let Some(ct) = default_content_type(request.body_mode) {
            headers.push(("Content-Type".to_string(), ct.to_string()));
        }
    }

    // Resolve the outgoing body bytes. Binary mode treats the text as a path;
    // FormData assembles a multipart body by hand.
    let body_bytes: Vec<u8> = if !allows_body || raw_body.is_empty() {
        Vec::new()
    } else if request.body_mode == BodyMode::Binary {
        std::fs::read(raw_body.trim())
            .map_err(|e| anyhow!("读取二进制文件失败 ({}): {e}", raw_body.trim()))?
    } else if request.body_mode == BodyMode::FormData {
        build_multipart_body(&raw_body, MULTIPART_BOUNDARY)?
    } else if request.body_mode == BodyMode::FormUrlEncoded {
        build_form_urlencoded_body(&raw_body).into_bytes()
    } else {
        raw_body.clone().into_bytes()
    };

    // Build cURL preview and request dump from the same resolved headers/body.
    let body_preview = match request.body_mode {
        BodyMode::Binary if !raw_body.is_empty() => format!("<二进制文件: {}>", raw_body.trim()),
        BodyMode::FormUrlEncoded => String::from_utf8_lossy(&body_bytes).into_owned(),
        _ => raw_body.clone(),
    };
    let header_lines: Vec<String> = headers.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    let curl_preview = build_curl_preview(&url, request.method, &header_lines, &body_preview);
    let request_dump = build_request_dump(&url, request.method, &header_lines, &body_preview);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;

    let method = reqwest::Method::from_bytes(request.method.label().as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let mut req = client.request(method, &url);

    for (k, v) in &headers {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
            if let Ok(value) = reqwest::header::HeaderValue::from_str(v) {
                req = req.header(name, value);
            }
        }
    }

    if !body_bytes.is_empty() {
        req = req.body(body_bytes);
    }

    let started = Instant::now();
    let resp = req.send()?;
    let duration_ms = started.elapsed().as_millis();

    let status_code = resp.status().as_u16();
    let status_line = format!(
        "HTTP/1.1 {} {}",
        status_code,
        resp.status().canonical_reason().unwrap_or("")
    );

    let mut headers_text = status_line.clone();
    headers_text.push('\n');
    let mut cookies_text = String::new();
    let mut content_type = String::new();
    for (key, value) in resp.headers() {
        let value_str = value.to_str().unwrap_or("");
        headers_text.push_str(&format!("{key}: {value_str}\n"));
        if key.as_str().eq_ignore_ascii_case("set-cookie") {
            cookies_text.push_str(value_str);
            cookies_text.push('\n');
        }
        if key.as_str().eq_ignore_ascii_case("content-type") {
            content_type = value_str.to_string();
        }
    }

    // For binary content types, read raw bytes to preserve data integrity.
    // For text content types, read as text directly.
    let is_binary = is_binary_response_content_type(&content_type);
    let (resp_body, body_bytes) = if is_binary {
        let bytes = resp.bytes()?.to_vec();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        (text, Some(bytes))
    } else {
        let text = resp.text()?;
        (text, None)
    };
    let size_bytes = if body_bytes.is_some() {
        body_bytes.as_ref().map(|b| b.len()).unwrap_or(0)
    } else {
        resp_body.len()
    };

    Ok(ApiResponse {
        status_line: status_line.clone(),
        status_code,
        duration_ms,
        size_bytes,
        body: prettify_body(&resp_body),
        headers: headers_text,
        cookies: cookies_text,
        content_type,
        request_dump,
        curl: curl_preview,
        logs: vec![
            format!("发送 {} {}", request.method.label(), url),
            format!("响应 {}", status_line),
            format!("耗时 {} ms", duration_ms),
        ],
        assertion_results: Vec::new(),
        body_bytes,
    })
}

/// Build a runnable code snippet for the request in the given language.
/// Mirrors the real send path: the URL carries path + query params (and any
/// query-located auth), headers carry request + environment + header-located
/// auth + a combined `Cookie` line, and form bodies are split into `form_data`.
pub fn code_snippet(
    environment: &ApiEnvironment,
    request: &ApiRequest,
    lang: crate::code_gen::CodeLanguage,
) -> String {
    let url = build_final_url(environment, request);
    let query_auth: Vec<(String, String)> = request
        .auth
        .iter()
        .filter(|r| r.enabled && r.description == "query" && !r.key.trim().is_empty())
        .map(|r| (r.key.trim().to_string(), r.value.trim().to_string()))
        .collect();
    let url = if query_auth.is_empty() {
        url
    } else {
        append_query_params(&url, &query_auth)
    };

    let mut headers: Vec<KeyValueRow> = request
        .headers
        .iter()
        .filter(|r| r.enabled && !r.key.trim().is_empty())
        .cloned()
        .collect();
    for r in &request.auth {
        if r.enabled && r.description != "query" && !r.key.trim().is_empty() {
            headers.push(r.clone());
        }
    }
    for r in &environment.headers {
        if r.enabled && !r.key.trim().is_empty() {
            headers.push(r.clone());
        }
    }
    let cookie_pairs: Vec<String> = request
        .cookies
        .iter()
        .filter(|r| r.enabled && !r.key.trim().is_empty())
        .map(|r| format!("{}={}", r.key.trim(), r.value.trim()))
        .collect();
    if !cookie_pairs.is_empty() {
        headers.push(KeyValueRow::new("Cookie", cookie_pairs.join("; ")));
    }

    let form_data = if matches!(
        request.body_mode,
        BodyMode::FormData | BodyMode::FormUrlEncoded
    ) {
        parse_kv_text(&request.body)
    } else {
        Vec::new()
    };

    let code_req = crate::code_gen::CodeGenRequest {
        method: request.method.label().to_string(),
        url,
        headers,
        body: request.body.clone(),
        body_mode: request.body_mode,
        form_data,
    };
    crate::code_gen::generate(lang, &code_req)
}

/// Append query parameters to a base URL using `Url`'s structured query API.
/// Both regular params and query-located auth share this encoding path, so
/// keys/values with `&`, `=`, spaces, or non-ASCII bytes are percent-encoded
/// rather than being concatenated raw.
fn append_query_params(base_url: &str, params: &[(String, String)]) -> String {
    let mut url = match Url::parse(base_url) {
        Ok(url) => url,
        Err(_) => return base_url.to_string(),
    };
    {
        let mut query = url.query_pairs_mut();
        for (k, v) in params {
            query.append_pair(k, v);
        }
    }
    url.to_string()
}

pub fn split_request_url(raw: &str) -> (String, Vec<KeyValueRow>) {
    let (without_fragment, fragment) = raw
        .split_once('#')
        .map(|(url, fragment)| (url, Some(fragment)))
        .unwrap_or((raw, None));
    let (base, query) = without_fragment
        .split_once('?')
        .map(|(base, query)| (base, query))
        .unwrap_or((without_fragment, ""));
    let mut clean_base = base.to_string();
    if let Some(fragment) = fragment {
        clean_base.push('#');
        clean_base.push_str(fragment);
    }
    let params = url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| KeyValueRow::new(key.into_owned(), value.into_owned()))
        .collect();
    (clean_base, params)
}

pub fn compose_request_url(base: &str, rows: &[KeyValueRow]) -> String {
    let (without_fragment, fragment) = base
        .split_once('#')
        .map(|(url, fragment)| (url, Some(fragment)))
        .unwrap_or((base, None));
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for row in rows
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        serializer.append_pair(row.key.trim(), &row.value);
    }
    let query = serializer.finish();
    let mut result = without_fragment.to_string();
    if !query.is_empty() {
        result.push('?');
        result.push_str(&query);
    }
    if let Some(fragment) = fragment {
        result.push('#');
        result.push_str(fragment);
    }
    result
}

pub fn merge_url_query_rows(existing: &[KeyValueRow], parsed: Vec<KeyValueRow>) -> Vec<KeyValueRow> {
    let mut used = vec![false; existing.len()];
    let mut merged = Vec::with_capacity(parsed.len() + existing.len());
    for parsed_row in parsed {
        let matching = existing.iter().enumerate().find(|(index, row)| {
            !used[*index] && row.enabled && row.key == parsed_row.key
        });
        if let Some((index, row)) = matching {
            used[index] = true;
            let mut row = row.clone();
            row.value = parsed_row.value;
            merged.push(row);
        } else {
            merged.push(parsed_row);
        }
    }
    merged.extend(
        existing
            .iter()
            .enumerate()
            .filter(|(index, row)| !used[*index] && !row.enabled)
            .map(|(_, row)| row.clone()),
    );
    merged
}

pub fn extract_path_parameter_names(raw_url: &str) -> Vec<String> {
    let path = raw_url
        .split(['?', '#'])
        .next()
        .unwrap_or(raw_url);
    let mut names = Vec::new();
    let chars: Vec<char> = path.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '{' && (index == 0 || chars[index - 1] != '{') {
            if let Some(end_offset) = chars[index + 1..].iter().position(|ch| *ch == '}') {
                let end = index + 1 + end_offset;
                if end + 1 >= chars.len() || chars[end + 1] != '}' {
                    let name: String = chars[index + 1..end].iter().collect();
                    if valid_path_parameter_name(&name) && !names.contains(&name) {
                        names.push(name);
                    }
                }
                index = end;
            }
        }
        index += 1;
    }
    for segment in path.split('/') {
        if let Some(name) = segment.strip_prefix(':') {
            if valid_path_parameter_name(name) && !names.iter().any(|item| item == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn valid_path_parameter_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

pub fn merge_path_parameter_rows(
    existing: &[KeyValueRow],
    names: &[String],
) -> Vec<KeyValueRow> {
    if names.is_empty() {
        return existing.to_vec();
    }
    names
        .iter()
        .map(|name| {
            existing
                .iter()
                .find(|row| row.key == *name)
                .cloned()
                .unwrap_or_else(|| KeyValueRow::new(name, ""))
        })
        .collect()
}

fn build_final_url(environment: &ApiEnvironment, request: &ApiRequest) -> String {
    let base_url = substitute_vars(environment.base_url.trim(), environment);
    let request_path = request_path_with_segments(request, |value| {
        substitute_vars(value, environment)
    })
    .unwrap_or_else(|_| request.path.clone());
    let path = substitute_vars(request_path.trim(), environment);
    let path = if path.starts_with("http://") || path.starts_with("https://") {
        path
    } else {
        let base = base_url.trim_end_matches('/');
        let route = if path.starts_with('/') {
            path
        } else {
            format!("/{path}")
        };
        format!("{base}{route}")
    };

    let query: Vec<(String, String)> = request
        .params
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| {
            (
                row.key.trim().to_string(),
                substitute_vars(row.value.trim(), environment),
            )
        })
        .collect();
    if query.is_empty() {
        path
    } else {
        append_query_params(&path, &query)
    }
}

fn request_path_with_segments<F>(request: &ApiRequest, resolve: F) -> Result<String>
where
    F: Fn(&str) -> String,
{
    let base_path = resolve(request.path.trim());
    let names = extract_path_parameter_names(&base_path);
    if !names.is_empty() {
        let mut path = base_path;
        for name in names {
            let row = request
                .path_rows
                .iter()
                .find(|row| row.key == name)
                .ok_or_else(|| anyhow!("Path 参数 {name} 未配置"))?;
            if !row.enabled || row.value.trim().is_empty() {
                bail!("Path 参数 {name} 未启用或未填写");
            }
            let value = resolve(row.value.trim());
            let encoded = url::form_urlencoded::byte_serialize(value.as_bytes())
                .collect::<String>()
                .replace('+', "%20");
            path = path.replace(&format!("{{{name}}}"), &encoded);
            path = path
                .split('/')
                .map(|segment| {
                    if segment == format!(":{name}") {
                        encoded.as_str()
                    } else {
                        segment
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
        }
        return Ok(path);
    }
    let extra_path = request
        .path_rows
        .iter()
        .filter(|row| row.enabled && !row.value.trim().is_empty())
        .map(|row| resolve(row.value.trim()).trim_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if extra_path.is_empty() {
        Ok(base_path)
    } else {
        Ok(format!(
            "{}/{}",
            base_path.trim_end_matches('/'),
            extra_path.join("/")
        ))
    }
}

fn build_request_dump(url: &str, method: HttpMethod, headers: &[String], body: &str) -> String {
    let mut dump = format!("{} {}\n", method.label(), url);
    for header in headers {
        dump.push_str(header);
        dump.push('\n');
    }
    if !body.is_empty() {
        dump.push('\n');
        dump.push_str(body);
    }
    dump
}

fn build_curl_preview(url: &str, method: HttpMethod, headers: &[String], body: &str) -> String {
    let mut preview = format!("curl -X {} '{}'", method.label(), url);
    for header in headers {
        preview.push_str(&format!(" \\\n  -H '{}'", header.replace('\'', "\\'")));
    }
    if !body.is_empty() {
        preview.push_str(&format!(
            " \\\n  --data-raw '{}'",
            body.replace('\'', "\\'")
        ));
    }
    preview
}

fn build_form_urlencoded_body(raw_body: &str) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (enabled, key, value) in parse_kv_lines(raw_body) {
        if enabled && !key.trim().is_empty() {
            serializer.append_pair(key.trim(), value.trim());
        }
    }
    serializer.finish()
}

fn default_content_type(mode: BodyMode) -> Option<&'static str> {
    match mode {
        BodyMode::Json => Some("application/json"),
        BodyMode::Xml => Some("application/xml"),
        BodyMode::FormUrlEncoded => Some("application/x-www-form-urlencoded"),
        BodyMode::Text => Some("text/plain; charset=utf-8"),
        BodyMode::FormData | BodyMode::Binary | BodyMode::None => None,
    }
}

/// Fixed multipart boundary. reqwest is built without the `multipart` feature,
/// so the `multipart/form-data` body is assembled by hand.
const MULTIPART_BOUNDARY: &str = "----QingqiFormBoundary7MA4YWxkTrZu0gW";

/// Detect MIME type from file bytes (magic bytes) with extension fallback.
fn detect_file_mime_type(bytes: &[u8], path: &str) -> String {
    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type().to_string();
    }
    mime_guess::from_path(path)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

/// Build a `multipart/form-data` body from `key=value` lines. A value of the
/// form `@<path>` is sent as a file part (the file is read here); other values
/// are plain text fields. Lines starting with `#` are skipped (disabled).
fn build_multipart_body(raw_body: &str, boundary: &str) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    for line in raw_body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .map(|(k, v)| (k.trim(), v.trim()))
            .unwrap_or((line, ""));
        if key.is_empty() {
            continue;
        }
        out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        if let Some(path) = value.strip_prefix('@') {
            let path = path.trim();
            let bytes =
                std::fs::read(path).map_err(|e| anyhow!("读取表单文件失败 ({path}): {e}"))?;
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("file");
            let content_type = detect_file_mime_type(&bytes, path);
            out.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{key}\"; filename=\"{filename}\"\r\n"
                )
                .as_bytes(),
            );
            out.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
            out.extend_from_slice(&bytes);
            out.extend_from_slice(b"\r\n");
        } else {
            out.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{key}\"\r\n\r\n").as_bytes(),
            );
            out.extend_from_slice(value.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok(out)
}

/// Check if a response Content-Type indicates binary data that should be
/// read as raw bytes (not converted to UTF-8 text).
fn is_binary_response_content_type(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    let ct = ct.split(';').next().unwrap_or("").trim();
    ct.starts_with("image/")
        || ct.starts_with("audio/")
        || ct.starts_with("video/")
        || ct.starts_with("font/")
        || ct == "application/octet-stream"
        || ct == "application/pdf"
        || ct == "application/zip"
        || ct == "application/gzip"
        || ct == "application/x-gzip"
        || ct == "application/vnd.apache.parquet"
        || ct.starts_with("application/vnd.")
        || ct.starts_with("application/x-")
}

fn prettify_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    serde_json::from_str::<Value>(trimmed)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| body.to_string())
}

fn substitute_vars(text: &str, environment: &ApiEnvironment) -> String {
    let env_vars: HashMap<String, String> = environment
        .variables
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| (row.key.trim().to_string(), row.value.trim().to_string()))
        .collect();
    variable_service::resolve_text(
        text,
        &HashMap::new(),
        &env_vars,
        &[],
        &HashMap::new(),
        &[],
        &HashMap::new(),
        &[],
    )
}

pub fn detect_body_mode(body: &str) -> &'static str {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "none";
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return "json";
    }
    "text"
}

#[cfg(test)]
fn resolve_with_temp(
    text: &str,
    temporary: &HashMap<String, String>,
    env_vars: &HashMap<String, String>,
) -> String {
    variable_service::resolve_text(
        text,
        temporary,
        env_vars,
        &[],
        &HashMap::new(),
        &[],
        &HashMap::new(),
        &[],
    )
}

// ── Conversion helpers ──

fn env_full_to_ui(full: &EnvironmentFull) -> ApiEnvironment {
    ApiEnvironment {
        name: full.env.name.clone(),
        badge: full
            .env
            .name
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default(),
        color: 0x338855,
        base_url: full.env.base_url.clone(),
        variables: full
            .variables
            .iter()
            .map(|v| KeyValueRow {
                enabled: v.enabled,
                key: v.var_key.clone(),
                value: v.var_value.clone(),
                value_type: String::new(),
                description: String::new(),
            })
            .collect(),
        headers: full
            .headers
            .iter()
            .map(|h| KeyValueRow {
                enabled: h.enabled,
                key: h.header_key.clone(),
                value: h.header_value.clone(),
                value_type: String::new(),
                description: String::new(),
            })
            .collect(),
    }
}

fn env_ui_to_full(env: &ApiEnvironment, id: &str) -> EnvironmentFull {
    EnvironmentFull {
        env: crate::model::Environment {
            id: id.to_string(),
            name: env.name.clone(),
            base_url: env.base_url.clone(),
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
        },
        variables: env
            .variables
            .iter()
            .enumerate()
            .map(|(i, v)| EnvVariable {
                id: 0,
                environment_id: id.to_string(),
                enabled: v.enabled,
                var_key: v.key.clone(),
                var_value: v.value.clone(),
                sort_order: i as i64,
            })
            .collect(),
        headers: env
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| EnvHeader {
                id: 0,
                environment_id: id.to_string(),
                enabled: h.enabled,
                header_key: h.key.clone(),
                header_value: h.value.clone(),
                sort_order: i as i64,
            })
            .collect(),
    }
}

fn build_groups_from_nodes(nodes: &[CollectionNode]) -> Vec<ApiGroup> {
    let mut groups = Vec::new();

    for node in nodes {
        if node.kind == NodeKind::Folder && node.parent_id.is_none() {
            groups.push(build_group_from_node(node, nodes));
        }
    }

    let root_endpoints: Vec<&CollectionNode> = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Endpoint && n.parent_id.is_none())
        .collect();
    if !root_endpoints.is_empty() {
        groups.insert(
            0,
            ApiGroup {
                id: None,
                name: String::from("默认"),
                folders: Vec::new(),
                requests: root_endpoints
                    .iter()
                    .map(|ep| node_to_request(ep, nodes))
                    .collect(),
            },
        );
    }

    groups
}

fn build_group_from_node(node: &CollectionNode, nodes: &[CollectionNode]) -> ApiGroup {
    let mut folders = Vec::new();
    let mut requests = Vec::new();

    for child in nodes {
        if child.parent_id.as_deref() == Some(node.id.as_str()) {
            match child.kind {
                NodeKind::Folder => {
                    folders.push(build_group_from_node(child, nodes));
                }
                NodeKind::Endpoint => {
                    requests.push(node_to_request(child, nodes));
                }
                NodeKind::Case => {}
            }
        }
    }

    ApiGroup {
        id: Some(node.id.clone()),
        name: node.name.clone(),
        folders,
        requests,
    }
}

fn node_to_request(node: &CollectionNode, nodes: &[CollectionNode]) -> ApiRequest {
    let snapshot = node.request.clone();
    let method = HttpMethod::from_label(&node.method);
    let mut request = ApiRequest {
        node_id: node.id.clone(),
        title: node.name.clone(),
        method,
        path: if snapshot.url.is_empty() {
            node.url.clone()
        } else {
            snapshot.url
        },
        params: parse_kv_text(&snapshot.params_text),
        path_rows: parse_kv_text(&snapshot.path_params_text),
        body: snapshot.body_text,
        body_mode: BodyMode::from_db(&snapshot.body_mode),
        body_payloads: snapshot.body_payloads,
        editor_tab: snapshot.editor_tab,
        headers: parse_kv_text(&snapshot.headers_text),
        cookies: parse_kv_text(&snapshot.cookies_text),
        auth: parse_kv_text(&format_auth(&snapshot.auth_type, &snapshot.auth_value)),
        pre_ops: snapshot.pre_ops_text,
        post_ops: snapshot.post_ops_text,
        scenarios: node_scenarios(node, nodes),
    };
    request.ensure_body_payloads();
    request
}

/// Collect the `Case` child nodes of an endpoint as scenarios, ordered by
/// `sort_order`. Cases are not executed on load, so each is reported as
/// `Pending` until a run flips it.
fn node_scenarios(endpoint: &CollectionNode, nodes: &[CollectionNode]) -> Vec<ApiScenario> {
    let mut cases: Vec<&CollectionNode> = nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Case && n.parent_id.as_deref() == Some(endpoint.id.as_str())
        })
        .collect();
    cases.sort_by_key(|n| n.sort_order);
    cases
        .iter()
        .map(|c| ApiScenario {
            node_id: c.id.clone(),
            name: c.name.clone(),
            request: Some(Box::new(case_node_to_request(c, endpoint))),
        })
        .collect()
}

fn case_node_to_request(case: &CollectionNode, endpoint: &CollectionNode) -> ApiRequest {
    let snapshot = if case.request.is_empty() {
        endpoint.request.clone()
    } else {
        case.request.clone()
    };
    let method_label = if snapshot.method.is_empty() {
        if case.method.is_empty() {
            endpoint.method.as_str()
        } else {
            case.method.as_str()
        }
    } else {
        snapshot.method.as_str()
    };
    let url = if snapshot.url.is_empty() {
        if case.url.is_empty() {
            endpoint.url.clone()
        } else {
            case.url.clone()
        }
    } else {
        snapshot.url.clone()
    };
    let mut request = ApiRequest {
        node_id: case.id.clone(),
        title: case.name.clone(),
        method: HttpMethod::from_label(method_label),
        path: url,
        params: parse_kv_text(&snapshot.params_text),
        path_rows: parse_kv_text(&snapshot.path_params_text),
        body: snapshot.body_text,
        body_mode: BodyMode::from_db(&snapshot.body_mode),
        body_payloads: snapshot.body_payloads,
        editor_tab: snapshot.editor_tab,
        headers: parse_kv_text(&snapshot.headers_text),
        cookies: parse_kv_text(&snapshot.cookies_text),
        auth: parse_kv_text(&format_auth(&snapshot.auth_type, &snapshot.auth_value)),
        pre_ops: snapshot.pre_ops_text,
        post_ops: snapshot.post_ops_text,
        scenarios: Vec::new(),
    };
    request.ensure_body_payloads();
    request
}

fn request_to_snapshot(method: &str, url: &str, request: &ApiRequest) -> RequestSnapshot {
    let body_text = request.active_body_text();
    RequestSnapshot {
        method: method.to_string(),
        url: url.to_string(),
        params_text: format_kv_rows(&request.params),
        path_params_text: format_kv_rows(&request.path_rows),
        headers_text: format_kv_rows(&request.headers),
        cookies_text: format_kv_rows(&request.cookies),
        body_text,
        body_mode: request.body_mode.as_str().to_string(),
        body_payloads: request.body_payloads.clone(),
        editor_tab: request.editor_tab.clone(),
        auth_type: extract_auth_type(&request.auth),
        auth_value: extract_auth_value(&request.auth),
        pre_ops_text: request.pre_ops.clone(),
        post_ops_text: request.post_ops.clone(),
    }
}

fn parse_kv_text(text: &str) -> Vec<KeyValueRow> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            // A leading `#` marks a disabled row (round-trips with `format_kv_rows`).
            let (enabled, content) = match line.strip_prefix('#') {
                Some(rest) => (false, rest.trim()),
                None => (true, line),
            };
            let mut parts = content.splitn(3, '\t');
            let pair = parts.next().unwrap_or_default().trim();
            let value_type = parts.next().unwrap_or_default().trim();
            let description = parts.next().unwrap_or_default().trim();
            let (key, value) = pair
                .split_once('=')
                .or_else(|| pair.split_once(':'))
                .map(|(k, v)| (k.trim(), v.trim()))
                .unwrap_or((pair, ""));
            KeyValueRow {
                enabled,
                key: key.to_string(),
                value: value.to_string(),
                value_type: value_type.to_string(),
                description: description.to_string(),
            }
        })
        .collect()
}

fn parse_kv_lines(text: &str) -> Vec<(bool, String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (enabled, content) = match line.strip_prefix('#') {
                Some(rest) => (false, rest.trim()),
                None => (true, line),
            };
            let pair = content.split('\t').next().unwrap_or_default().trim();
            let (key, value) = pair
                .split_once('=')
                .map(|(k, v)| (k.trim(), v.trim()))
                .unwrap_or((pair, ""));
            (enabled, key.to_string(), value.to_string())
        })
        .collect()
}

fn format_kv_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .filter(|r| !r.key.is_empty())
        .map(|r| {
            let mut body = format!("{}={}", r.key, r.value);
            let value_type = sanitize_kv_metadata(&r.value_type);
            let description = sanitize_kv_metadata(&r.description);
            if !value_type.is_empty() || !description.is_empty() {
                body.push('\t');
                body.push_str(&value_type);
                body.push('\t');
                body.push_str(&description);
            }
            if r.enabled { body } else { format!("# {body}") }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize_kv_metadata(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ").trim().to_string()
}

fn format_auth(auth_type: &str, auth_value: &str) -> String {
    let t = auth_type.trim();
    let v = auth_value.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return String::new();
    }
    match t.to_lowercase().as_str() {
        "bearer" => format!("Authorization=Bearer {v}"),
        "basic" => format!("Authorization=Basic {v}"),
        "apikey" => format!("X-API-Key={v}"),
        _ => format!("{t}={v}"),
    }
}

fn extract_auth_type(auth_rows: &[KeyValueRow]) -> String {
    for row in auth_rows {
        let key = row.key.trim().to_lowercase();
        if key == "authorization" {
            let val = row.value.trim();
            if val.starts_with("Bearer ") {
                return "bearer".into();
            } else if val.starts_with("Basic ") {
                return "basic".into();
            }
        } else if key == "x-api-key" {
            return "apikey".into();
        }
    }
    String::new()
}

fn extract_auth_value(auth_rows: &[KeyValueRow]) -> String {
    for row in auth_rows {
        let key = row.key.trim().to_lowercase();
        if key == "authorization" {
            let val = row.value.trim();
            if let Some(rest) = val.strip_prefix("Bearer ") {
                return rest.to_string();
            } else if let Some(rest) = val.strip_prefix("Basic ") {
                return rest.to_string();
            }
        } else if key == "x-api-key" {
            return row.value.trim().to_string();
        }
    }
    String::new()
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 encode (with `=` padding). Implemented inline because the
/// workspace does not depend on a base64 crate and `reqwest` only pulls
/// `blocking`/`stream` features.
pub(crate) fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Standard base64 decode. Returns `None` on invalid input. Lossy bytes are
/// converted to a UTF-8 string by the caller when needed.
pub(crate) fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let cleaned: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks(4) {
        let mut acc = 0u32;
        let mut bits = 0;
        for &c in chunk {
            acc = (acc << 6) | val(c)?;
            bits += 6;
        }
        // Left-align the accumulated bits and emit whole bytes.
        acc <<= 24 - bits;
        let n_bytes = bits / 8;
        for i in 0..n_bytes {
            out.push(((acc >> (16 - i * 8)) & 0xff) as u8);
        }
    }
    Some(out)
}

fn default_environments() -> Vec<ApiEnvironment> {
    vec![ApiEnvironment {
        name: String::from("默认环境"),
        badge: String::from("默"),
        color: 0x338855,
        base_url: String::from("http://127.0.0.1:8000"),
        variables: vec![
            KeyValueRow::new("BASE_URL", "http://127.0.0.1:8000"),
            KeyValueRow::new("API_KEY", ""),
            KeyValueRow::new("AUTH_TOKEN", ""),
        ],
        headers: Vec::new(),
    }]
}

impl HttpMethod {
    pub fn from_label(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "PATCH" => Self::Patch,
            "DELETE" | "DEL" => Self::Delete,
            "HEAD" => Self::Head,
            "OPTIONS" => Self::Options,
            _ => Self::Get,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RequestSnapshot;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
    use std::fs;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_store() -> ApiDebuggerDataSource {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-api-svc-test-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(dir.clone())));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "api_debugger/main",
                dir.join("test.db"),
            ))
            .unwrap();
        ApiDebuggerDataSource::open(database, "api_debugger/main").unwrap()
    }

    fn service_with_store(store: ApiDebuggerDataSource) -> ApiService {
        ApiService {
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                pending_groups: None,
                pending_environments: None,
            }),
            update_subscribers: Mutex::new(Vec::new()),
            data_source: store,
            generation: AtomicU64::new(0),
        }
    }

    #[test]
    fn state_change_notifies_subscribers() {
        let service = service_with_store(temp_store());
        let updates = service.subscribe_updates();

        service.publish_notice(String::from("updated"));

        updates
            .recv_timeout(Duration::from_secs(1))
            .expect("state change event");
        assert_eq!(service.take_pending_notice().as_deref(), Some("updated"));
    }

    #[test]
    fn build_groups_empty_store() {
        let store = temp_store();
        let nodes = store.list_collection_nodes().unwrap();
        let groups = build_groups_from_nodes(&nodes);
        assert!(groups.is_empty());
    }

    #[test]
    fn build_groups_with_folder_and_endpoints() {
        let store = temp_store();

        store
            .create_collection_node(
                "folder-1",
                None,
                NodeKind::Folder,
                "用户模块",
                "",
                "",
                &RequestSnapshot::default(),
            )
            .unwrap();
        store
            .create_collection_node(
                "ep-1",
                Some("folder-1"),
                NodeKind::Endpoint,
                "/user/info",
                "GET",
                "/api/v1/user/info",
                &RequestSnapshot {
                    method: "GET".into(),
                    url: "/api/v1/user/info".into(),
                    params_text: "page=1".into(),
                    headers_text: "Authorization=Bearer tok".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        store
            .create_collection_node(
                "ep-2",
                Some("folder-1"),
                NodeKind::Endpoint,
                "/user/login",
                "POST",
                "/api/v1/user/login",
                &RequestSnapshot {
                    method: "POST".into(),
                    url: "/api/v1/user/login".into(),
                    body_text: r#"{"email":"a@b.com"}"#.into(),
                    ..Default::default()
                },
            )
            .unwrap();

        let nodes = store.list_collection_nodes().unwrap();
        let groups = build_groups_from_nodes(&nodes);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "用户模块");
        assert_eq!(groups[0].requests.len(), 2);

        assert_eq!(groups[0].requests[0].title, "/user/info");
        assert_eq!(groups[0].requests[0].method, HttpMethod::Get);
        assert_eq!(groups[0].requests[0].path, "/api/v1/user/info");
        assert_eq!(groups[0].requests[0].params.len(), 1);
        assert_eq!(groups[0].requests[0].params[0].key, "page");
        assert_eq!(groups[0].requests[0].headers.len(), 1);
        assert_eq!(groups[0].requests[0].headers[0].key, "Authorization");

        assert_eq!(groups[0].requests[1].title, "/user/login");
        assert_eq!(groups[0].requests[1].method, HttpMethod::Post);
        assert_eq!(groups[0].requests[1].body, r#"{"email":"a@b.com"}"#);
    }

    #[test]
    fn build_groups_root_endpoints_get_default_group() {
        let store = temp_store();

        store
            .create_collection_node(
                "ep-root",
                None,
                NodeKind::Endpoint,
                "health",
                "GET",
                "/health",
                &RequestSnapshot::default(),
            )
            .unwrap();

        let nodes = store.list_collection_nodes().unwrap();
        let groups = build_groups_from_nodes(&nodes);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "默认");
        assert_eq!(groups[0].requests.len(), 1);
        assert_eq!(groups[0].requests[0].title, "health");
    }

    #[test]
    fn case_nodes_load_as_endpoint_scenarios() {
        let store = temp_store();

        store
            .create_collection_node(
                "ep-cases",
                None,
                NodeKind::Endpoint,
                "下单",
                "POST",
                "/orders",
                &RequestSnapshot::default(),
            )
            .unwrap();
        store
            .create_collection_node(
                "case-1",
                Some("ep-cases"),
                NodeKind::Case,
                "正常下单",
                "",
                "",
                &RequestSnapshot::default(),
            )
            .unwrap();
        store
            .create_collection_node(
                "case-2",
                Some("ep-cases"),
                NodeKind::Case,
                "库存不足",
                "",
                "",
                &RequestSnapshot::default(),
            )
            .unwrap();

        let nodes = store.list_collection_nodes().unwrap();
        let groups = build_groups_from_nodes(&nodes);

        // The endpoint loads as a single request; its two cases become scenarios
        // and are not promoted to standalone requests.
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].requests.len(), 1);
        let request = &groups[0].requests[0];
        assert_eq!(request.title, "下单");
        assert_eq!(request.scenarios.len(), 2);
        let names: Vec<&str> = request.scenarios.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"正常下单"));
        assert!(names.contains(&"库存不足"));
        assert_eq!(request.scenarios[0].node_id, "case-1");
        let scenario_request = request.scenarios[0]
            .request
            .as_deref()
            .expect("scenario carries request variant");
        assert_eq!(scenario_request.node_id, "case-1");
        assert_eq!(scenario_request.title, "正常下单");
        assert_eq!(scenario_request.path, "/orders");
    }

    #[test]
    fn create_case_copies_parent_request_snapshot() {
        let service = service_with_store(temp_store());
        let parent = service
            .create_endpoint(None, "登录", "POST", "/login")
            .expect("endpoint");
        let mut request = ApiRequest {
            node_id: parent.id.clone(),
            title: "登录".into(),
            method: HttpMethod::Post,
            path: "/login".into(),
            params: vec![KeyValueRow::new("debug", "1")],
            path_rows: Vec::new(),
            body: r#"{"account":"demo"}"#.into(),
            body_mode: BodyMode::Json,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: vec![KeyValueRow::new("X-App", "qingqi")],
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: "set token=abc".into(),
            post_ops: "status == 200".into(),
            scenarios: Vec::new(),
        };
        service
            .persist_endpoint_snapshot("登录", "POST", "/login", &request)
            .expect("persist endpoint");

        let case = service.create_case(&parent.id, "错误密码").expect("case");
        let snapshot = &case.request;
        assert_eq!(case.parent_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(case.method, "POST");
        assert_eq!(case.url, "/login");
        assert_eq!(snapshot.body_text, r#"{"account":"demo"}"#);
        assert_eq!(snapshot.pre_ops_text, "set token=abc");
        assert_eq!(snapshot.post_ops_text, "status == 200");

        request.node_id = case.id.clone();
        request.title = "错误密码".into();
        request.body = r#"{"account":"wrong"}"#.into();
        service
            .persist_endpoint_snapshot("错误密码", "POST", "/login", &request)
            .expect("persist case variant");
        let updated_case = service
            .get_collection_node(&case.id)
            .unwrap()
            .expect("case node");
        let updated_snapshot = &updated_case.request;
        assert_eq!(updated_case.name, "错误密码");
        assert_eq!(updated_snapshot.body_text, r#"{"account":"wrong"}"#);
    }

    #[test]
    fn build_groups_nested_folders_preserve_hierarchy() {
        let store = temp_store();

        store
            .create_collection_node(
                "folder-a",
                None,
                NodeKind::Folder,
                "API",
                "",
                "",
                &RequestSnapshot::default(),
            )
            .unwrap();
        store
            .create_collection_node(
                "folder-b",
                Some("folder-a"),
                NodeKind::Folder,
                "Users",
                "",
                "",
                &RequestSnapshot::default(),
            )
            .unwrap();
        store
            .create_collection_node(
                "ep-deep",
                Some("folder-b"),
                NodeKind::Endpoint,
                "/user/list",
                "GET",
                "/api/users",
                &RequestSnapshot::default(),
            )
            .unwrap();

        let nodes = store.list_collection_nodes().unwrap();
        let groups = build_groups_from_nodes(&nodes);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "API");
        assert_eq!(groups[0].requests.len(), 0);
        assert_eq!(groups[0].folders.len(), 1);
        assert_eq!(groups[0].folders[0].name, "Users");
        assert_eq!(groups[0].folders[0].requests.len(), 1);
        assert_eq!(groups[0].folders[0].requests[0].title, "/user/list");
    }

    #[test]
    fn snapshot_roundtrip_via_request_to_snapshot() {
        let request = ApiRequest {
            node_id: String::new(),
            title: String::from("test"),
            method: HttpMethod::Post,
            path: String::from("/api/test"),
            params: vec![KeyValueRow::new("page", "1")],
            path_rows: vec![KeyValueRow::new("id", "42")],
            body: String::from(r#"{"key":"val"}"#),
            body_mode: BodyMode::Json,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: vec![KeyValueRow::new("Content-Type", "application/json")],
            cookies: vec![KeyValueRow::new("sid", "abc")],
            auth: vec![KeyValueRow::new("Authorization", "Bearer tok")],
            pre_ops: String::from("set x=1"),
            post_ops: String::from("extract id=$.id"),
            scenarios: Vec::new(),
        };

        let snapshot = request_to_snapshot("POST", "/api/test", &request);
        assert_eq!(snapshot.method, "POST");
        assert_eq!(snapshot.url, "/api/test");
        assert!(snapshot.params_text.contains("page=1"));
        assert!(snapshot.path_params_text.contains("id=42"));
        assert!(
            snapshot
                .headers_text
                .contains("Content-Type=application/json")
        );
        assert!(snapshot.cookies_text.contains("sid=abc"));
        assert_eq!(snapshot.auth_type, "bearer");
        assert_eq!(snapshot.auth_value, "tok");
        assert_eq!(snapshot.body_text, r#"{"key":"val"}"#);
        assert_eq!(snapshot.body_mode, "json");
        assert_eq!(snapshot.pre_ops_text, "set x=1");
        assert_eq!(snapshot.post_ops_text, "extract id=$.id");
    }

    #[test]
    fn code_snippet_includes_auth_cookies_and_query() {
        let environment = ApiEnvironment {
            name: "Test".into(),
            badge: "T".into(),
            color: 0,
            base_url: "https://api.example.com".into(),
            variables: Vec::new(),
            headers: vec![KeyValueRow::new("X-Env", "envval")],
        };
        let mut apikey = KeyValueRow::new("api_key", "secret");
        apikey.description = "query".into();
        let request = ApiRequest {
            node_id: String::new(),
            title: "t".into(),
            method: HttpMethod::Get,
            path: "/users".into(),
            params: vec![KeyValueRow::new("page", "2")],
            path_rows: Vec::new(),
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: vec![KeyValueRow::new("Accept", "application/json")],
            cookies: vec![KeyValueRow::new("sid", "abc")],
            auth: vec![apikey],
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };

        let curl = code_snippet(&environment, &request, crate::code_gen::CodeLanguage::Curl);
        assert!(
            curl.contains("https://api.example.com/users"),
            "url: {curl}"
        );
        assert!(curl.contains("page=2"), "params: {curl}");
        assert!(curl.contains("api_key=secret"), "query auth: {curl}");
        assert!(curl.contains("Accept: application/json"), "header: {curl}");
        assert!(curl.contains("X-Env: envval"), "env header: {curl}");
        assert!(curl.contains("Cookie: sid=abc"), "cookie: {curl}");
    }

    #[test]
    fn code_snippet_includes_enabled_path_rows() {
        let environment = ApiEnvironment {
            name: "Test".into(),
            badge: "T".into(),
            color: 0,
            base_url: "https://api.example.com".into(),
            variables: vec![KeyValueRow::new("USER_ID", "42")],
            headers: Vec::new(),
        };
        let mut disabled = KeyValueRow::new("ignored", "disabled");
        disabled.enabled = false;
        let request = ApiRequest {
            node_id: String::new(),
            title: "t".into(),
            method: HttpMethod::Get,
            path: "/users".into(),
            params: Vec::new(),
            path_rows: vec![KeyValueRow::new("id", "{{USER_ID}}"), disabled],
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };

        let curl = code_snippet(&environment, &request, crate::code_gen::CodeLanguage::Curl);
        assert!(
            curl.contains("https://api.example.com/users/42"),
            "url: {curl}"
        );
        assert!(!curl.contains("disabled"), "url: {curl}");
    }

    #[test]
    fn request_path_with_segments_appends_enabled_rows() {
        let mut disabled = KeyValueRow::new("disabled", "off");
        disabled.enabled = false;
        let request = ApiRequest {
            node_id: String::new(),
            title: "t".into(),
            method: HttpMethod::Get,
            path: "/api".into(),
            params: Vec::new(),
            path_rows: vec![KeyValueRow::new("version", "/v1/"), disabled],
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };

        let path = request_path_with_segments(&request, |value| value.to_string());
        assert_eq!(path.unwrap(), "/api/v1");
    }

    #[test]
    fn code_snippet_form_urlencoded_uses_form_data() {
        let environment = ApiEnvironment {
            name: "Test".into(),
            badge: "T".into(),
            color: 0,
            base_url: "https://h.example.com".into(),
            variables: Vec::new(),
            headers: Vec::new(),
        };
        let request = ApiRequest {
            node_id: String::new(),
            title: "t".into(),
            method: HttpMethod::Post,
            path: "/submit".into(),
            params: Vec::new(),
            path_rows: Vec::new(),
            body: "a=1\nb=2".into(),
            body_mode: BodyMode::FormUrlEncoded,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };

        let curl = code_snippet(&environment, &request, crate::code_gen::CodeLanguage::Curl);
        assert!(curl.contains("--data-raw"), "data flag: {curl}");
        assert!(curl.contains("a=1&b=2"), "form body: {curl}");
    }

    #[test]
    fn build_form_urlencoded_body_uses_enabled_kv_lines() {
        let body = build_form_urlencoded_body("a=1\n# disabled=x\nb=2");
        assert_eq!(body, "a=1&b=2");
    }

    #[test]
    fn persist_endpoint_roundtrip() {
        let store = temp_store();

        store
            .create_collection_node(
                "ep-persist",
                None,
                NodeKind::Endpoint,
                "/api/test",
                "GET",
                "/api/test",
                &RequestSnapshot::default(),
            )
            .unwrap();

        let request = ApiRequest {
            node_id: String::new(),
            title: String::from("/api/test"),
            method: HttpMethod::Post,
            path: String::from("/api/test/v2"),
            params: vec![KeyValueRow::new("id", "1")],
            path_rows: Vec::new(),
            body: String::from(r#"{"data": true}"#),
            body_mode: BodyMode::Json,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: vec![KeyValueRow::new("X-Test", "yes")],
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };

        let snapshot = request_to_snapshot("POST", "/api/test/v2", &request);
        store
            .update_collection_node("ep-persist", "/api/test", "POST", "/api/test/v2", &snapshot)
            .unwrap();

        let node = store.get_collection_node("ep-persist").unwrap().unwrap();
        assert_eq!(node.method, "POST");
        assert_eq!(node.url, "/api/test/v2");

        let restored = &node.request;
        assert_eq!(restored.method, "POST");
        assert_eq!(restored.url, "/api/test/v2");
        assert!(restored.body_text.contains("data"));
    }

    #[test]
    fn default_environments_are_honest() {
        let envs = default_environments();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "默认环境");
        assert_eq!(envs[0].base_url, "http://127.0.0.1:8000");
    }

    #[test]
    fn parse_kv_and_format_roundtrip() {
        let text = "key1=value1\nkey2=value2\n\nkey3=val=ue3";
        let rows = parse_kv_text(text);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].key, "key1");
        assert_eq!(rows[0].value, "value1");
        assert_eq!(rows[2].value, "val=ue3");

        let formatted = format_kv_rows(&rows);
        assert!(formatted.contains("key1=value1"));
        assert!(formatted.contains("key3=val=ue3"));
    }

    #[test]
    fn http_method_from_label() {
        assert_eq!(HttpMethod::from_label("get"), HttpMethod::Get);
        assert_eq!(HttpMethod::from_label("POST"), HttpMethod::Post);
        assert_eq!(HttpMethod::from_label("Put"), HttpMethod::Put);
        assert_eq!(HttpMethod::from_label("PATCH"), HttpMethod::Patch);
        assert_eq!(HttpMethod::from_label("delete"), HttpMethod::Delete);
        assert_eq!(HttpMethod::from_label("DEL"), HttpMethod::Delete);
        assert_eq!(HttpMethod::from_label("unknown"), HttpMethod::Get);
    }

    #[test]
    fn http_method_head_options_and_allows_body() {
        assert_eq!(HttpMethod::from_label("HEAD"), HttpMethod::Head);
        assert_eq!(HttpMethod::from_label("options"), HttpMethod::Options);
        assert_eq!(HttpMethod::Head.label(), "HEAD");
        assert_eq!(HttpMethod::Options.label(), "OPTIONS");
        assert!(!HttpMethod::Get.allows_body());
        assert!(!HttpMethod::Head.allows_body());
        assert!(HttpMethod::Post.allows_body());
        assert!(HttpMethod::Put.allows_body());
        assert!(HttpMethod::Delete.allows_body());
        assert!(HttpMethod::Options.allows_body());
    }

    #[test]
    fn default_content_type_maps_body_modes() {
        assert_eq!(
            default_content_type(BodyMode::Json),
            Some("application/json")
        );
        assert_eq!(default_content_type(BodyMode::Xml), Some("application/xml"));
        assert_eq!(
            default_content_type(BodyMode::FormUrlEncoded),
            Some("application/x-www-form-urlencoded")
        );
        assert!(default_content_type(BodyMode::Text).is_some());
        assert_eq!(default_content_type(BodyMode::None), None);
        assert_eq!(default_content_type(BodyMode::Binary), None);
        assert_eq!(default_content_type(BodyMode::FormData), None);
    }

    #[test]
    fn build_multipart_body_encodes_text_fields() {
        let body = build_multipart_body("name=alice\n# disabled=x\nrole=admin", "BOUNDARY")
            .expect("multipart builds");
        let text = String::from_utf8(body).expect("utf8");
        assert!(text.starts_with("--BOUNDARY\r\n"));
        assert!(text.contains("Content-Disposition: form-data; name=\"name\"\r\n\r\nalice\r\n"));
        assert!(text.contains("Content-Disposition: form-data; name=\"role\"\r\n\r\nadmin\r\n"));
        // Disabled rows are skipped.
        assert!(!text.contains("disabled"));
        // Terminates with the closing boundary.
        assert!(text.ends_with("--BOUNDARY--\r\n"));
    }

    #[test]
    fn auth_extract_and_format_roundtrip() {
        let auth_rows = vec![KeyValueRow::new("Authorization", "Bearer my-token")];
        assert_eq!(extract_auth_type(&auth_rows), "bearer");
        assert_eq!(extract_auth_value(&auth_rows), "my-token");

        let formatted = format_auth("bearer", "my-token");
        assert_eq!(formatted, "Authorization=Bearer my-token");

        let parsed = parse_kv_text(&formatted);
        assert_eq!(extract_auth_type(&parsed), "bearer");
        assert_eq!(extract_auth_value(&parsed), "my-token");
    }

    #[test]
    fn resolve_with_temp_overrides_env_vars() {
        let mut temporary = HashMap::new();
        temporary.insert("TOKEN".into(), "temp-abc".into());
        let mut env_vars = HashMap::new();
        env_vars.insert("TOKEN".into(), "env-xyz".into());

        let result = resolve_with_temp("Bearer {{TOKEN}}", &temporary, &env_vars);
        assert_eq!(result, "Bearer temp-abc");
    }

    #[test]
    fn resolve_with_temp_falls_back_to_env() {
        let temporary = HashMap::new();
        let mut env_vars = HashMap::new();
        env_vars.insert("HOST".into(), "example.com".into());

        let result = resolve_with_temp("http://{{HOST}}/api", &temporary, &env_vars);
        assert_eq!(result, "http://example.com/api");
    }

    #[test]
    fn extract_then_resolve_chains_value() {
        // Mirror the send path: a request `extract`s a value from its response,
        // it is persisted to the environment store, and the next request resolves
        // a template that references it.
        let extracted = script_service::extract_variables(
            "extract token=$.data.token",
            r#"{"data":{"token":"t-42"}}"#,
        );
        assert_eq!(extracted.get("token"), Some(&"t-42".to_string()));

        let store: Vec<ApiVariable> = extracted
            .iter()
            .map(|(k, v)| ApiVariable {
                scope: VariableScope::Environment,
                env_name: "Dev".into(),
                var_key: k.clone(),
                var_value: v.clone(),
                updated_at: String::new(),
            })
            .collect();

        let resolved = variable_service::resolve_text(
            "Bearer {{token}}",
            &HashMap::new(),
            &HashMap::new(),
            &store,
            &HashMap::new(),
            &[],
            &HashMap::new(),
            &[],
        );
        assert_eq!(resolved, "Bearer t-42");
    }

    #[test]
    fn pre_ops_draft_modifies_url_headers_body() {
        let mut draft = script_service::RequestDraft {
            method: "GET".into(),
            url: "/api/v1/users".into(),
            params: HashMap::new(),
            headers: HashMap::new(),
            body: String::new(),
        };
        let temp = script_service::apply_pre_ops(
            &mut draft,
            "set token=abc123\nheader Authorization: Bearer {{token}}\nquery page=1",
        );
        assert_eq!(temp.get("token").unwrap(), "abc123");
        assert_eq!(
            draft.headers.get("Authorization").unwrap(),
            "Bearer {{token}}"
        );
        assert_eq!(draft.params.get("page").unwrap(), "1");
    }

    #[test]
    fn history_persisted_after_send() {
        let store = temp_store();
        let id = store
            .insert_history(
                "tab-send",
                "GET",
                "/api/test",
                200,
                "200 OK",
                r#"{"ok":true}"#,
            )
            .unwrap();
        assert!(id > 0);

        let history = store.list_history("tab-send", 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].method, "GET");
        assert_eq!(history[0].url, "/api/test");
        assert_eq!(history[0].status, 200);
        assert_eq!(history[0].title, "200 OK");
        assert!(history[0].response.contains("ok"));
    }

    #[test]
    fn script_assertion_results_in_response() {
        use crate::script_service;

        let body = r#"{"code":200,"data":{"id":42}}"#;
        let assertions = "status == 200\njson $.code == 200\nbody contains 'data'";
        let results = script_service::run_assertions(assertions, 200, body);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, passed)| *passed));

        let results = script_service::run_assertions("status == 404", 200, body);
        assert_eq!(results.len(), 1);
        assert!(!results[0].1);
    }

    #[test]
    fn format_kv_rows_filters_empty_keys() {
        let rows = vec![
            KeyValueRow {
                enabled: true,
                key: "valid".into(),
                value: "yes".into(),
                value_type: String::new(),
                description: String::new(),
            },
            KeyValueRow {
                enabled: true,
                key: String::new(),
                value: "no-key".into(),
                value_type: String::new(),
                description: String::new(),
            },
            KeyValueRow {
                enabled: true,
                key: "also".into(),
                value: "ok".into(),
                value_type: String::new(),
                description: String::new(),
            },
        ];
        let formatted = format_kv_rows(&rows);
        assert!(formatted.contains("valid=yes"));
        assert!(formatted.contains("also=ok"));
        assert!(!formatted.contains("no-key"));
    }

    #[test]
    fn kv_text_roundtrip_preserves_disabled_rows() {
        let rows = vec![
            KeyValueRow {
                enabled: true,
                key: "Accept".into(),
                value: "application/json".into(),
                value_type: String::new(),
                description: String::new(),
            },
            KeyValueRow {
                enabled: false,
                key: "X-Debug".into(),
                value: "1".into(),
                value_type: String::new(),
                description: String::new(),
            },
        ];
        let text = format_kv_rows(&rows);
        assert_eq!(text, "Accept=application/json\n# X-Debug=1");

        let restored = parse_kv_text(&text);
        assert_eq!(restored.len(), 2);
        assert!(restored[0].enabled);
        assert_eq!(restored[0].key, "Accept");
        assert!(!restored[1].enabled);
        assert_eq!(restored[1].key, "X-Debug");
        assert_eq!(restored[1].value, "1");
    }

    #[test]
    fn kv_text_roundtrip_preserves_type_and_description() {
        let rows = vec![KeyValueRow {
            enabled: true,
            key: "page".into(),
            value: "1".into(),
            value_type: "number".into(),
            description: "页码".into(),
        }];
        let text = format_kv_rows(&rows);
        assert_eq!(text, "page=1\tnumber\t页码");

        let restored = parse_kv_text(&text);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].value_type, "number");
        assert_eq!(restored[0].description, "页码");
    }

    #[test]
    fn base64_encode_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        // The canonical Basic-auth example.
        assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn base64_roundtrip_basic_auth() {
        for cred in ["user:pass", "admin:s3cr3t!", "a:", ":b", "用户:密码"] {
            let encoded = base64_encode(cred.as_bytes());
            let decoded = base64_decode(&encoded).expect("valid base64");
            assert_eq!(decoded, cred.as_bytes());
        }
    }

    #[test]
    fn service_create_environment() {
        let store = temp_store();
        let service = ApiService {
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                pending_groups: None,
                pending_environments: None,
            }),
            update_subscribers: Mutex::new(Vec::new()),
            data_source: store,
            generation: AtomicU64::new(0),
        };
        let env = service
            .create_environment("测试环境", "http://test.api.com")
            .unwrap();
        assert_eq!(env.name, "测试环境");
        assert_eq!(env.base_url, "http://test.api.com");
        assert_eq!(env.badge, "测");
    }

    #[test]
    fn service_duplicate_environment() {
        let store = temp_store();
        let service = ApiService {
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                pending_groups: None,
                pending_environments: None,
            }),
            update_subscribers: Mutex::new(Vec::new()),
            data_source: store,
            generation: AtomicU64::new(0),
        };
        // Index 0 is the seeded default environment
        let dup = service.duplicate_environment(0).unwrap();
        assert_eq!(dup.name, "默认环境 副本");
        assert_eq!(dup.base_url, "http://127.0.0.1:8000");
        assert_eq!(dup.variables.len(), 3); // seeded variables copied
    }

    #[test]
    fn service_delete_environment_fails_if_last() {
        let store = temp_store();
        let service = ApiService {
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                pending_groups: None,
                pending_environments: None,
            }),
            update_subscribers: Mutex::new(Vec::new()),
            data_source: store,
            generation: AtomicU64::new(0),
        };
        // Only one seeded env — cannot delete
        let result = service.delete_environment_by_index(0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("至少保留一个环境"));
    }

    #[test]
    fn service_delete_environment_succeeds_with_multiple() {
        let store = temp_store();
        let service = ApiService {
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                pending_groups: None,
                pending_environments: None,
            }),
            update_subscribers: Mutex::new(Vec::new()),
            data_source: store,
            generation: AtomicU64::new(0),
        };
        service
            .create_environment("额外环境", "http://extra.com")
            .unwrap();
        // Now we have 2 envs; delete the second one
        assert!(service.delete_environment_by_index(1).unwrap());
    }

    #[test]
    fn service_save_environment_fields() {
        let store = temp_store();
        let service = ApiService {
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                pending_groups: None,
                pending_environments: None,
            }),
            update_subscribers: Mutex::new(Vec::new()),
            data_source: store,
            generation: AtomicU64::new(0),
        };
        service
            .save_environment_fields(
                0,
                "更新后环境",
                "http://updated.com",
                "KEY1=val1\nKEY2=val2",
                "Authorization=Bearer tok",
            )
            .unwrap();
        let envs = service.list_environments_ui();
        assert_eq!(envs[0].name, "更新后环境");
        assert_eq!(envs[0].base_url, "http://updated.com");
        assert_eq!(envs[0].variables.len(), 2);
        assert_eq!(envs[0].headers.len(), 1);
    }

    #[test]
    fn global_variables_can_be_replaced() {
        let service = service_with_store(temp_store());
        service
            .save_global_variables(&[
                KeyValueRow::new("TOKEN", "first"),
                KeyValueRow::new("HOST", "localhost"),
            ])
            .unwrap();
        service
            .save_global_variables(&[
                KeyValueRow::new("TOKEN", "updated"),
                KeyValueRow::new("", "ignored"),
            ])
            .unwrap();

        let rows = service.list_global_variables();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "TOKEN");
        assert_eq!(rows[0].value, "updated");
    }

    #[test]
    fn environment_export_import_json_roundtrip() {
        let service = service_with_store(temp_store());
        service
            .save_environment_fields(
                0,
                "本地",
                "http://localhost:3000",
                "TOKEN=abc\n# DISABLED=no",
                "Accept=application/json",
            )
            .unwrap();

        let json = service.export_environments_json().unwrap();
        assert!(json.contains("\"version\""));
        assert!(json.contains("localhost:3000"));

        let imported = service_with_store(temp_store());
        let count = imported.import_environments_json(&json).unwrap();
        assert_eq!(count, 1);

        let envs = imported.list_environments_ui();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "本地");
        assert_eq!(envs[0].base_url, "http://localhost:3000");
        assert_eq!(envs[0].variables[0].key, "TOKEN");
        assert!(!envs[0].variables[1].enabled);
        assert_eq!(envs[0].headers[0].key, "Accept");
    }

    #[test]
    fn public_echo_files_import_through_service() {
        let service = service_with_store(temp_store());
        let collection = include_str!("../examples/postman-echo-scenarios.postman_collection.json");
        let environments = include_str!("../examples/postman-echo-environments.json");

        let imported = service.import_from_postman(collection).unwrap();
        assert_eq!(imported.len(), 14);
        let groups = service.build_collection_tree().unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "基础请求");
        assert_eq!(groups[0].requests.len(), 8);
        assert_eq!(groups[1].name, "请求体与方法");
        assert_eq!(groups[1].requests.len(), 6);

        assert_eq!(service.import_environments_json(environments).unwrap(), 1);
        let imported_environments = service.list_environments_ui();
        assert_eq!(imported_environments.len(), 1);
        assert_eq!(
            imported_environments[0].base_url,
            "https://postman-echo.com"
        );
        assert!(
            imported_environments[0]
                .variables
                .iter()
                .any(|variable| variable.key == "SEARCH_TERM" && variable.value == "qingqi")
        );
    }

    #[test]
    fn parse_kv_lines_parses_and_filters() {
        let result = parse_kv_lines("KEY1=val1\n\n# KEY2=val2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, "KEY1");
        assert_eq!(result[0].2, "val1");
        assert!(result[0].0); // enabled
        assert_eq!(result[1].1, "KEY2");
        assert!(!result[1].0);
    }

    #[test]
    fn detect_body_mode_detects_json() {
        assert_eq!(detect_body_mode(r#"{"key": "value"}"#), "json");
        assert_eq!(detect_body_mode("[1, 2, 3]"), "json");
        assert_eq!(detect_body_mode("  {\"a\":1}  "), "json");
        assert_eq!(detect_body_mode(""), "none");
        assert_eq!(detect_body_mode("   "), "none");
        assert_eq!(detect_body_mode("plain text body"), "text");
        assert_eq!(detect_body_mode("key=value&foo=bar"), "text");
    }

    #[test]
    fn format_auth_for_input_unknown_type_uppercases_key() {
        let text = format_auth_for_input("oauth2", "tok");
        assert_eq!(text, "OAUTH2=tok");
    }

    // ── Query parameter encoding regression tests (FIX-016) ──

    /// Verify that special characters in query keys/values are percent-encoded
    /// and survive a round-trip through `Url::parse`. Compares parsed query
    /// pairs rather than fragile strings.
    #[test]
    fn query_params_special_chars_roundtrip() {
        let cases = vec![
            ("a b", "space in key"),
            ("a&b", "ampersand in key"),
            ("key", "x=y"),
            ("key", "中文"),
            ("key", "+%#?"),
            ("key", ""),
        ];
        for (raw_key, raw_value) in cases {
            let base = "https://example.com/path";
            let encoded =
                append_query_params(base, &[(raw_key.to_string(), raw_value.to_string())]);
            let parsed = Url::parse(&encoded).expect("encoded URL is valid");
            let pairs: Vec<_> = parsed.query_pairs().collect();
            assert_eq!(
                pairs.len(),
                1,
                "single pair for ({raw_key}, {raw_value}): {encoded}"
            );
            assert_eq!(pairs[0].0, raw_key, "key roundtrip for {raw_key}");
            assert_eq!(pairs[0].1, raw_value, "value roundtrip for {raw_value}");
        }
    }

    /// Multiple query params each with special characters must all survive
    /// encoding without merging or losing any.
    #[test]
    fn query_params_multiple_special_pairs() {
        let params: Vec<(String, String)> = vec![
            ("filter".to_string(), "a&b".to_string()),
            ("q".to_string(), "hello world".to_string()),
            ("expr".to_string(), "x=y".to_string()),
        ];
        let encoded = append_query_params("https://api.test/search", &params);
        let parsed = Url::parse(&encoded).expect("valid url");
        let pairs: Vec<_> = parsed.query_pairs().collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("filter".into(), "a&b".into()));
        assert_eq!(pairs[1], ("q".into(), "hello world".into()));
        assert_eq!(pairs[2], ("expr".into(), "x=y".into()));
    }

    #[test]
    fn request_url_split_and_compose_preserves_duplicates_and_fragment() {
        let (base, rows) = split_request_url("/search?q=a+b&q=%E4%B8%AD%E6%96%87&empty=#part");
        assert_eq!(base, "/search#part");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].value, "a b");
        assert_eq!(rows[1].value, "中文");
        assert_eq!(compose_request_url(&base, &rows), "/search?q=a+b&q=%E4%B8%AD%E6%96%87&empty=#part");
    }

    #[test]
    fn path_parameter_extraction_ignores_template_variables() {
        assert_eq!(
            extract_path_parameter_names("/{{HOST}}/users/{id}/:section?x=1"),
            vec![String::from("id"), String::from("section")]
        );
    }

    #[test]
    fn request_path_replaces_named_parameters_and_validates_missing_values() {
        let mut request = ApiRequest {
            node_id: String::new(),
            title: String::from("path"),
            method: HttpMethod::Get,
            path: String::from("/users/{id}/:section"),
            params: Vec::new(),
            path_rows: vec![
                KeyValueRow::new("id", "a b"),
                KeyValueRow::new("section", "detail"),
            ],
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("path"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };
        assert_eq!(
            request_path_with_segments(&request, str::to_string).unwrap(),
            "/users/a%20b/detail"
        );
        request.path_rows[0].enabled = false;
        assert!(
            request_path_with_segments(&request, str::to_string)
                .unwrap_err()
                .to_string()
                .contains("id")
        );
    }

    /// Query-located auth must go through the same encoding path as regular
    /// params so `&`/`=` in the value don't corrupt the URL.
    #[test]
    fn code_snippet_query_auth_encoded() {
        let environment = ApiEnvironment {
            name: "T".into(),
            badge: "T".into(),
            color: 0,
            base_url: "https://h.example.com".into(),
            variables: Vec::new(),
            headers: Vec::new(),
        };
        let mut apikey = KeyValueRow::new("sig", "a=b&c");
        apikey.description = "query".into();
        let request = ApiRequest {
            node_id: String::new(),
            title: "t".into(),
            method: HttpMethod::Get,
            path: "/endpoint".into(),
            params: vec![KeyValueRow::new("normal", "v")],
            path_rows: Vec::new(),
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: vec![apikey],
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };
        let curl = code_snippet(&environment, &request, crate::code_gen::CodeLanguage::Curl);
        let url_str = curl
            .lines()
            .find_map(|line| {
                let trimmed = line.trim();
                let inner = trimmed.strip_prefix("\"")?.strip_suffix("\"")?;
                Some(inner.to_string())
            })
            .expect("url line in snippet");
        let parsed = Url::parse(&url_str).expect("valid url in snippet");
        let sig = parsed
            .query_pairs()
            .find(|(k, _)| k == "sig")
            .map(|(_, v)| v.to_string())
            .expect("sig param present");
        assert_eq!(sig, "a=b&c");
    }

    /// form_urlencoded body must encode special characters, including `&`,
    /// `=`, spaces, and non-ASCII bytes.
    #[test]
    fn build_form_urlencoded_encodes_special_chars() {
        let body =
            build_form_urlencoded_body("name=alice bob\nfilter=a&b\nexpr=x=y\nemoji=中文\nempty=");
        let parsed: Vec<(String, String)> = url::form_urlencoded::parse(body.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(parsed.len(), 5);
        assert_eq!(parsed[0], ("name".into(), "alice bob".into()));
        assert_eq!(parsed[1], ("filter".into(), "a&b".into()));
        assert_eq!(parsed[2], ("expr".into(), "x=y".into()));
        assert_eq!(parsed[3], ("emoji".into(), "中文".into()));
        assert_eq!(parsed[4], ("empty".into(), "".into()));
    }

    /// build_final_url must encode query params via the structured API and
    /// round-trip cleanly through `Url`.
    #[test]
    fn build_final_url_encodes_query_params() {
        let environment = ApiEnvironment {
            name: "T".into(),
            badge: "T".into(),
            color: 0,
            base_url: "https://api.example.com".into(),
            variables: Vec::new(),
            headers: Vec::new(),
        };
        let request = ApiRequest {
            node_id: String::new(),
            title: "t".into(),
            method: HttpMethod::Get,
            path: "/search".into(),
            params: vec![
                KeyValueRow::new("q", "a b"),
                KeyValueRow::new("filter", "x=y"),
                KeyValueRow::new("tag", "中文"),
            ],
            path_rows: Vec::new(),
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };
        let url = build_final_url(&environment, &request);
        let parsed = Url::parse(&url).expect("valid url");
        assert_eq!(parsed.path(), "/search");
        let pairs: Vec<_> = parsed.query_pairs().collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("q".into(), "a b".into()));
        assert_eq!(pairs[1], ("filter".into(), "x=y".into()));
        assert_eq!(pairs[2], ("tag".into(), "中文".into()));
    }

    // ── URL sync edge cases ──

    #[test]
    fn url_sync_preserves_fragment_only() {
        let (base, rows) = split_request_url("/api/users#section-1");
        assert_eq!(base, "/api/users#section-1");
        assert!(rows.is_empty());
        let composed = compose_request_url(&base, &rows);
        assert_eq!(composed, "/api/users#section-1");
    }

    #[test]
    fn url_sync_preserves_empty_query_value() {
        let (base, rows) = split_request_url("/api/search?q=&page=1");
        assert_eq!(base, "/api/search");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].key, "q");
        assert_eq!(rows[0].value, "");
        assert_eq!(rows[1].key, "page");
        assert_eq!(rows[1].value, "1");
        let composed = compose_request_url(&base, &rows);
        assert_eq!(composed, "/api/search?q=&page=1");
    }

    #[test]
    fn url_sync_handles_relative_url_with_query() {
        let (base, rows) = split_request_url("api/v2/items?category=books&sort=asc");
        assert_eq!(base, "api/v2/items");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].key, "category");
        assert_eq!(rows[0].value, "books");
    }

    #[test]
    fn url_sync_handles_duplicate_query_keys() {
        let (base, rows) = split_request_url("/api/filter?tag=a&tag=b&tag=c");
        assert_eq!(base, "/api/filter");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].value, "a");
        assert_eq!(rows[1].value, "b");
        assert_eq!(rows[2].value, "c");
        let composed = compose_request_url(&base, &rows);
        assert_eq!(composed, "/api/filter?tag=a&tag=b&tag=c");
    }

    #[test]
    fn merge_url_query_rows_preserves_disabled_rows() {
        let existing = vec![
            KeyValueRow::new("a", "old"),
            KeyValueRow {
                enabled: false,
                key: "b".into(),
                value: "disabled".into(),
                value_type: String::new(),
                description: String::new(),
            },
        ];
        let parsed = vec![
            KeyValueRow::new("a", "new"),
            KeyValueRow::new("c", "added"),
        ];
        let merged = merge_url_query_rows(&existing, parsed);
        // a should be updated to new value, b should remain disabled, c added
        assert_eq!(merged.len(), 3);
        assert!(merged.iter().any(|r| r.key == "a" && r.value == "new"));
        assert!(merged.iter().any(|r| r.key == "b" && !r.enabled));
        assert!(merged.iter().any(|r| r.key == "c" && r.value == "added"));
    }

    // ── Path parameter edge cases ──

    #[test]
    fn path_param_missing_blocks_send() {
        let request = ApiRequest {
            node_id: String::new(),
            title: String::from("test"),
            method: HttpMethod::Get,
            path: String::from("/users/{id}"),
            params: Vec::new(),
            path_rows: vec![KeyValueRow {
                enabled: true,
                key: "id".into(),
                value: String::new(),
                value_type: String::new(),
                description: String::new(),
            }],
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };
        let err = request_path_with_segments(&request, str::to_string).unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn path_param_disabled_blocks_send() {
        let request = ApiRequest {
            node_id: String::new(),
            title: String::from("test"),
            method: HttpMethod::Get,
            path: String::from("/users/{id}/posts/{post_id}"),
            params: Vec::new(),
            path_rows: vec![
                KeyValueRow::new("id", "123"),
                KeyValueRow {
                    enabled: false,
                    key: "post_id".into(),
                    value: "456".into(),
                    value_type: String::new(),
                    description: String::new(),
                },
            ],
            body: String::new(),
            body_mode: BodyMode::None,
            body_payloads: RequestBody::default(),
            editor_tab: String::from("params"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };
        let err = request_path_with_segments(&request, str::to_string).unwrap_err();
        assert!(err.to_string().contains("post_id"));
    }

    #[test]
    fn path_param_ignores_double_braces_as_variable() {
        let names = extract_path_parameter_names("/api/{{HOST}}/{env}/:version");
        assert_eq!(names, vec!["env", "version"]);
    }

    // ── Request panel ratio default ──

    #[test]
    fn request_panel_ratio_default_is_40_percent() {
        let service = service_with_store(temp_store());
        let ratio = service.load_request_panel_ratio();
        assert!((ratio - 0.4).abs() < f32::EPSILON);
    }

    // ── cURL import populates structured body ──

    #[test]
    fn curl_import_form_data_populates_structured_body() {
        let service = service_with_store(temp_store());
        let curl = "curl -X POST https://api.example.com/upload -F 'file=@/tmp/test.png' -F 'title=demo'";
        let node = service.import_from_curl(curl).expect("import cURL");
        let snapshot = &node.request;
        assert_eq!(snapshot.body_mode, "formdata");
        assert_eq!(snapshot.body_payloads.form_data.len(), 2);
        assert_eq!(snapshot.body_payloads.form_data[0].key, "file");
        assert_eq!(snapshot.body_payloads.form_data[0].value, "@/tmp/test.png");
        assert_eq!(snapshot.body_payloads.form_data[1].key, "title");
        assert_eq!(snapshot.body_payloads.form_data[1].value, "demo");
    }

    #[test]
    fn curl_import_json_body_populates_structured_json() {
        let service = service_with_store(temp_store());
        let curl = r#"curl -X POST https://api.example.com/users -H 'Content-Type: application/json' -d '{"name":"alice"}'"#;
        let node = service.import_from_curl(curl).expect("import cURL");
        let snapshot = &node.request;
        assert_eq!(snapshot.body_mode, "json");
        assert_eq!(snapshot.body_payloads.json, r#"{"name":"alice"}"#);
    }

    // ── Per-request editor tab and body mode ──

    #[test]
    fn per_request_editor_tab_restored_on_reload() {
        let service = service_with_store(temp_store());
        let node = service
            .create_endpoint(None, "test", "GET", "/api/test")
            .expect("create endpoint");

        // Update the endpoint with body tab and JSON body mode
        let mut request = ApiRequest {
            node_id: node.id.clone(),
            title: "test".into(),
            method: HttpMethod::Get,
            path: "/api/test".into(),
            params: Vec::new(),
            path_rows: Vec::new(),
            body: String::new(),
            body_mode: BodyMode::Json,
            body_payloads: RequestBody {
                json: r#"{"key":"value"}"#.into(),
                ..Default::default()
            },
            editor_tab: String::from("body"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };
        service
            .persist_endpoint_snapshot("test", "GET", "/api/test", &request)
            .expect("persist");

        // Reload and verify
        let groups = service.build_collection_tree().expect("build tree");
        let reloaded = &groups[0].requests[0];
        assert_eq!(reloaded.editor_tab, "body");
        assert_eq!(reloaded.body_mode, BodyMode::Json);
        assert_eq!(reloaded.body_payloads.json, r#"{"key":"value"}"#);

        // Switch to params tab and verify it persists differently
        request.editor_tab = String::from("params");
        request.body_mode = BodyMode::Text;
        request.body_payloads.text = "plain".into();
        service
            .persist_endpoint_snapshot("test", "GET", "/api/test", &request)
            .expect("persist");

        let groups = service.build_collection_tree().expect("build tree");
        let reloaded = &groups[0].requests[0];
        assert_eq!(reloaded.editor_tab, "params");
        assert_eq!(reloaded.body_mode, BodyMode::Text);
        assert_eq!(reloaded.body_payloads.text, "plain");
        // JSON content should still be preserved from earlier
        assert_eq!(reloaded.body_payloads.json, r#"{"key":"value"}"#);
    }

    // ── response_tab persistence (PluginDictStore) ──

    #[test]
    fn response_tab_has_all_variants() {
        let tabs = [
            (ResponseTab::Body, "body"),
            (ResponseTab::Cookies, "cookies"),
            (ResponseTab::Headers, "headers"),
            (ResponseTab::Request, "request"),
            (ResponseTab::Curl, "curl"),
            (ResponseTab::Logs, "logs"),
            (ResponseTab::History, "history"),
            (ResponseTab::Code, "code"),
        ];
        for (tab, expected) in tabs {
            assert_eq!(tab.as_str(), expected);
            assert_eq!(ResponseTab::from_db(expected), tab);
        }
    }

    // ── Body mode roundtrip ──

    #[test]
    fn body_mode_roundtrip_all_variants() {
        for mode in BodyMode::all() {
            let s = mode.as_str();
            assert_eq!(BodyMode::from_db(s), mode, "roundtrip failed for {:?}", mode);
        }
    }

    // ── Send only reads active body mode ──

    #[test]
    fn send_ignores_inactive_body_modes() {
        let mut request = ApiRequest {
            node_id: String::new(),
            title: String::from("test"),
            method: HttpMethod::Post,
            path: String::from("/api/test"),
            params: Vec::new(),
            path_rows: Vec::new(),
            body: String::new(),
            body_mode: BodyMode::Json,
            body_payloads: RequestBody {
                json: r#"{"active":true}"#.into(),
                text: "should not be sent".into(),
                xml: "<inactive/>".into(),
                binary_path: "/tmp/ignored.bin".into(),
                urlencoded: vec![KeyValueRow::new("x", "should-not-send")],
                form_data: vec![KeyValueRow::new("f", "should-not-send")],
            },
            editor_tab: String::from("body"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };

        // Verify active_body_text returns only the active mode
        request.body_mode = BodyMode::Json;
        assert_eq!(request.active_body_text(), r#"{"active":true}"#);

        request.body_mode = BodyMode::Text;
        assert_eq!(request.active_body_text(), "should not be sent");

        request.body_mode = BodyMode::Xml;
        assert_eq!(request.active_body_text(), "<inactive/>");

        request.body_mode = BodyMode::None;
        assert!(request.active_body_text().is_empty());
    }
}
