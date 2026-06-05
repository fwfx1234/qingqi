use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Instant,
};

use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    data_source::ApiDebuggerDataSource,
    model::{CollectionNode, HttpTab, NodeKind, RequestSnapshot},
    script_service,
    store::ApiWorkspace,
    variable_service,
};
use qingqi_plugin::{database::DatabaseService, log_error, storage::AppPaths};

// Re-export model types for backward compatibility
pub use crate::model::{
    ApiEnvironment, ApiGroup, ApiRequest, ApiScenario, AuthType, BodyMode, EnvHeader,
    EnvVariable, EnvironmentFull, HttpMethod, KeyValueRow, ScenarioStatus,
};

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
            Self::Headers => "Headers",
            Self::Request => "Request",
            Self::Curl => "cURL",
            Self::Logs => "日志",
            Self::History => "历史",
            Self::Code => "代码",
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
    pub request_dump: String,
    pub curl: String,
    pub logs: Vec<String>,
    pub assertion_results: Vec<(String, bool)>,
}

#[derive(Clone, Debug)]
struct ApiServiceState {
    in_flight: bool,
    pending_response: Option<ApiResponse>,
    pending_error: Option<String>,
    pending_notice: Option<String>,
    last_tab_id: String,
}

pub struct ApiService {
    revision: AtomicU64,
    cancel_flag: AtomicBool,
    state: Mutex<ApiServiceState>,
    data_source: ApiDebuggerDataSource,
}

impl ApiService {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> Self {
        let _ = paths;
        let data_source = ApiDebuggerDataSource::open(database, "api_debugger/main")
            .expect("无法打开 API 调试器数据库");
        Self {
            revision: AtomicU64::new(0),
            cancel_flag: AtomicBool::new(false),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub fn is_in_flight(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.in_flight)
            .unwrap_or(false)
    }

    pub fn cancel_request(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
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

    fn publish_notice(&self, notice: String) {
        if let Ok(mut state) = self.state.lock() {
            state.pending_notice = Some(notice);
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
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

    pub fn persist_endpoint_snapshot(
        &self,
        title: &str,
        method: &str,
        url: &str,
        request: &ApiRequest,
    ) -> Result<()> {
        let nodes = self.data_source.list_collection_nodes()?;
        if let Some(node) = nodes
            .iter()
            .find(|n| n.name == title && n.kind == NodeKind::Endpoint)
        {
            let snapshot = request_to_snapshot(method, url, request);
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
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn save_workspace_async(self: &Arc<Self>, environments: Vec<ApiEnvironment>) {
        let service = Arc::clone(self);
        thread::spawn(move || {
            if let Err(error) = service.save_workspace(&[], &environments) {
                service.publish_notice(format!("工作区保存失败: {error}"));
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
        self.revision.fetch_add(1, Ordering::SeqCst);
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
        thread::spawn(move || match service.create_endpoint(parent_id.as_deref(), &name, &method, &url) {
            Ok(_) => service.publish_notice(format!("已创建端点 {}", name)),
            Err(e) => service.publish_notice(format!("创建端点失败: {e}")),
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
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(node)
    }

    pub fn create_folder_async(self: &Arc<Self>, parent_id: Option<String>, name: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.create_folder(parent_id.as_deref(), &name) {
            Ok(_) => service.publish_notice(format!("已创建分组 {}", name)),
            Err(e) => service.publish_notice(format!("创建分组失败: {e}")),
        });
    }

    pub fn delete_collection_item(&self, node_id: &str) -> Result<usize> {
        let count = self.data_source.delete_collection_node_recursive(node_id)?;
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(count)
    }

    pub fn delete_collection_item_async(self: &Arc<Self>, node_id: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.delete_collection_item(&node_id) {
            Ok(count) => service.publish_notice(format!("已删除 {} 项", count)),
            Err(e) => service.publish_notice(format!("删除失败: {e}")),
        });
    }

    pub fn rename_collection_item(&self, node_id: &str, new_name: &str) -> Result<()> {
        let node = self
            .data_source
            .get_collection_node(node_id)?
            .ok_or_else(|| anyhow!("节点不存在"))?;
        let snapshot = RequestSnapshot::from_json(&node.request_json);
        self.data_source
            .update_collection_node(node_id, new_name, &node.method, &node.url, &snapshot)?;
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn rename_collection_item_async(self: &Arc<Self>, node_id: String, new_name: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.rename_collection_item(&node_id, &new_name) {
            Ok(()) => service.publish_notice(format!("已重命名为 {}", new_name)),
            Err(e) => service.publish_notice(format!("重命名失败: {e}")),
        });
    }

    // ── Import ──

    pub fn import_from_curl(&self, curl_text: &str) -> Result<CollectionNode> {
        let parsed = crate::curl_parser::parse_curl(curl_text)
            .map_err(|e| anyhow!("cURL 解析失败: {e}"))?;
        let id = format!("node-{}", Uuid::new_v4().simple());
        let snapshot = RequestSnapshot {
            method: parsed.method.clone(),
            url: parsed.url.clone(),
            headers_text: parsed
                .headers
                .iter()
                .map(|h| format!("{}={}", h.key, h.value))
                .collect::<Vec<_>>()
                .join("\n"),
            body_text: parsed.body.clone(),
            body_mode: parsed.body_mode.as_str().to_string(),
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
            &parsed.url,
            &snapshot,
        )
    }

    pub fn import_from_curl_async(self: &Arc<Self>, curl_text: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.import_from_curl(&curl_text) {
            Ok(node) => service.publish_notice(format!("已导入 cURL 请求: {}", node.name)),
            Err(e) => service.publish_notice(format!("cURL 导入失败: {e}")),
        });
    }

    pub fn import_from_openapi(&self, content: &str) -> Result<Vec<CollectionNode>> {
        let collection = crate::import_openapi::parse_openapi(content)
            .map_err(|e| anyhow!("OpenAPI 解析失败: {e}"))?;
        let mut nodes = Vec::new();
        for endpoint in &collection.endpoints {
            let id = format!("node-{}", Uuid::new_v4().simple());
            let parent_id = endpoint.parent_folder.as_deref();
            let node = self.data_source.create_collection_node(
                &id,
                parent_id,
                NodeKind::Endpoint,
                &endpoint.name,
                &endpoint.method,
                &endpoint.url,
                &endpoint.snapshot,
            )?;
            nodes.push(node);
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(nodes)
    }

    pub fn import_from_postman(&self, content: &str) -> Result<Vec<CollectionNode>> {
        let collection = crate::import_postman::parse_postman(content)
            .map_err(|e| anyhow!("Postman 解析失败: {e}"))?;
        let mut nodes = Vec::new();
        for endpoint in &collection.endpoints {
            let id = format!("node-{}", Uuid::new_v4().simple());
            let parent_id = endpoint.parent_folder.as_deref();
            let node = self.data_source.create_collection_node(
                &id,
                parent_id,
                NodeKind::Endpoint,
                &endpoint.name,
                &endpoint.method,
                &endpoint.url,
                &endpoint.snapshot,
            )?;
            nodes.push(node);
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(nodes)
    }

    // ── Export ──

    pub fn export_collection_as_openapi(&self) -> Result<String> {
        let nodes = self.data_source.list_collection_nodes()?;
        let mut paths = serde_json::Map::new();
        for node in &nodes {
            if node.kind != NodeKind::Endpoint {
                continue;
            }
            let snapshot = RequestSnapshot::from_json(&node.request_json);
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
        tab_id: &str,
    ) -> Result<()> {
        {
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
            state.last_tab_id = tab_id.to_string();
        }
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.revision.fetch_add(1, Ordering::SeqCst);

        let service = Arc::clone(self);
        let pre_ops = pre_ops_text.to_string();
        let post_ops = post_ops_text.to_string();
        let tid = tab_id.to_string();
        thread::spawn(move || {
            if service.cancel_flag.load(Ordering::SeqCst) {
                if let Ok(mut state) = service.state.lock() {
                    state.in_flight = false;
                    state.pending_notice = Some(String::from("请求已取消"));
                }
                service.revision.fetch_add(1, Ordering::SeqCst);
                return;
            }
            let result = perform_request(&environment, &request, &pre_ops);
            if let Ok(mut state) = service.state.lock() {
                state.in_flight = false;
                match result {
                    Ok((response, _extracted_vars)) => {
                        // Run assertions if post-ops contains assertions
                        let mut resp = response;
                        if !post_ops.is_empty() {
                            let assertion_results = script_service::run_assertions(
                                &post_ops,
                                resp.status_code,
                                &resp.body,
                            );
                            if !assertion_results.is_empty() {
                                let summary =
                                    script_service::format_assertion_results(&assertion_results);
                                resp.logs.push(format!("断言结果:\n{summary}"));
                                resp.assertion_results = assertion_results;
                            }
                        }
                        state.pending_response = Some(resp);
                        state.pending_error = None;
                    }
                    Err(error) => {
                        state.pending_error = Some(error.to_string());
                        state.pending_response = Some(ApiResponse {
                            status_line: String::from("请求失败"),
                            status_code: 0,
                            duration_ms: 0,
                            size_bytes: 0,
                            body: format!("{{\n  \"error\": {:?}\n}}", error.to_string()),
                            headers: String::new(),
                            request_dump: String::new(),
                            curl: String::new(),
                            logs: vec![format!("请求失败: {error}")],
                            assertion_results: Vec::new(),
                        });
                    }
                }
            }

            // Persist history and save tab state
            if let Some(resp) = service.take_pending_response_ref() {
                let title = resp.status_line.clone();
                let method = request.method.label().to_string();
                let url_str = build_final_url(&environment, &request);
                log_error!(
                    service.data_source.insert_history(
                        &tid,
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
            // Save tab state if we have a meaningful tab_id
            if !tid.is_empty() {
                // Preserve existing tab fields so send doesn't overwrite the
                // view-persisted draft state with partial data.
                let existing = service
                    .data_source
                    .list_tabs()
                    .ok()
                    .and_then(|tabs| tabs.into_iter().find(|t| t.id == tid));
                let existing_node_id = existing
                    .as_ref()
                    .and_then(|t| {
                        if t.node_id.is_empty() {
                            None
                        } else {
                            Some(t.node_id.clone())
                        }
                    })
                    .unwrap_or_default();
                let existing_active_tab =
                    existing.as_ref().map(|t| t.active_request_tab).unwrap_or(0);

                let auth_type = extract_auth_type(&request.auth);
                let auth_value = extract_auth_value(&request.auth);

                let tab = crate::model::HttpTab {
                    id: tid.clone(),
                    name: request.title.clone(),
                    method: request.method.label().to_string(),
                    url: request.path.clone(),
                    request_mode: "rest".into(),
                    body_mode: detect_body_mode(&request.body).to_string(),
                    auth_type,
                    auth_value,
                    headers_text: format_kv_rows(&request.headers),
                    cookies_text: format_kv_rows(&request.cookies),
                    body_text: request.body.clone(),
                    params_text: format_kv_rows(&request.params),
                    path_params_text: format_kv_rows(&request.path_rows),
                    pre_ops_text: pre_ops.clone(),
                    post_ops_text: post_ops.clone(),
                    node_id: existing_node_id,
                    active_request_tab: existing_active_tab,
                    updated_at: String::new(),
                };
                log_error!(
                    service.data_source.save_tab(&tab),
                    warn,
                    "保存标签页状态失败"
                );
            }

            service.revision.fetch_add(1, Ordering::SeqCst);
        });

        Ok(())
    }

    fn take_pending_response_ref(&self) -> Option<ApiResponse> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.pending_response.clone())
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
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(result)
    }

    pub fn create_environment_async(self: &Arc<Self>, name: String, base_url: String) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.create_environment(&name, &base_url) {
            Ok(env) => service.publish_notice(format!("已创建环境 {}", env.name)),
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
        self.revision.fetch_add(1, Ordering::SeqCst);
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
                    description: String::new(),
                })
                .collect(),
        })
    }

    pub fn duplicate_environment_async(self: &Arc<Self>, source_index: usize) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.duplicate_environment(source_index) {
            Ok(env) => service.publish_notice(format!("已复制为 {}", env.name)),
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
        if deleted {
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
        Ok(deleted)
    }

    pub fn delete_environment_by_index_async(self: &Arc<Self>, index: usize) {
        let service = Arc::clone(self);
        thread::spawn(move || match service.delete_environment_by_index(index) {
            Ok(true) => service.publish_notice(String::from("已删除环境")),
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
        self.revision.fetch_add(1, Ordering::SeqCst);
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
                Ok(()) => service.publish_notice(format!("已保存环境 {}", name)),
                Err(error) => service.publish_notice(format!("保存环境失败: {error}")),
            }
        });
    }

    // ── Tab persistence ──

    pub fn load_persisted_tabs(&self) -> Vec<HttpTab> {
        self.data_source.list_tabs().unwrap_or_default()
    }

    pub fn save_tab_state(&self, tab: &HttpTab) -> Result<()> {
        self.data_source.save_tab(tab)?;
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn save_tab_state_async(self: &Arc<Self>, tab: HttpTab) {
        let service = Arc::clone(self);
        thread::spawn(move || {
            log_error!(service.save_tab_state(&tab), warn, "保存标签页状态失败");
        });
    }

    pub fn delete_persisted_tab(&self, tab_id: &str) -> Result<bool> {
        let deleted = self.data_source.delete_tab(tab_id)?;
        if deleted {
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
        Ok(deleted)
    }

    pub fn delete_persisted_tab_async(self: &Arc<Self>, tab_id: String) {
        let service = Arc::clone(self);
        thread::spawn(move || {
            log_error!(
                service.delete_persisted_tab(&tab_id),
                warn,
                "删除持久化标签页失败"
            );
        });
    }

    pub fn load_persisted_tab_by_id(&self, tab_id: &str) -> Option<HttpTab> {
        self.data_source
            .list_tabs()
            .unwrap_or_default()
            .into_iter()
            .find(|t| t.id == tab_id)
    }
}

// ── Tab draft conversion ──
//
// `TabDraft` captures the textual editor state held by the view layer.
// `build_http_tab`/`restore_tab_draft` convert between the persisted
// `HttpTab` row and the editor draft, centralising the auth-text parsing
// and body-mode detection so the view layer doesn't reimplement them.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabDraft {
    pub url: String,
    pub params_text: String,
    pub path_params_text: String,
    pub body_text: String,
    pub headers_text: String,
    pub cookies_text: String,
    pub auth_text: String,
    pub pre_ops_text: String,
    pub post_ops_text: String,
    pub active_request_tab: i64,
}

pub fn build_http_tab(
    tab_id: &str,
    node_id: &str,
    name: &str,
    method: &str,
    draft: &TabDraft,
) -> HttpTab {
    let auth_rows = parse_kv_text(&draft.auth_text);
    HttpTab {
        id: tab_id.to_string(),
        name: name.to_string(),
        method: method.to_string(),
        url: draft.url.clone(),
        request_mode: "rest".into(),
        body_mode: detect_body_mode(&draft.body_text).to_string(),
        auth_type: extract_auth_type(&auth_rows),
        auth_value: extract_auth_value(&auth_rows),
        headers_text: draft.headers_text.clone(),
        cookies_text: draft.cookies_text.clone(),
        body_text: draft.body_text.clone(),
        params_text: draft.params_text.clone(),
        path_params_text: draft.path_params_text.clone(),
        pre_ops_text: draft.pre_ops_text.clone(),
        post_ops_text: draft.post_ops_text.clone(),
        node_id: node_id.to_string(),
        active_request_tab: draft.active_request_tab,
        updated_at: String::new(),
    }
}

pub fn restore_tab_draft(tab: &HttpTab) -> TabDraft {
    TabDraft {
        url: tab.url.clone(),
        params_text: tab.params_text.clone(),
        path_params_text: tab.path_params_text.clone(),
        body_text: tab.body_text.clone(),
        headers_text: tab.headers_text.clone(),
        cookies_text: tab.cookies_text.clone(),
        auth_text: format_auth_for_input(&tab.auth_type, &tab.auth_value),
        pre_ops_text: tab.pre_ops_text.clone(),
        post_ops_text: tab.post_ops_text.clone(),
        active_request_tab: tab.active_request_tab,
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
) -> Result<(ApiResponse, HashMap<String, String>)> {
    let mut draft = script_service::RequestDraft {
        method: request.method.label().to_string(),
        url: request.path.clone(),
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

    let resolved_url = resolve_with_temp(&draft.url, &temporary, &env_vars);
    let resolved_params: Vec<String> = draft
        .params
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| {
            format!(
                "{}={}",
                resolve_with_temp(k, &temporary, &env_vars),
                resolve_with_temp(v, &temporary, &env_vars)
            )
        })
        .collect();

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
        format!("{base_url}?{}", resolved_params.join("&"))
    };

    let body = if matches!(request.method, HttpMethod::Get | HttpMethod::Delete) {
        String::new()
    } else {
        resolve_with_temp(&draft.body, &temporary, &env_vars)
    };

    let curl_preview = build_curl_preview(&url, request.method, &[], &body);
    let request_dump = build_request_dump(&url, request.method, &[], &body);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;

    let method = reqwest::Method::from_bytes(request.method.label().as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let mut req = client.request(method, &url);

    // Add headers from draft
    for (k, v) in &draft.headers {
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&resolve_with_temp(v, &temporary, &env_vars)) {
                req = req.header(header_name, header_value);
            }
        }
    }

    // Add auth headers
    for r in &request.auth {
        if !r.enabled || r.key.trim().is_empty() {
            continue;
        }
        let val = resolve_with_temp(r.value.trim(), &temporary, &env_vars);
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(r.key.trim().as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&val) {
                req = req.header(header_name, header_value);
            }
        }
    }

    // Add cookies
    let cookie_pairs: Vec<String> = request
        .cookies
        .iter()
        .filter(|r| r.enabled && !r.key.trim().is_empty())
        .map(|r| format!("{}={}", r.key.trim(), resolve_with_temp(r.value.trim(), &temporary, &env_vars)))
        .collect();
    if !cookie_pairs.is_empty() {
        if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&cookie_pairs.join("; ")) {
            req = req.header(reqwest::header::COOKIE, header_value);
        }
    }

    // Add body
    if !body.is_empty() && !matches!(request.method, HttpMethod::Get | HttpMethod::Delete) {
        req = req.body(body.clone());
    }

    // Add environment headers
    for r in &environment.headers {
        if !r.enabled || r.key.trim().is_empty() {
            continue;
        }
        let val = resolve_with_temp(&r.value, &temporary, &env_vars);
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(r.key.trim().as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&val) {
                req = req.header(header_name, header_value);
            }
        }
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
    for (key, value) in resp.headers() {
        headers_text.push_str(&format!("{}: {}\n", key, value.to_str().unwrap_or("")));
    }

    let resp_body = resp.text()?;
    let size_bytes = resp_body.len();

    Ok((
        ApiResponse {
            status_line: status_line.clone(),
            status_code,
            duration_ms,
            size_bytes,
            body: prettify_body(&resp_body),
            headers: headers_text,
            request_dump,
            curl: curl_preview,
            logs: vec![
                format!("发送 {} {}", request.method.label(), url),
                format!("响应 {}", status_line),
                format!("耗时 {} ms", duration_ms),
            ],
            assertion_results: Vec::new(),
        },
        temporary,
    ))
}

fn build_final_url(environment: &ApiEnvironment, request: &ApiRequest) -> String {
    let base_url = substitute_vars(environment.base_url.trim(), environment);
    let path = substitute_vars(request.path.trim(), environment);
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

    let extra_path = request
        .path_rows
        .iter()
        .filter(|row| row.enabled && !row.value.trim().is_empty())
        .map(|row| substitute_vars(row.value.trim(), environment))
        .collect::<Vec<_>>();
    let path = if extra_path.is_empty() {
        path
    } else {
        format!("{}/{}", path.trim_end_matches('/'), extra_path.join("/"))
    };

    let query = request
        .params
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| {
            format!(
                "{}={}",
                row.key.trim(),
                substitute_vars(row.value.trim(), environment)
            )
        })
        .collect::<Vec<_>>();
    if query.is_empty() {
        path
    } else {
        format!("{path}?{}", query.join("&"))
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

fn prettify_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::from("{\n  \"message\": \"empty body\"\n}");
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
    let top_folders: Vec<&CollectionNode> = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Folder && n.parent_id.is_none())
        .collect();

    for folder in &top_folders {
        let endpoints = collect_descendant_endpoints(folder.id.as_str(), nodes);
        let requests: Vec<ApiRequest> = endpoints.iter().map(|ep| node_to_request(ep)).collect();
        groups.push(ApiGroup {
            id: Some(folder.id.clone()),
            name: folder.name.clone(),
            requests,
        });
    }

    // Root-level endpoints (not under any folder) go into a default group
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
                requests: root_endpoints
                    .iter()
                    .map(|ep| node_to_request(ep))
                    .collect(),
            },
        );
    }

    groups
}

fn collect_descendant_endpoints<'a>(
    root_id: &str,
    nodes: &'a [CollectionNode],
) -> Vec<&'a CollectionNode> {
    let mut result = Vec::new();
    let mut queue = vec![root_id];
    while let Some(current_id) = queue.pop() {
        for node in nodes {
            if node.parent_id.as_deref() == Some(current_id) {
                match node.kind {
                    NodeKind::Endpoint => result.push(node),
                    NodeKind::Folder => queue.push(node.id.as_str()),
                    NodeKind::Case => {}
                }
            }
        }
    }
    result
}

fn node_to_request(node: &CollectionNode) -> ApiRequest {
    let snapshot = RequestSnapshot::from_json(&node.request_json);
    let method = HttpMethod::from_label(&node.method);
    ApiRequest {
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
        headers: parse_kv_text(&snapshot.headers_text),
        cookies: parse_kv_text(&snapshot.cookies_text),
        auth: parse_kv_text(&format_auth(&snapshot.auth_type, &snapshot.auth_value)),
        pre_ops: snapshot.pre_ops_text,
        post_ops: snapshot.post_ops_text,
        scenarios: Vec::new(),
    }
}

fn request_to_snapshot(method: &str, url: &str, request: &ApiRequest) -> RequestSnapshot {
    RequestSnapshot {
        method: method.to_string(),
        url: url.to_string(),
        params_text: format_kv_rows(&request.params),
        path_params_text: format_kv_rows(&request.path_rows),
        headers_text: format_kv_rows(&request.headers),
        cookies_text: format_kv_rows(&request.cookies),
        body_text: request.body.clone(),
        body_mode: String::new(),
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
            let (key, value) = line
                .split_once('=')
                .or_else(|| line.split_once(':'))
                .map(|(k, v)| (k.trim(), v.trim()))
                .unwrap_or((line, ""));
            KeyValueRow {
                enabled: true,
                key: key.to_string(),
                value: value.to_string(),
                description: String::new(),
            }
        })
        .collect()
}

fn parse_kv_lines(text: &str) -> Vec<(bool, String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (key, value) = line
                .split_once('=')
                .map(|(k, v)| (k.trim(), v.trim()))
                .unwrap_or((line, ""));
            (true, key.to_string(), value.to_string())
        })
        .collect()
}

fn format_kv_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .filter(|r| !r.key.is_empty())
        .map(|r| format!("{}={}", r.key, r.value))
        .collect::<Vec<_>>()
        .join("\n")
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
            _ => Self::Get,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RequestSnapshot;
    use qingqi_plugin::{database::DatabaseService, log_error, storage::AppPaths};
    use std::fs;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

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
    fn build_groups_nested_folders_collect_descendants() {
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
        assert_eq!(groups[0].requests.len(), 1);
        assert_eq!(groups[0].requests[0].title, "/user/list");
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
        assert_eq!(snapshot.pre_ops_text, "set x=1");
        assert_eq!(snapshot.post_ops_text, "extract id=$.id");
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

        let restored = RequestSnapshot::from_json(&node.request_json);
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
    fn tab_state_persisted() {
        let store = temp_store();
        let tab = crate::model::HttpTab {
            id: "tab-persist".into(),
            name: "Test".into(),
            method: "POST".into(),
            url: "/api/data".into(),
            request_mode: "rest".into(),
            body_mode: "json".into(),
            auth_type: "bearer".into(),
            auth_value: "tok".into(),
            headers_text: "Content-Type=application/json".into(),
            cookies_text: String::new(),
            body_text: r#"{"key":"val"}"#.into(),
            params_text: String::new(),
            path_params_text: String::new(),
            pre_ops_text: "set x=1".into(),
            post_ops_text: "status == 200".into(),
            node_id: "node-1".into(),
            active_request_tab: 2,
            updated_at: String::new(),
        };
        store.save_tab(&tab).unwrap();

        let tabs = store.list_tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].name, "Test");
        assert_eq!(tabs[0].method, "POST");
        assert_eq!(tabs[0].pre_ops_text, "set x=1");
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
                description: String::new(),
            },
            KeyValueRow {
                enabled: true,
                key: String::new(),
                value: "no-key".into(),
                description: String::new(),
            },
            KeyValueRow {
                enabled: true,
                key: "also".into(),
                value: "ok".into(),
                description: String::new(),
            },
        ];
        let formatted = format_kv_rows(&rows);
        assert!(formatted.contains("valid=yes"));
        assert!(formatted.contains("also=ok"));
        assert!(!formatted.contains("no-key"));
    }

    #[test]
    fn service_create_environment() {
        let store = temp_store();
        let service = ApiService {
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
        };
        let env = service
            .create_environment("测试环境", "http://test.api.com")
            .unwrap();
        assert_eq!(env.name, "测试环境");
        assert_eq!(env.base_url, "http://test.api.com");
        assert_eq!(env.badge, "测");
        assert!(service.revision() > 0);
    }

    #[test]
    fn service_duplicate_environment() {
        let store = temp_store();
        let service = ApiService {
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
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
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
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
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
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
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
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
    fn service_tab_persistence_lifecycle() {
        let store = temp_store();
        let service = ApiService {
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
        };
        let tab = HttpTab {
            id: "tab-uuid-1".into(),
            name: "用户接口".into(),
            method: "GET".into(),
            url: "/api/user".into(),
            request_mode: "rest".into(),
            body_mode: "none".into(),
            auth_type: String::new(),
            auth_value: String::new(),
            headers_text: String::new(),
            cookies_text: String::new(),
            body_text: String::new(),
            params_text: String::new(),
            path_params_text: String::new(),
            pre_ops_text: String::new(),
            post_ops_text: String::new(),
            node_id: "node-1".into(),
            active_request_tab: 0,
            updated_at: String::new(),
        };
        service.save_tab_state(&tab).unwrap();
        let loaded = service.load_persisted_tabs();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "用户接口");
        assert!(service.delete_persisted_tab("tab-uuid-1").unwrap());
        assert!(service.load_persisted_tabs().is_empty());
    }

    #[test]
    fn parse_kv_lines_parses_and_filters() {
        let result = parse_kv_lines("KEY1=val1\n\nKEY2=val2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, "KEY1");
        assert_eq!(result[0].2, "val1");
        assert!(result[0].0); // enabled
    }

    #[test]
    fn load_persisted_tab_by_id_finds_correct_tab() {
        let store = temp_store();
        let service = ApiService {
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
        };

        let tab_a = HttpTab {
            id: "tab-a".into(),
            name: "Tab A".into(),
            method: "GET".into(),
            url: "/a".into(),
            ..empty_tab_fields()
        };
        let tab_b = HttpTab {
            id: "tab-b".into(),
            name: "Tab B".into(),
            method: "POST".into(),
            url: "/b".into(),
            ..empty_tab_fields()
        };
        service.save_tab_state(&tab_a).unwrap();
        service.save_tab_state(&tab_b).unwrap();

        let found = service.load_persisted_tab_by_id("tab-b").unwrap();
        assert_eq!(found.name, "Tab B");
        assert_eq!(found.method, "POST");

        assert!(service.load_persisted_tab_by_id("tab-missing").is_none());
    }

    #[test]
    fn tab_persists_auth_fields_roundtrip() {
        let store = temp_store();
        let tab = HttpTab {
            id: "tab-auth".into(),
            name: "Auth Test".into(),
            method: "GET".into(),
            url: "/api/protected".into(),
            request_mode: "rest".into(),
            body_mode: "json".into(),
            auth_type: "bearer".into(),
            auth_value: "my-secret-token".into(),
            headers_text: "Accept=application/json".into(),
            cookies_text: String::new(),
            body_text: String::new(),
            params_text: String::new(),
            path_params_text: String::new(),
            pre_ops_text: String::new(),
            post_ops_text: String::new(),
            node_id: "node-1".into(),
            active_request_tab: 5,
            updated_at: String::new(),
        };
        store.save_tab(&tab).unwrap();

        let tabs = store.list_tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].auth_type, "bearer");
        assert_eq!(tabs[0].auth_value, "my-secret-token");
        assert_eq!(tabs[0].body_mode, "json");
        assert_eq!(tabs[0].active_request_tab, 5);
        assert_eq!(tabs[0].headers_text, "Accept=application/json");
    }

    #[test]
    fn tab_persists_all_draft_fields_roundtrip() {
        let store = temp_store();
        let tab = HttpTab {
            id: "tab-full".into(),
            name: "Full Draft".into(),
            method: "POST".into(),
            url: "/api/v2/data".into(),
            request_mode: "rest".into(),
            body_mode: "json".into(),
            auth_type: "bearer".into(),
            auth_value: "tok123".into(),
            headers_text: "Content-Type=application/json\nX-Custom=hello".into(),
            cookies_text: "session=abc".into(),
            body_text: r#"{"name":"test"}"#.into(),
            params_text: "page=1&limit=10".into(),
            path_params_text: "id=42".into(),
            pre_ops_text: "set token=abc".into(),
            post_ops_text: "status == 200".into(),
            node_id: "node-ep-5".into(),
            active_request_tab: 2,
            updated_at: String::new(),
        };
        store.save_tab(&tab).unwrap();

        let loaded = store.list_tabs().unwrap();
        assert_eq!(loaded.len(), 1);
        let t = &loaded[0];
        assert_eq!(t.name, "Full Draft");
        assert_eq!(t.method, "POST");
        assert_eq!(t.url, "/api/v2/data");
        assert_eq!(t.auth_type, "bearer");
        assert_eq!(t.auth_value, "tok123");
        assert_eq!(
            t.headers_text,
            "Content-Type=application/json\nX-Custom=hello"
        );
        assert_eq!(t.cookies_text, "session=abc");
        assert_eq!(t.body_text, r#"{"name":"test"}"#);
        assert_eq!(t.params_text, "page=1&limit=10");
        assert_eq!(t.path_params_text, "id=42");
        assert_eq!(t.pre_ops_text, "set token=abc");
        assert_eq!(t.post_ops_text, "status == 200");
        assert_eq!(t.node_id, "node-ep-5");
        assert_eq!(t.active_request_tab, 2);
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
    fn send_tab_preserves_existing_node_id() {
        let store = temp_store();
        // Pre-save a tab with a known node_id
        let existing = HttpTab {
            id: "tab-send-node".into(),
            name: "Before".into(),
            method: "GET".into(),
            url: "/api/test".into(),
            node_id: "node-collection-42".into(),
            body_text: "{}".into(),
            ..empty_tab_fields()
        };
        store.save_tab(&existing).unwrap();

        // Simulate what the service thread does: look up existing node_id
        let found_node_id = store
            .list_tabs()
            .unwrap()
            .into_iter()
            .find(|t| t.id == "tab-send-node")
            .and_then(|t| {
                if t.node_id.is_empty() {
                    None
                } else {
                    Some(t.node_id)
                }
            })
            .unwrap_or_default();
        assert_eq!(found_node_id, "node-collection-42");

        // Now save a new version — should preserve node_id
        let updated = HttpTab {
            id: "tab-send-node".into(),
            name: "After".into(),
            method: "POST".into(),
            url: "/api/test/v2".into(),
            node_id: found_node_id,
            body_mode: "json".into(),
            body_text: r#"{"new":true}"#.into(),
            ..empty_tab_fields()
        };
        store.save_tab(&updated).unwrap();

        let tabs = store.list_tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].node_id, "node-collection-42");
        assert_eq!(tabs[0].method, "POST");
        assert_eq!(tabs[0].body_mode, "json");
    }

    // Helper for tests: provides default empty fields for HttpTab
    fn empty_tab_fields() -> HttpTab {
        HttpTab {
            id: String::new(),
            name: String::new(),
            method: String::new(),
            url: String::new(),
            request_mode: String::new(),
            body_mode: String::new(),
            auth_type: String::new(),
            auth_value: String::new(),
            headers_text: String::new(),
            cookies_text: String::new(),
            body_text: String::new(),
            params_text: String::new(),
            path_params_text: String::new(),
            pre_ops_text: String::new(),
            post_ops_text: String::new(),
            node_id: String::new(),
            active_request_tab: 0,
            updated_at: String::new(),
        }
    }

    fn sample_draft() -> TabDraft {
        TabDraft {
            url: "/api/v1/data".into(),
            params_text: "page=1\nlimit=10".into(),
            path_params_text: "id=42".into(),
            body_text: r#"{"name":"qingqi"}"#.into(),
            headers_text: "Content-Type=application/json\nX-Custom=hello".into(),
            cookies_text: "session=abc".into(),
            auth_text: "Authorization=Bearer tok123".into(),
            pre_ops_text: "set token=abc".into(),
            post_ops_text: "status == 200".into(),
            active_request_tab: editor_tab_index(EditorTab::Body),
        }
    }

    #[test]
    fn editor_tab_index_roundtrip_all_variants() {
        for tab in [
            EditorTab::Params,
            EditorTab::Path,
            EditorTab::Body,
            EditorTab::Headers,
            EditorTab::Cookies,
            EditorTab::Auth,
            EditorTab::PreOps,
            EditorTab::PostOps,
        ] {
            let idx = editor_tab_index(tab);
            assert_eq!(index_to_editor_tab(idx), Some(tab));
        }
    }

    #[test]
    fn index_to_editor_tab_rejects_out_of_range() {
        assert_eq!(index_to_editor_tab(-1), None);
        assert_eq!(index_to_editor_tab(8), None);
        assert_eq!(index_to_editor_tab(999), None);
    }

    #[test]
    fn build_http_tab_copies_textual_fields() {
        let draft = sample_draft();
        let tab = build_http_tab("tab-1", "node-x", "用户接口", "POST", &draft);

        assert_eq!(tab.id, "tab-1");
        assert_eq!(tab.node_id, "node-x");
        assert_eq!(tab.name, "用户接口");
        assert_eq!(tab.method, "POST");
        assert_eq!(tab.url, "/api/v1/data");
        assert_eq!(tab.params_text, "page=1\nlimit=10");
        assert_eq!(tab.path_params_text, "id=42");
        assert_eq!(tab.body_text, r#"{"name":"qingqi"}"#);
        assert_eq!(
            tab.headers_text,
            "Content-Type=application/json\nX-Custom=hello"
        );
        assert_eq!(tab.cookies_text, "session=abc");
        assert_eq!(tab.pre_ops_text, "set token=abc");
        assert_eq!(tab.post_ops_text, "status == 200");
        assert_eq!(tab.request_mode, "rest");
        assert_eq!(tab.active_request_tab, editor_tab_index(EditorTab::Body));
    }

    #[test]
    fn build_http_tab_extracts_bearer_auth() {
        let mut draft = sample_draft();
        draft.auth_text = "Authorization=Bearer my-token".into();
        let tab = build_http_tab("tab-a", "", "n", "GET", &draft);
        assert_eq!(tab.auth_type, "bearer");
        assert_eq!(tab.auth_value, "my-token");
    }

    #[test]
    fn build_http_tab_extracts_basic_auth() {
        let mut draft = sample_draft();
        draft.auth_text = "Authorization=Basic dXNlcjpwYXNz".into();
        let tab = build_http_tab("tab-a", "", "n", "GET", &draft);
        assert_eq!(tab.auth_type, "basic");
        assert_eq!(tab.auth_value, "dXNlcjpwYXNz");
    }

    #[test]
    fn build_http_tab_extracts_apikey_auth() {
        let mut draft = sample_draft();
        draft.auth_text = "X-API-Key=secret".into();
        let tab = build_http_tab("tab-a", "", "n", "GET", &draft);
        assert_eq!(tab.auth_type, "apikey");
        assert_eq!(tab.auth_value, "secret");
    }

    #[test]
    fn build_http_tab_blank_auth_yields_empty_fields() {
        let mut draft = sample_draft();
        draft.auth_text = String::new();
        let tab = build_http_tab("tab-a", "", "n", "GET", &draft);
        assert!(tab.auth_type.is_empty());
        assert!(tab.auth_value.is_empty());
    }

    #[test]
    fn build_http_tab_detects_body_modes() {
        let mut draft = sample_draft();
        draft.body_text = r#"{"key":"val"}"#.into();
        assert_eq!(
            build_http_tab("a", "", "n", "POST", &draft).body_mode,
            "json"
        );

        draft.body_text = "[1, 2, 3]".into();
        assert_eq!(
            build_http_tab("a", "", "n", "POST", &draft).body_mode,
            "json"
        );

        draft.body_text = "plain body".into();
        assert_eq!(
            build_http_tab("a", "", "n", "POST", &draft).body_mode,
            "text"
        );

        draft.body_text = String::new();
        assert_eq!(
            build_http_tab("a", "", "n", "POST", &draft).body_mode,
            "none"
        );
    }

    #[test]
    fn restore_tab_draft_reverses_build_http_tab() {
        let original = sample_draft();
        let tab = build_http_tab("tab-r", "node-r", "n", "POST", &original);
        let restored = restore_tab_draft(&tab);

        assert_eq!(restored.url, original.url);
        assert_eq!(restored.params_text, original.params_text);
        assert_eq!(restored.path_params_text, original.path_params_text);
        assert_eq!(restored.body_text, original.body_text);
        assert_eq!(restored.headers_text, original.headers_text);
        assert_eq!(restored.cookies_text, original.cookies_text);
        assert_eq!(restored.pre_ops_text, original.pre_ops_text);
        assert_eq!(restored.post_ops_text, original.post_ops_text);
        assert_eq!(restored.active_request_tab, original.active_request_tab);
        // auth_text round-trips back to the canonical "Authorization=Bearer X" form
        assert_eq!(restored.auth_text, "Authorization=Bearer tok123");
    }

    #[test]
    fn restore_tab_draft_format_apikey() {
        let mut tab = empty_tab_fields();
        tab.auth_type = "apikey".into();
        tab.auth_value = "secret".into();
        let draft = restore_tab_draft(&tab);
        assert_eq!(draft.auth_text, "X-API-Key=secret");
    }

    #[test]
    fn restore_tab_draft_format_basic() {
        let mut tab = empty_tab_fields();
        tab.auth_type = "basic".into();
        tab.auth_value = "abc".into();
        let draft = restore_tab_draft(&tab);
        assert_eq!(draft.auth_text, "Authorization=Basic abc");
    }

    #[test]
    fn restore_tab_draft_empty_auth_yields_empty_text() {
        let mut tab = empty_tab_fields();
        tab.auth_type = "bearer".into();
        tab.auth_value = String::new();
        assert!(restore_tab_draft(&tab).auth_text.is_empty());

        tab.auth_type = String::new();
        tab.auth_value = "stray-value".into();
        assert!(restore_tab_draft(&tab).auth_text.is_empty());
    }

    #[test]
    fn format_auth_for_input_unknown_type_uppercases_key() {
        let text = format_auth_for_input("oauth2", "tok");
        assert_eq!(text, "OAUTH2=tok");
    }

    #[test]
    fn build_then_persist_then_restore_via_service_roundtrip() {
        let store = temp_store();
        let service = ApiService {
            revision: AtomicU64::new(0),
            state: Mutex::new(ApiServiceState {
                in_flight: false,
                pending_response: None,
                pending_error: None,
                pending_notice: None,
                last_tab_id: String::new(),
            }),
            data_source: store,
            cancel_flag: AtomicBool::new(false),
        };
        let draft = sample_draft();
        let tab = build_http_tab("tab-uuid-1", "node-1", "Sample", "POST", &draft);
        service.save_tab_state(&tab).unwrap();

        let loaded = service
            .load_persisted_tab_by_id("tab-uuid-1")
            .expect("tab should be persisted");
        let restored = restore_tab_draft(&loaded);
        assert_eq!(restored, draft);
        assert_eq!(loaded.method, "POST");
        assert_eq!(loaded.name, "Sample");
        assert_eq!(loaded.node_id, "node-1");
        assert_eq!(loaded.auth_type, "bearer");
        assert_eq!(loaded.auth_value, "tok123");
    }
}
