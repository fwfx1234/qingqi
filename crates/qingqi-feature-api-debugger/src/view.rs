use std::sync::Arc;

use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, AnyElement, div, hsla,
    prelude::FluentBuilder, px, rgb,
};
use gpui_component::{
    IndexPath, Sizable, Size,
    button::{Button, ButtonVariants},
    IconName,
    select::{Select, SelectEvent, SelectState},
};
use uuid::Uuid;

use crate::code_gen::CodeLanguage;
use crate::service::{
    self, ApiEnvironment, ApiGroup, ApiRequest, ApiResponse, ApiScenario, ApiService, AuthType,
    BodyMode, EditorTab, EnvDetailTab, HttpHistory, HttpMethod, KeyValueRow, ResponseTab,
    ScenarioStatus, TabDraft,
};
use qingqi_ui::{
    text_input::{TextInput, TextInputStyle},
    theme, ui,
    ui::glass,
};

const STACK_BREAKPOINT_PX: f32 = 980.0;

#[derive(Clone, Debug)]
enum OpenTab {
    Request {
        index: usize,
        tab_id: String,
        /// Collection node id this tab is associated with (may be empty if
        /// the tab was created without a known node).
        node_id: String,
    },
    Scenario {
        request_index: usize,
        scenario_index: usize,
        tab_id: String,
    },
}

impl PartialEq for OpenTab {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Request { index: a, .. }, Self::Request { index: b, .. }) => a == b,
            (
                Self::Scenario {
                    request_index: a1,
                    scenario_index: a2,
                    ..
                },
                Self::Scenario {
                    request_index: b1,
                    scenario_index: b2,
                    ..
                },
            ) => a1 == b1 && a2 == b2,
            _ => false,
        }
    }
}

impl Eq for OpenTab {}

impl OpenTab {
    fn tab_id(&self) -> &str {
        match self {
            Self::Request { tab_id, .. } | Self::Scenario { tab_id, .. } => tab_id,
        }
    }

    fn node_id(&self) -> &str {
        match self {
            Self::Request { node_id, .. } => node_id,
            Self::Scenario { .. } => "",
        }
    }

    fn new_request(index: usize) -> Self {
        Self::Request {
            index,
            tab_id: format!("tab-{}", Uuid::new_v4().simple()),
            node_id: String::new(),
        }
    }

    fn new_scenario(request_index: usize, scenario_index: usize) -> Self {
        Self::Scenario {
            request_index,
            scenario_index,
            tab_id: format!("tab-{}", Uuid::new_v4().simple()),
        }
    }

    fn with_id(index: usize, tab_id: String, node_id: String) -> Self {
        Self::Request {
            index,
            tab_id,
            node_id,
        }
    }
}

/// One editable key/value row, backed by two live text inputs plus an
/// enabled flag. Cloning a row clones the entity handles (cheap) so the same
/// underlying inputs can be rendered from a snapshot passed into `editor_panel`.
#[derive(Clone)]
struct KvRow {
    enabled: bool,
    key: Entity<TextInput>,
    value: Entity<TextInput>,
}

/// Reusable editable key/value table model — the source of truth for the
/// Params / Path / Headers / Cookies tabs. It serializes to the legacy
/// `KEY=VALUE` line format (with a leading `# ` marking disabled rows) via
/// `parse_rows`/`format_rows`, so the existing text-based persistence
/// (TabDraft / HttpTab / RequestSnapshot) keeps working unchanged.
struct KvEditor {
    rows: Vec<KvRow>,
}

impl KvEditor {
    fn new(cx: &mut App, rows: &[KeyValueRow]) -> Self {
        let mut editor = Self { rows: Vec::new() };
        editor.set_rows(cx, rows);
        editor
    }

    fn from_text(cx: &mut App, text: &str) -> Self {
        Self::new(cx, &parse_rows(text))
    }

    /// Rebuild the live inputs from a row model (used when switching
    /// request/tab/scenario).
    fn set_rows(&mut self, cx: &mut App, rows: &[KeyValueRow]) {
        self.rows = rows
            .iter()
            .map(|row| KvRow {
                enabled: row.enabled,
                key: kv_input(cx, &row.key, "键"),
                value: kv_input(cx, &row.value, "值"),
            })
            .collect();
    }

    fn set_from_text(&mut self, cx: &mut App, text: &str) {
        self.set_rows(cx, &parse_rows(text));
    }

    /// Read the current inputs back into a row model.
    fn to_rows(&self, cx: &App) -> Vec<KeyValueRow> {
        self.rows
            .iter()
            .map(|row| KeyValueRow {
                enabled: row.enabled,
                key: row.key.read(cx).text().trim().to_string(),
                value: row.value.read(cx).text().trim().to_string(),
                description: String::new(),
            })
            .collect()
    }

    fn to_text(&self, cx: &App) -> String {
        format_rows(&self.to_rows(cx))
    }

    fn add_row(&mut self, cx: &mut App) {
        self.rows.push(KvRow {
            enabled: true,
            key: kv_input(cx, "", "键"),
            value: kv_input(cx, "", "值"),
        });
    }

    fn remove_row(&mut self, index: usize) {
        if index < self.rows.len() {
            self.rows.remove(index);
        }
    }

    fn toggle(&mut self, index: usize) {
        if let Some(row) = self.rows.get_mut(index) {
            row.enabled = !row.enabled;
        }
    }
}

/// Live input entities for the Auth tab form, snapshotted into `editor_panel`.
#[derive(Clone)]
struct AuthFormInputs {
    bearer: Entity<TextInput>,
    basic_user: Entity<TextInput>,
    basic_pass: Entity<TextInput>,
    apikey_name: Entity<TextInput>,
    apikey_value: Entity<TextInput>,
    in_query: bool,
}

/// Plain-string values derived from canonical auth rows, used to seed the
/// Auth form inputs. The reverse of `ApiDebuggerView::auth_rows`.
#[derive(Default)]
struct AuthFormValues {
    auth_type: Option<AuthType>,
    bearer: String,
    basic_user: String,
    basic_pass: String,
    apikey_name: String,
    apikey_value: String,
    in_query: bool,
}

/// Decode canonical auth rows (the header/query pairs produced by
/// `auth_rows`) back into editable form values.
fn derive_auth_form(rows: &[KeyValueRow]) -> AuthFormValues {
    let Some(row) = rows.iter().find(|r| !r.key.trim().is_empty()) else {
        return AuthFormValues {
            auth_type: Some(AuthType::None),
            ..Default::default()
        };
    };
    let key = row.key.trim();
    let value = row.value.trim();
    if key.eq_ignore_ascii_case("authorization") {
        if let Some(token) = value.strip_prefix("Bearer ").or_else(|| value.strip_prefix("bearer "))
        {
            return AuthFormValues {
                auth_type: Some(AuthType::BearerToken),
                bearer: token.trim().to_string(),
                ..Default::default()
            };
        }
        if let Some(encoded) = value.strip_prefix("Basic ").or_else(|| value.strip_prefix("basic "))
        {
            let decoded = service::base64_decode(encoded.trim())
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default();
            let (user, pass) = decoded.split_once(':').unwrap_or((decoded.as_str(), ""));
            return AuthFormValues {
                auth_type: Some(AuthType::BasicAuth),
                basic_user: user.to_string(),
                basic_pass: pass.to_string(),
                ..Default::default()
            };
        }
    }
    // Anything else is treated as an API key (header or query).
    AuthFormValues {
        auth_type: Some(AuthType::ApiKey),
        apikey_name: key.to_string(),
        apikey_value: value.to_string(),
        in_query: row.description.trim().eq_ignore_ascii_case("query"),
        ..Default::default()
    }
}

pub struct ApiDebuggerView {
    service: Arc<ApiService>,
    groups: Vec<ApiGroup>,
    environments: Vec<ApiEnvironment>,
    open_tabs: Vec<OpenTab>,
    active_tab: OpenTab,
    selected_request: usize,
    selected_scenario: Option<usize>,
    selected_environment: usize,
    editor_tab: EditorTab,
    response_tab: ResponseTab,
    /// Selected language for the response "代码" tab.
    response_code_lang: CodeLanguage,
    /// History rows for the active tab (loaded lazily when the 历史 tab opens).
    history_entries: Vec<HttpHistory>,
    env_detail_tab: EnvDetailTab,
    body_mode: BodyMode,
    auth_type: AuthType,
    show_env_popup: bool,
    show_env_manager: bool,
    show_collection_menu: bool,
    collection_menu_title: String,
    collection_menu_position: Option<(f32, f32)>,
    collection_menu_node_id: String,
    method_select_state: Option<Entity<SelectState<Vec<HttpMethod>>>>,
    show_curl_import: bool,
    curl_import_input: Entity<TextInput>,
    show_rename: bool,
    rename_input: Entity<TextInput>,
    rename_node_id: String,
    path_input: Entity<TextInput>,
    params_kv: KvEditor,
    path_kv: KvEditor,
    body_input: Entity<TextInput>,
    headers_kv: KvEditor,
    cookies_kv: KvEditor,
    auth_bearer_input: Entity<TextInput>,
    auth_basic_user_input: Entity<TextInput>,
    auth_basic_pass_input: Entity<TextInput>,
    auth_apikey_name_input: Entity<TextInput>,
    auth_apikey_value_input: Entity<TextInput>,
    auth_apikey_in_query: bool,
    pre_ops_input: Entity<TextInput>,
    post_ops_input: Entity<TextInput>,
    env_name_input: Entity<TextInput>,
    env_base_url_input: Entity<TextInput>,
    env_variables_input: Entity<TextInput>,
    env_headers_input: Entity<TextInput>,
    response: ApiResponse,
    notice: String,
    last_revision: u64,
}

impl ApiDebuggerView {
    pub fn new(service: Arc<ApiService>, cx: &mut App) -> Self {
        let workspace_result = service.load_workspace();
        let (groups, environments, notice) = match workspace_result {
            Ok(workspace) => {
                if workspace.groups.is_empty()
                    || workspace.groups.iter().all(|g| g.requests.is_empty())
                {
                    // Honest empty state: one placeholder group so the editor can render
                    let empty_request = ApiRequest {
                        node_id: String::new(),
                        title: String::from("新请求"),
                        method: HttpMethod::Get,
                        path: String::from("/"),
                        params: Vec::new(),
                        path_rows: Vec::new(),
                        body: String::new(),
                        body_mode: BodyMode::None,
                        headers: Vec::new(),
                        cookies: Vec::new(),
                        auth: Vec::new(),
                        pre_ops: String::new(),
                        post_ops: String::new(),
                        scenarios: Vec::new(),
                    };
                    (
                        vec![ApiGroup {
                            id: None,
                            name: String::from("集合"),
                            requests: vec![empty_request],
                        }],
                        workspace.environments,
                        String::from("集合为空，点击 + 创建第一个请求"),
                    )
                } else {
                    (
                        workspace.groups,
                        workspace.environments,
                        String::from("已加载 API 调试器"),
                    )
                }
            }
            Err(error) => {
                let empty_request = ApiRequest {
                    node_id: String::new(),
                    title: String::from("新请求"),
                    method: HttpMethod::Get,
                    path: String::from("/"),
                    params: Vec::new(),
                    path_rows: Vec::new(),
                    body: String::new(),
                    body_mode: BodyMode::None,
                    headers: Vec::new(),
                    cookies: Vec::new(),
                    auth: Vec::new(),
                    pre_ops: String::new(),
                    post_ops: String::new(),
                    scenarios: Vec::new(),
                };
                (
                    vec![ApiGroup {
                        id: None,
                        name: String::from("集合"),
                        requests: vec![empty_request],
                    }],
                    service.list_environments_ui(),
                    format!("工作区加载失败: {error}"),
                )
            }
        };
        let selected_request = 0usize;
        let selected_scenario = request_at(&groups, selected_request)
            .and_then(|request| (!request.scenarios.is_empty()).then_some(0));

        // Try to restore persisted tabs
        let persisted_tabs = service.load_persisted_tabs();
        let first_persisted_tab = persisted_tabs.first().cloned();
        let (open_tabs, active_tab, restored_request) = if !persisted_tabs.is_empty() {
            let mut tabs = Vec::new();
            let mut first_request_index = 0usize;
            for ptab in &persisted_tabs {
                // Match persisted tab to current group/request by method+url
                let matched_index =
                    find_request_index_by_method_url(&groups, &ptab.method, &ptab.url);
                let req_index = matched_index.unwrap_or(0);
                if tabs.is_empty() {
                    first_request_index = req_index;
                }
                tabs.push(OpenTab::with_id(
                    req_index,
                    ptab.id.clone(),
                    ptab.node_id.clone(),
                ));
            }
            let active = tabs
                .first()
                .cloned()
                .unwrap_or_else(|| OpenTab::new_request(selected_request));
            (tabs, active, first_request_index)
        } else {
            let active_tab = selected_scenario
                .map(|index| OpenTab::new_scenario(selected_request, index))
                .unwrap_or_else(|| OpenTab::new_request(selected_request));
            let mut tabs = vec![active_tab.clone()];
            if request_at(&groups, 1).is_some() {
                tabs.push(OpenTab::new_request(1));
            }
            (tabs, active_tab, selected_request)
        };
        let request = request_at(&groups, restored_request)
            .expect("api request should exist")
            .clone();
        let environment = environments
            .first()
            .cloned()
            .unwrap_or_else(|| ApiEnvironment {
                name: String::from("默认环境"),
                badge: String::from("默"),
                color: 0x338855,
                base_url: String::from("http://127.0.0.1:8000"),
                variables: Vec::new(),
                headers: Vec::new(),
            });

        // Determine initial input values: prefer persisted tab draft over collection data
        let (
            init_path,
            init_params,
            init_path_rows,
            init_body,
            init_headers,
            init_cookies,
            init_auth,
            init_pre_ops,
            init_post_ops,
            init_editor_tab,
        ) = if let Some(ref tab) = first_persisted_tab {
            let draft = service::restore_tab_draft(tab);
            let et =
                service::index_to_editor_tab(draft.active_request_tab).unwrap_or(EditorTab::Params);
            (
                draft.url,
                draft.params_text,
                draft.path_params_text,
                draft.body_text,
                draft.headers_text,
                draft.cookies_text,
                draft.auth_text,
                draft.pre_ops_text,
                draft.post_ops_text,
                et,
            )
        } else {
            (
                request.path.clone(),
                format_rows(&request.params),
                format_rows(&request.path_rows),
                request.body.clone(),
                format_rows(&request.headers),
                format_rows(&request.cookies),
                format_rows(&request.auth),
                request.pre_ops.clone(),
                request.post_ops.clone(),
                EditorTab::Params,
            )
        };

        let rev = service.revision();
        let init_auth_form = derive_auth_form(&parse_rows(&init_auth));

        Self {
            service,
            groups,
            environments,
            open_tabs,
            active_tab,
            selected_request: restored_request,
            selected_scenario,
            selected_environment: 0,
            editor_tab: init_editor_tab,
            response_tab: ResponseTab::Body,
            response_code_lang: CodeLanguage::Curl,
            history_entries: Vec::new(),
            env_detail_tab: EnvDetailTab::Variables,
            body_mode: BodyMode::from_db(&detect_body_mode(&init_body)),
            auth_type: init_auth_form.auth_type.unwrap_or(AuthType::None),
            show_env_popup: false,
            show_env_manager: false,
            show_collection_menu: false,
            collection_menu_title: String::from("集合"),
            collection_menu_position: None,
            collection_menu_node_id: String::new(),
            method_select_state: None,
            show_curl_import: false,
            curl_import_input: multiline_input(cx, "", "粘贴 cURL 命令..."),
            show_rename: false,
            rename_input: single_input(cx, "", "输入新名称..."),
            rename_node_id: String::new(),
            path_input: single_input(cx, &init_path, "/api/v1/user/info"),
            params_kv: KvEditor::from_text(cx, &init_params),
            path_kv: KvEditor::from_text(cx, &init_path_rows),
            body_input: multiline_input(cx, &init_body, "{ }"),
            headers_kv: KvEditor::from_text(cx, &init_headers),
            cookies_kv: KvEditor::from_text(cx, &init_cookies),
            auth_bearer_input: single_input(cx, &init_auth_form.bearer, "Token"),
            auth_basic_user_input: single_input(cx, &init_auth_form.basic_user, "用户名"),
            auth_basic_pass_input: single_input(cx, &init_auth_form.basic_pass, "密码"),
            auth_apikey_name_input: single_input(cx, &init_auth_form.apikey_name, "Key（如 X-API-Key）"),
            auth_apikey_value_input: single_input(cx, &init_auth_form.apikey_value, "Value"),
            auth_apikey_in_query: init_auth_form.in_query,
            pre_ops_input: multiline_input(cx, &init_pre_ops, "Pre-ops"),
            post_ops_input: multiline_input(cx, &init_post_ops, "Post-ops"),
            env_name_input: single_input(cx, &environment.name, "环境名称"),
            env_base_url_input: single_input(cx, &environment.base_url, "http://localhost:8080"),
            env_variables_input: multiline_input(
                cx,
                &format_rows(&environment.variables),
                "KEY=VALUE",
            ),
            env_headers_input: multiline_input(cx, &format_rows(&environment.headers), "KEY=VALUE"),
            response: sample_response(),
            notice,
            last_revision: rev,
        }
    }

    fn sync_service_updates(&mut self) {
        let current_revision = self.service.revision();
        if current_revision != self.last_revision {
            if let Ok(workspace) = self.service.load_workspace() {
                self.groups = workspace.groups;
                self.environments = workspace.environments;
                self.selected_environment = self
                    .selected_environment
                    .min(self.environments.len().saturating_sub(1));
            }
            self.last_revision = current_revision;
        }

        if let Some(response) = self.service.take_pending_response() {
            let assertion_summary = if !response.assertion_results.is_empty() {
                let passed = response
                    .assertion_results
                    .iter()
                    .filter(|(_, p)| *p)
                    .count();
                let total = response.assertion_results.len();
                format!(" · 断言 {passed}/{total} 通过")
            } else {
                String::new()
            };
            self.notice = format!("响应已更新 · {}{assertion_summary}", response.status_line);
            self.response = response;
            // Keep the history list live if the user is watching that tab.
            if self.response_tab == ResponseTab::History {
                self.refresh_history();
            }
        }
        if let Some(error) = self.service.take_pending_error() {
            self.notice = format!("请求失败: {error}");
        }
        if let Some(notice) = self.service.take_pending_notice() {
            self.environments = self.service.list_environments_ui();
            self.selected_environment = self
                .selected_environment
                .min(self.environments.len().saturating_sub(1));
            self.notice = notice;
        }
    }

    fn selected_request(&self) -> &ApiRequest {
        request_at(&self.groups, self.selected_request).expect("request should exist")
    }

    fn selected_request_mut(&mut self) -> &mut ApiRequest {
        request_at_mut(&mut self.groups, self.selected_request).expect("request should exist")
    }

    fn selected_environment(&self) -> &ApiEnvironment {
        self.environments
            .get(self.selected_environment)
            .expect("environment should exist")
    }

    fn selected_environment_mut(&mut self) -> &mut ApiEnvironment {
        self.environments
            .get_mut(self.selected_environment)
            .expect("environment should exist")
    }

    fn sync_models(&mut self, cx: &App) {
        let path = self.path_input.read(cx).text();
        let params = self.params_kv.to_rows(cx);
        let path_rows = self.path_kv.to_rows(cx);
        let body = self.body_input.read(cx).text();
        let headers = self.headers_kv.to_rows(cx);
        let cookies = self.cookies_kv.to_rows(cx);
        let auth = self.auth_rows(cx);
        let pre_ops = self.pre_ops_input.read(cx).text();
        let post_ops = self.post_ops_input.read(cx).text();
        let body_mode = self.body_mode;

        {
            let request = self.selected_request_mut();
            request.path = path;
            request.params = params;
            request.path_rows = path_rows;
            request.body = body;
            request.body_mode = body_mode;
            request.headers = headers;
            request.cookies = cookies;
            request.auth = auth;
            request.pre_ops = pre_ops;
            request.post_ops = post_ops;
        }

        let env_name = self.env_name_input.read(cx).text();
        let env_base_url = self.env_base_url_input.read(cx).text();
        let env_variables = parse_rows(&self.env_variables_input.read(cx).text());
        let env_headers = parse_rows(&self.env_headers_input.read(cx).text());

        {
            let environment = self.selected_environment_mut();
            environment.name = env_name;
            environment.base_url = env_base_url;
            environment.variables = env_variables;
            environment.headers = env_headers;
        }
    }

    fn persist_endpoint_if_needed(&self) {
        if self.groups.is_empty() {
            return;
        }
        let request = self.selected_request();
        let method_label = request.method.label().to_string();
        if let Err(error) = self.service.persist_endpoint_snapshot(
            &request.title,
            &method_label,
            &request.path,
            request,
        ) {
            tracing::warn!("持久化端点失败: {error}");
        }
    }

    fn persist_workspace(&mut self) {
        // Environment edits persist through the explicit env-manager save
        // (`save_environment_changes` → UUID-keyed CRUD). The old index-based
        // `save_workspace_async` rewrote every environment with `env-{i}` IDs on
        // each send/switch — clobbering the UUID IDs and racing the background
        // CRUD threads (#22) — so it's no longer driven from the hot path.
        self.persist_endpoint_if_needed();
    }

    fn reload_request_inputs(&mut self, cx: &mut App) {
        let request = self.selected_request().clone();
        self.path_input.update(cx, |input, input_cx| {
            input.set_text(request.path.clone(), input_cx)
        });
        self.params_kv.set_rows(cx, &request.params);
        self.path_kv.set_rows(cx, &request.path_rows);
        self.body_input.update(cx, |input, input_cx| {
            input.set_text(request.body.clone(), input_cx)
        });
        self.headers_kv.set_rows(cx, &request.headers);
        self.cookies_kv.set_rows(cx, &request.cookies);
        self.load_auth_form(cx, &request.auth);
        self.pre_ops_input.update(cx, |input, input_cx| {
            input.set_text(request.pre_ops.clone(), input_cx)
        });
        self.post_ops_input.update(cx, |input, input_cx| {
            input.set_text(request.post_ops.clone(), input_cx)
        });
    }

    fn reload_environment_inputs(&mut self, cx: &mut App) {
        let environment = self.selected_environment().clone();
        self.env_name_input.update(cx, |input, input_cx| {
            input.set_text(environment.name.clone(), input_cx)
        });
        self.env_base_url_input.update(cx, |input, input_cx| {
            input.set_text(environment.base_url.clone(), input_cx)
        });
        self.env_variables_input.update(cx, |input, input_cx| {
            input.set_text(format_rows(&environment.variables), input_cx)
        });
        self.env_headers_input.update(cx, |input, input_cx| {
            input.set_text(format_rows(&environment.headers), input_cx)
        });
    }

    fn ensure_open_tab(&mut self, tab: OpenTab) {
        if !self.open_tabs.contains(&tab) {
            self.open_tabs.push(tab);
        }
    }

    fn collect_tab_draft(&self, cx: &App) -> TabDraft {
        TabDraft {
            url: self.path_input.read(cx).text(),
            params_text: self.params_kv.to_text(cx),
            path_params_text: self.path_kv.to_text(cx),
            body_text: self.body_input.read(cx).text(),
            headers_text: self.headers_kv.to_text(cx),
            cookies_text: self.cookies_kv.to_text(cx),
            auth_text: format_rows(&self.auth_rows(cx)),
            pre_ops_text: self.pre_ops_input.read(cx).text(),
            post_ops_text: self.post_ops_input.read(cx).text(),
            active_request_tab: service::editor_tab_index(self.editor_tab),
        }
    }

    fn persist_current_tab_state(&self, cx: &App) {
        let tab_id = self.active_tab.tab_id().to_string();
        if tab_id.is_empty() {
            return;
        }
        let request = self.selected_request();
        let draft = self.collect_tab_draft(cx);
        let tab = service::build_http_tab(
            &tab_id,
            self.active_tab.node_id(),
            &request.title,
            request.method.label(),
            &draft,
        );
        self.service.save_tab_state_async(tab);
    }

    fn restore_inputs_from_tab(&mut self, tab: &crate::model::HttpTab, cx: &mut App) {
        let draft = service::restore_tab_draft(tab);
        self.path_input
            .update(cx, |input, input_cx| input.set_text(draft.url, input_cx));
        self.params_kv.set_from_text(cx, &draft.params_text);
        self.path_kv.set_from_text(cx, &draft.path_params_text);
        self.body_input.update(cx, |input, input_cx| {
            input.set_text(draft.body_text, input_cx)
        });
        self.headers_kv.set_from_text(cx, &draft.headers_text);
        self.cookies_kv.set_from_text(cx, &draft.cookies_text);
        self.pre_ops_input.update(cx, |input, input_cx| {
            input.set_text(draft.pre_ops_text, input_cx)
        });
        self.post_ops_input.update(cx, |input, input_cx| {
            input.set_text(draft.post_ops_text, input_cx)
        });
        let auth_rows = parse_rows(&draft.auth_text);
        self.load_auth_form(cx, &auth_rows);
    }

    fn close_open_tab(&mut self, tab_index: usize, cx: &mut App) {
        if tab_index >= self.open_tabs.len() {
            return;
        }
        let tab_id = self.open_tabs[tab_index].tab_id().to_string();
        self.service.delete_persisted_tab_async(tab_id);

        let was_active = self.open_tabs[tab_index] == self.active_tab;
        self.open_tabs.remove(tab_index);

        if self.open_tabs.is_empty() {
            self.active_tab = OpenTab::new_request(0);
            self.selected_request = 0;
            self.selected_scenario = None;
            self.open_tabs.push(self.active_tab.clone());
            self.reload_request_inputs(cx);
        } else if was_active {
            let new_index = tab_index.min(self.open_tabs.len() - 1);
            let tab = self.open_tabs[new_index].clone();
            self.active_tab = tab;
            match &self.active_tab {
                OpenTab::Request { index, .. } => {
                    self.selected_request = *index;
                    self.selected_scenario = None;
                }
                OpenTab::Scenario {
                    request_index,
                    scenario_index,
                    ..
                } => {
                    self.selected_request = *request_index;
                    self.selected_scenario = Some(*scenario_index);
                }
            }
            self.reload_request_inputs(cx);
        }
    }

    fn select_request(&mut self, index: usize, cx: &mut App) {
        self.sync_models(cx);
        self.persist_current_tab_state(cx);
        self.selected_request = index;
        self.selected_scenario = None;
        let new_tab = OpenTab::new_request(index);
        let new_tab_id = new_tab.tab_id().to_string();
        self.active_tab = new_tab.clone();
        self.ensure_open_tab(new_tab);

        // Try restoring from persisted draft for this tab
        if let Some(persisted) = self.service.load_persisted_tab_by_id(&new_tab_id) {
            self.restore_inputs_from_tab(&persisted, cx);
            let tab_idx = persisted.active_request_tab;
            if let Some(et) = service::index_to_editor_tab(tab_idx) {
                self.editor_tab = et;
            }
        } else {
            self.reload_request_inputs(cx);
        }

        self.persist_current_tab_state(cx);
        self.notice = format!("已切换到 {}", self.selected_request().title);
    }

    fn select_scenario(&mut self, request_index: usize, scenario_index: usize, cx: &mut App) {
        self.sync_models(cx);
        self.persist_current_tab_state(cx);
        self.selected_request = request_index;
        self.selected_scenario = Some(scenario_index);
        let new_tab = OpenTab::new_scenario(request_index, scenario_index);
        let new_tab_id = new_tab.tab_id().to_string();
        self.active_tab = new_tab.clone();
        self.ensure_open_tab(new_tab);

        if let Some(persisted) = self.service.load_persisted_tab_by_id(&new_tab_id) {
            self.restore_inputs_from_tab(&persisted, cx);
            let tab_idx = persisted.active_request_tab;
            if let Some(et) = service::index_to_editor_tab(tab_idx) {
                self.editor_tab = et;
            }
        } else {
            self.reload_request_inputs(cx);
        }

        self.persist_current_tab_state(cx);
        self.notice = format!("已切换到场景 {}", self.current_title());
    }

    fn select_open_tab(&mut self, tab: OpenTab, cx: &mut App) {
        match &tab {
            OpenTab::Request { index, .. } => self.select_request(*index, cx),
            OpenTab::Scenario {
                request_index,
                scenario_index,
                ..
            } => self.select_scenario(*request_index, *scenario_index, cx),
        }
    }

    fn select_environment(&mut self, index: usize, cx: &mut App) {
        self.sync_models(cx);
        self.persist_workspace();
        self.selected_environment = index;
        self.show_env_popup = false;
        self.reload_environment_inputs(cx);
        self.notice = format!("已切换到 {}", self.selected_environment().name);
    }

    fn set_method(&mut self, method: HttpMethod, cx: &App) {
        self.sync_models(cx);
        let request = self.selected_request_mut();
        request.method = method;
        self.notice = format!("请求方法已切换为 {}", request.method.label());
        self.persist_workspace();
        self.persist_current_tab_state(cx);
    }

    fn init_method_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.method_select_state.is_some() {
            return;
        }

        let methods: Vec<HttpMethod> = HttpMethod::all().to_vec();
        let current = self.selected_request().method;
        let selected_index = methods
            .iter()
            .position(|m| *m == current)
            .map(|i| IndexPath::default().row(i));

        let state = cx.new(|cx| SelectState::new(methods, selected_index, window, cx));
        cx.subscribe(&state, move |this: &mut ApiDebuggerView, _, event: &SelectEvent<Vec<HttpMethod>>, cx| {
            if let SelectEvent::Confirm(Some(method)) = event {
                this.set_method(*method, cx);
            }
        })
        .detach();

        self.method_select_state = Some(state);
    }

    fn send_request(&mut self, cx: &mut App) {
        self.sync_models(cx);
        self.persist_current_tab_state(cx);
        self.persist_workspace();
        let request = self.selected_request().clone();
        let environment = self.selected_environment().clone();
        let pre_ops = request.pre_ops.clone();
        let post_ops = request.post_ops.clone();
        let tab_id = self.active_tab.tab_id().to_string();
        match self
            .service
            .send_request(environment, request, &pre_ops, &post_ops, &tab_id)
        {
            Ok(()) => self.notice = String::from("请求发送中..."),
            Err(error) => self.notice = format!("发送失败: {error}"),
        }
    }

    fn cancel_request(&mut self, _cx: &App) {
        self.service.cancel_request();
        self.notice = String::from("请求已取消");
    }

    /// Pretty-print the JSON body in place. Leaves the body untouched and
    /// surfaces a notice if it does not parse.
    fn format_json_body(&mut self, cx: &mut App) {
        let text = self.body_input.read(cx).text();
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => {
                let pretty =
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.clone());
                self.body_input
                    .update(cx, |input, input_cx| input.set_text(pretty, input_cx));
                self.sync_models(cx);
                self.persist_current_tab_state(cx);
                self.notice = String::from("JSON 已格式化");
            }
            Err(error) => {
                self.notice = format!("JSON 无法解析: {error}");
            }
        }
    }

    /// Pick a file for Binary body mode; stores its path as the body text
    /// (the request path reads the file bytes at send time).
    fn pick_binary_file(&mut self, cx: &mut App) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("选择要上传的文件")
            .pick_file()
        else {
            self.notice = String::from("已取消选择文件");
            return;
        };
        let path_string = path.display().to_string();
        self.body_input
            .update(cx, |input, input_cx| input.set_text(path_string.clone(), input_cx));
        self.sync_models(cx);
        self.persist_current_tab_state(cx);
        self.notice = format!("已选择文件: {path_string}");
    }

    fn current_title(&self) -> String {
        match &self.active_tab {
            OpenTab::Request { index, .. } => request_at(&self.groups, *index)
                .map(|request| request.title.clone())
                .unwrap_or_else(|| String::from("请求")),
            OpenTab::Scenario {
                request_index,
                scenario_index,
                ..
            } => request_at(&self.groups, *request_index)
                .and_then(|request| request.scenarios.get(*scenario_index))
                .map(|scenario| scenario.name.clone())
                .unwrap_or_else(|| String::from("场景")),
        }
    }

    fn current_scenario(&self) -> Option<&ApiScenario> {
        self.selected_scenario
            .and_then(|index| self.selected_request().scenarios.get(index))
    }

    fn tab_title(&self, tab: &OpenTab) -> String {
        match tab {
            OpenTab::Request { index, .. } => request_at(&self.groups, *index)
                .map(|request| request.title.clone())
                .unwrap_or_else(|| String::from("请求")),
            OpenTab::Scenario {
                request_index,
                scenario_index,
                ..
            } => request_at(&self.groups, *request_index)
                .and_then(|request| request.scenarios.get(*scenario_index))
                .map(|scenario| scenario.name.clone())
                .unwrap_or_else(|| String::from("场景")),
        }
    }

    fn response_text(&self) -> String {
        match self.response_tab {
            ResponseTab::Body => self.response.body.clone(),
            ResponseTab::Cookies => {
                if self.response.cookies.trim().is_empty() {
                    String::from("（无 Set-Cookie 响应头）")
                } else {
                    self.response.cookies.clone()
                }
            }
            ResponseTab::Headers => self.response.headers.clone(),
            ResponseTab::Request => self.response.request_dump.clone(),
            ResponseTab::Curl => self.response.curl.clone(),
            ResponseTab::Logs => {
                let mut text = self.response.logs.join("\n");
                if !self.response.assertion_results.is_empty() {
                    text.push_str("\n\n--- 断言 ---\n");
                    for (assertion, passed) in &self.response.assertion_results {
                        let mark = if *passed { "PASS" } else { "FAIL" };
                        text.push_str(&format!("{mark}  {assertion}\n"));
                    }
                }
                text
            }
            // History and Code render their own widgets in `response_panel`;
            // the scrolled text body is only used for Code (the generated snippet).
            ResponseTab::History => String::new(),
            ResponseTab::Code => self.code_snippet(),
        }
    }

    /// Generate the code snippet for the response "代码" tab in the selected
    /// language, using the real (synced) request + environment.
    fn code_snippet(&self) -> String {
        service::code_snippet(
            self.selected_environment(),
            self.selected_request(),
            self.response_code_lang,
        )
    }

    /// Switch the response tab, lazily loading history when 历史 is opened.
    fn set_response_tab(&mut self, tab: ResponseTab) {
        self.response_tab = tab;
        if tab == ResponseTab::History {
            self.refresh_history();
        }
    }

    fn set_response_code_lang(&mut self, lang: CodeLanguage) {
        self.response_code_lang = lang;
    }

    /// Reload the active tab's request history from the database.
    fn refresh_history(&mut self) {
        let tab_id = self.active_tab.tab_id().to_string();
        match self.service.list_history(&tab_id, 50) {
            Ok(rows) => self.history_entries = rows,
            Err(error) => {
                self.history_entries.clear();
                tracing::warn!("加载历史记录失败: {error}");
            }
        }
    }

    fn clear_current_history(&mut self) {
        let tab_id = self.active_tab.tab_id().to_string();
        match self.service.clear_history(&tab_id) {
            Ok(count) => {
                self.history_entries.clear();
                self.notice = format!("已清空 {count} 条历史记录");
            }
            Err(error) => self.notice = format!("清空历史失败: {error}"),
        }
    }

    /// Load a stored history response into the response view (read-only replay).
    fn view_history_entry(&mut self, index: usize) {
        let Some(entry) = self.history_entries.get(index) else {
            return;
        };
        let created_at = entry.created_at.clone();
        self.response.status_line =
            format!("{} {} · {}", entry.method, entry.status, entry.url);
        self.response.status_code = entry.status.max(0) as u16;
        self.response.body = entry.response.clone();
        self.response.headers = String::new();
        self.response.cookies = String::new();
        self.response.content_type = String::new();
        self.response.assertion_results = Vec::new();
        self.response.logs = vec![format!("历史响应 @ {created_at}")];
        self.response_tab = ResponseTab::Body;
        self.notice = format!("已载入历史响应（{created_at}）");
    }

    fn copy_response_body(&mut self, cx: &mut App) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(self.response.body.clone()));
        self.notice = String::from("响应已复制到剪贴板");
    }

    /// Pretty-print the response body JSON in place (no-op for non-JSON).
    fn format_response_body(&mut self) {
        match serde_json::from_str::<serde_json::Value>(&self.response.body) {
            Ok(value) => match serde_json::to_string_pretty(&value) {
                Ok(pretty) => {
                    self.response.body = pretty;
                    self.notice = String::from("响应 JSON 已格式化");
                }
                Err(error) => self.notice = format!("格式化失败: {error}"),
            },
            Err(error) => self.notice = format!("响应不是合法 JSON: {error}"),
        }
    }

    /// Save the response body to a file via a native dialog. The suggested
    /// extension follows the response `Content-Type`.
    fn save_response_body(&mut self) {
        let suggested = format!(
            "response.{}",
            content_type_extension(&self.response.content_type)
        );
        let Some(path) = rfd::FileDialog::new()
            .set_title("保存响应到文件")
            .set_file_name(&suggested)
            .save_file()
        else {
            self.notice = String::from("已取消保存");
            return;
        };
        match std::fs::write(&path, self.response.body.as_bytes()) {
            Ok(()) => self.notice = format!("响应已保存: {}", path.display()),
            Err(error) => self.notice = format!("保存失败: {error}"),
        }
    }

    /// Text input backing a free-text editor tab (Body / Pre-ops / Post-ops).
    /// KV-table tabs (Params / Path / Headers / Cookies) and the Auth form tab
    /// return `None` — they render their own editors.
    fn text_editor_input(&self, tab: EditorTab) -> Option<Entity<TextInput>> {
        match tab {
            EditorTab::Body => Some(self.body_input.clone()),
            EditorTab::PreOps => Some(self.pre_ops_input.clone()),
            EditorTab::PostOps => Some(self.post_ops_input.clone()),
            EditorTab::Auth
            | EditorTab::Params
            | EditorTab::Path
            | EditorTab::Headers
            | EditorTab::Cookies => None,
        }
    }

    fn auth_form_inputs(&self) -> AuthFormInputs {
        AuthFormInputs {
            bearer: self.auth_bearer_input.clone(),
            basic_user: self.auth_basic_user_input.clone(),
            basic_pass: self.auth_basic_pass_input.clone(),
            apikey_name: self.auth_apikey_name_input.clone(),
            apikey_value: self.auth_apikey_value_input.clone(),
            in_query: self.auth_apikey_in_query,
        }
    }

    /// Build the canonical auth rows (header/query pairs) from the current
    /// auth type and form inputs. Basic credentials are base64-encoded here so
    /// the request path can send them verbatim. API keys placed in the query
    /// string carry `description = "query"`.
    fn auth_rows(&self, cx: &App) -> Vec<KeyValueRow> {
        match self.auth_type {
            AuthType::None => Vec::new(),
            AuthType::BearerToken => {
                let token = self.auth_bearer_input.read(cx).text().trim().to_string();
                if token.is_empty() {
                    Vec::new()
                } else {
                    vec![KeyValueRow::new("Authorization", format!("Bearer {token}"))]
                }
            }
            AuthType::BasicAuth => {
                let user = self.auth_basic_user_input.read(cx).text();
                let pass = self.auth_basic_pass_input.read(cx).text();
                if user.trim().is_empty() && pass.trim().is_empty() {
                    Vec::new()
                } else {
                    let encoded =
                        service::base64_encode(format!("{user}:{pass}").as_bytes());
                    vec![KeyValueRow::new("Authorization", format!("Basic {encoded}"))]
                }
            }
            AuthType::ApiKey => {
                let name = self.auth_apikey_name_input.read(cx).text().trim().to_string();
                let value = self.auth_apikey_value_input.read(cx).text().trim().to_string();
                if name.is_empty() {
                    Vec::new()
                } else {
                    let mut row = KeyValueRow::new(name, value);
                    row.description = if self.auth_apikey_in_query {
                        String::from("query")
                    } else {
                        String::from("header")
                    };
                    vec![row]
                }
            }
        }
    }

    /// Seed the auth type and form inputs from canonical auth rows.
    fn load_auth_form(&mut self, cx: &mut App, rows: &[KeyValueRow]) {
        let values = derive_auth_form(rows);
        self.auth_type = values.auth_type.unwrap_or(AuthType::None);
        self.auth_apikey_in_query = values.in_query;
        self.auth_bearer_input.update(cx, |input, input_cx| {
            input.set_text(values.bearer.clone(), input_cx)
        });
        self.auth_basic_user_input.update(cx, |input, input_cx| {
            input.set_text(values.basic_user.clone(), input_cx)
        });
        self.auth_basic_pass_input.update(cx, |input, input_cx| {
            input.set_text(values.basic_pass.clone(), input_cx)
        });
        self.auth_apikey_name_input.update(cx, |input, input_cx| {
            input.set_text(values.apikey_name.clone(), input_cx)
        });
        self.auth_apikey_value_input.update(cx, |input, input_cx| {
            input.set_text(values.apikey_value.clone(), input_cx)
        });
    }

    fn kv_editor(&self, tab: EditorTab) -> Option<&KvEditor> {
        match tab {
            EditorTab::Params => Some(&self.params_kv),
            EditorTab::Path => Some(&self.path_kv),
            EditorTab::Headers => Some(&self.headers_kv),
            EditorTab::Cookies => Some(&self.cookies_kv),
            _ => None,
        }
    }

    fn kv_editor_mut(&mut self, tab: EditorTab) -> Option<&mut KvEditor> {
        match tab {
            EditorTab::Params => Some(&mut self.params_kv),
            EditorTab::Path => Some(&mut self.path_kv),
            EditorTab::Headers => Some(&mut self.headers_kv),
            EditorTab::Cookies => Some(&mut self.cookies_kv),
            _ => None,
        }
    }

    fn save_environment_changes(&mut self, cx: &mut App) {
        self.sync_models(cx);
        let env = self.selected_environment().clone();
        self.service.save_environment_fields_async(
            self.selected_environment,
            env.name.clone(),
            env.base_url.clone(),
            format_rows(&env.variables),
            format_rows(&env.headers),
        );
        self.show_env_manager = false;
        self.notice = String::from("正在保存环境...");
    }

    fn reset_environment_changes(&mut self, cx: &mut App) {
        self.reload_environment_inputs(cx);
        self.notice = String::from("已重置环境编辑内容");
    }

    fn create_new_environment(&mut self) {
        self.service
            .create_environment_async(String::from("新环境"), String::new());
        self.notice = String::from("正在创建环境...");
    }

    fn duplicate_current_environment(&mut self, cx: &mut App) {
        self.sync_models(cx);
        self.service
            .duplicate_environment_async(self.selected_environment);
        self.notice = String::from("正在复制环境...");
    }

    fn delete_current_environment(&mut self, cx: &mut App) {
        self.sync_models(cx);
        self.service
            .delete_environment_by_index_async(self.selected_environment);
        self.notice = String::from("正在删除环境...");
    }

    fn open_collection_menu(&mut self, title: impl Into<String>, position: Option<(f32, f32)>, node_id: String) {
        self.collection_menu_title = title.into();
        self.collection_menu_position = position;
        self.collection_menu_node_id = node_id;
        self.show_collection_menu = true;
        self.show_env_popup = false;
        self.show_env_manager = false;
    }

    fn close_collection_menu(&mut self) {
        self.show_collection_menu = false;
        self.collection_menu_position = None;
        self.collection_menu_node_id = String::new();
    }

    fn create_new_endpoint(&mut self) {
        let parent_id = self.find_parent_id_for_new_node();
        let title = String::from("新请求");
        self.service
            .create_endpoint_async(parent_id, title, "GET".into(), "/".into());
        self.close_collection_menu();
    }

    fn create_new_folder(&mut self) {
        let parent_id = self.find_parent_id_for_new_node();
        let title = String::from("新分组");
        self.service.create_folder_async(parent_id, title);
        self.close_collection_menu();
    }

    /// Create a test case under the currently selected (persisted) endpoint. The
    /// new case appears as a scenario row beneath that request once the tree
    /// reloads.
    fn create_new_case(&mut self) {
        let parent_id = request_at(&self.groups, self.selected_request)
            .map(|request| request.node_id.clone())
            .unwrap_or_default();
        if parent_id.is_empty() {
            self.notice = String::from("请先选择一个已保存的端点再添加用例");
            self.close_collection_menu();
            return;
        }
        self.service
            .create_case_async(parent_id, String::from("新用例"));
        self.close_collection_menu();
    }

    fn delete_selected_collection_item(&mut self) {
        let node_id = self.collection_menu_node_id.clone();
        if !node_id.is_empty() {
            self.service.delete_collection_item_async(node_id);
        }
        self.close_collection_menu();
    }

    fn find_parent_id_for_new_node(&self) -> Option<String> {
        let request = self.selected_request();
        if request.node_id.is_empty() {
            None
        } else {
            Some(request.node_id.clone())
        }
    }

    fn import_curl(&mut self, cx: &App) {
        let curl_text = self.curl_import_input.read(cx).text();
        if !curl_text.is_empty() {
            self.service.import_from_curl_async(curl_text);
        }
        self.show_curl_import = false;
    }

    fn export_openapi(&mut self) {
        let json = match self.service.export_collection_as_openapi() {
            Ok(json) => json,
            Err(error) => {
                self.notice = format!("导出失败: {error}");
                self.close_collection_menu();
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title("导出为 OpenAPI")
            .set_file_name("openapi.json")
            .save_file()
        else {
            self.notice = String::from("已取消导出");
            self.close_collection_menu();
            return;
        };
        match std::fs::write(&path, json) {
            Ok(()) => self.notice = format!("已导出到 {}", path.display()),
            Err(error) => self.notice = format!("写入文件失败: {error}"),
        }
        self.close_collection_menu();
    }

    /// Pick an OpenAPI document (JSON or YAML) and import its endpoints into the
    /// collection tree. The parse + insert run on a background thread; the tree
    /// refreshes when the service publishes its completion notice.
    fn import_openapi_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("选择 OpenAPI 文件 (JSON / YAML)")
            .pick_file()
        else {
            self.close_collection_menu();
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => self.service.import_from_openapi_async(content),
            Err(error) => self.notice = format!("读取文件失败: {error}"),
        }
        self.close_collection_menu();
    }

    /// Pick a Postman Collection (v2.1 JSON) and import its requests.
    fn import_postman_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("选择 Postman Collection 文件")
            .pick_file()
        else {
            self.close_collection_menu();
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => self.service.import_from_postman_async(content),
            Err(error) => self.notice = format!("读取文件失败: {error}"),
        }
        self.close_collection_menu();
    }

    /// Open the rename dialog for the node the context menu was opened on,
    /// pre-filling the input with its current name.
    fn open_rename(&mut self, cx: &mut App) {
        let node_id = self.collection_menu_node_id.clone();
        if node_id.is_empty() {
            self.notice = String::from("请在具体节点上重命名");
            self.close_collection_menu();
            return;
        }
        let current_name = self
            .service
            .get_collection_node(&node_id)
            .ok()
            .flatten()
            .map(|node| node.name)
            .unwrap_or_default();
        self.rename_node_id = node_id;
        self.rename_input
            .update(cx, |input, input_cx| input.set_text(current_name, input_cx));
        self.show_rename = true;
        self.close_collection_menu();
    }

    fn confirm_rename(&mut self, cx: &App) {
        let new_name = self.rename_input.read(cx).text().trim().to_string();
        let node_id = self.rename_node_id.clone();
        if node_id.is_empty() {
            // nothing selected — just dismiss
        } else if new_name.is_empty() {
            self.notice = String::from("名称不能为空");
            return;
        } else {
            self.service.rename_collection_item_async(node_id, new_name);
        }
        self.show_rename = false;
        self.rename_node_id = String::new();
    }
}

impl Render for ApiDebuggerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_service_updates();
        self.init_method_select(window, cx);

        let dark = qingqi_ui::theme_mode::is_dark();
        let stacked = window.bounds().size.width < px(STACK_BREAKPOINT_PX);

        let entity = cx.entity();
        let groups = self.groups.clone();
        let environments = self.environments.clone();
        let open_tabs = self.open_tabs.clone();
        let active_tab = self.active_tab.clone();
        let selected_request = self.selected_request;
        let selected_scenario = self.selected_scenario;
        let selected_environment = self.selected_environment;
        let editor_tab = self.editor_tab;
        let body_mode = self.body_mode;
        let auth_type = self.auth_type;
        let response_tab = self.response_tab;
        let env_detail_tab = self.env_detail_tab;
        let show_env_popup = self.show_env_popup;
        let show_env_manager = self.show_env_manager;
        let show_collection_menu = self.show_collection_menu;
        let show_curl_import = self.show_curl_import;
        let method_select = self.method_select_state.as_ref().expect("init").clone();
        let collection_menu_title = self.collection_menu_title.clone();
        let collection_menu_position = self.collection_menu_position;
        let collection_menu_node_id = self.collection_menu_node_id.clone();
        let path_input = self.path_input.clone();
        let editor_text_input = self.text_editor_input(editor_tab);
        let editor_kv_rows = self
            .kv_editor(editor_tab)
            .map(|editor| editor.rows.clone())
            .unwrap_or_default();
        let editor_auth_form = self.auth_form_inputs();
        let env_name_input = self.env_name_input.clone();
        let curl_import_input = self.curl_import_input.clone();
        let show_rename = self.show_rename;
        let rename_input = self.rename_input.clone();
        let env_base_url_input = self.env_base_url_input.clone();
        let env_variables_input = self.env_variables_input.clone();
        let env_headers_input = self.env_headers_input.clone();
        let response = self.response.clone();
        let response_text = self.response_text();
        let response_history = self.history_entries.clone();
        let response_code_lang = self.response_code_lang;
        let notice = self.notice.clone();
        let current_request = self.selected_request().clone();
        let current_environment = self.selected_environment().clone();
        let current_scenario = self.current_scenario().cloned();
        let tab_titles = open_tabs
            .iter()
            .map(|tab| self.tab_title(tab))
            .collect::<Vec<_>>();
        let in_flight = self.service.is_in_flight();
        let chrome = crate::mac_ui::workspace_chrome_config();

        let esc_view = entity.clone();

        div()
            .relative()
            .size_full()
            .bg(theme::semantic().bg_glass)
            .rounded(px(12.0))
            .overflow_hidden()
            .font_family("Inter, PingFang SC")
            .text_color(theme::semantic().text_primary)
            .on_key_down(move |event, _window, cx| {
                if event.keystroke.key == "escape" {
                    esc_view.update(cx, |view, _cx| {
                        view.show_env_popup = false;
                        view.show_env_manager = false;
                        view.show_collection_menu = false;
                        view.show_curl_import = false;
                    });
                }
            })
            .child(
                div()
                    .size_full()
                    .relative()
                    .pt(px(chrome.metrics().content_top_padding + 6.0))
                    .pl(px(8.0))
                    .pr(px(8.0))
                    .pb(px(8.0))
                    .flex()
                    .gap(px(if stacked { 8.0 } else { 8.0 }))
                    .when(stacked, |layout| layout.flex_col())
                    .when(!stacked, |layout| layout.flex_row())
                    .child(collection_tree(
                        entity.clone(),
                        groups,
                        selected_request,
                        selected_scenario,
                        dark,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .border_1()
                            .border_color(glass::border(dark))
                            .bg(theme::semantic().bg_surface)
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(open_tabs_bar(
                                entity.clone(),
                                open_tabs,
                                active_tab,
                                tab_titles,
                                current_environment.clone(),
                                dark,
                            ))
                            .child(action_bar(
                                entity.clone(),
                                current_request.clone(),
                                current_environment.clone(),
                                path_input,
                                in_flight,
                                dark,
                                method_select,
                            ))
                            .when(current_scenario.is_some(), |content| {
                                content.child(scenario_banner(
                                    current_scenario.expect("scenario should exist"),
                                    current_request.clone(),
                                    dark,
                                ))
                            })
                            .child(
                                content_split(stacked)
                                    .child(editor_panel(
                                        entity.clone(),
                                        editor_tab,
                                        editor_text_input,
                                        editor_kv_rows,
                                        editor_auth_form,
                                        body_mode,
                                        auth_type,
                                        dark,
                                    ))
                                    .child(response_panel(
                                        entity.clone(),
                                        response_tab,
                                        response,
                                        response_text,
                                        response_history,
                                        response_code_lang,
                                        notice,
                                        dark,
                                    )),
                            ),
                    ),
            )
            .child(if show_env_popup {
                overlay_shell(
                    dark,
                    "api-env-popup-backdrop",
                    {
                        let entity = entity.clone();
                        move |_, cx| {
                            entity.update(cx, |view, _cx| view.show_env_popup = false);
                        }
                    },
                    env_popup(
                        entity.clone(),
                        environments.clone(),
                        selected_environment,
                        dark,
                    ),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if show_env_manager {
                overlay_shell(
                    dark,
                    "api-env-manager-backdrop",
                    {
                        let entity = entity.clone();
                        move |_, cx| {
                            entity.update(cx, |view, _cx| {
                                view.show_env_manager = false;
                                view.show_env_popup = false;
                            });
                        }
                    },
                    env_manager_dialog(
                        entity.clone(),
                        selected_environment,
                        env_detail_tab,
                        environments,
                        env_name_input,
                        env_base_url_input,
                        env_variables_input,
                        env_headers_input,
                        dark,
                    ),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if show_collection_menu {
                context_menu_overlay(
                    entity.clone(),
                    collection_menu_title,
                    collection_menu_position,
                    collection_menu_node_id,
                    dark,
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if show_curl_import {
                overlay_shell(
                    dark,
                    "api-curl-import-backdrop",
                    {
                        let view = entity.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.show_curl_import = false);
                        }
                    },
                    curl_import_dialog(entity.clone(), curl_import_input, dark),
                )
                .into_any_element()
             } else {
                div().into_any_element()
            })
            .child(if show_rename {
                overlay_shell(
                    dark,
                    "api-rename-backdrop",
                    {
                        let view = entity.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.show_rename = false);
                        }
                    },
                    rename_dialog(entity.clone(), rename_input, dark),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(ui::popup_window_chrome_with_titlebar_slot(
                chrome,
                Some(titlebar_new_button(entity.clone(), dark).into_any_element()),
            ))
    }
}

fn titlebar_new_button(
    view: Entity<ApiDebuggerView>,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            Button::new("api-titlebar-new")
                .ghost()
                .icon(IconName::Plus)
                .with_size(Size::XSmall)
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, _cx| {
                            view.open_collection_menu("新建", None, String::new());
                        });
                    }
                }),
        )
}

fn collection_tree(
    view: Entity<ApiDebuggerView>,
    groups: Vec<ApiGroup>,
    selected_request: usize,
    selected_scenario: Option<usize>,
    dark: bool,
) -> impl IntoElement {
    let mut request_index = 0usize;
    div()
        .w(px(260.0))
        .min_h(px(220.0))
        .flex_none()
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
        .rounded(px(8.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .id("api-tree-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(3.0))
                .py(px(4.0))
                .children(groups.into_iter().map(|group| {
                    let start = request_index;
                    request_index += group.requests.len();
                    group_section(
                        view.clone(),
                        group,
                        start,
                        selected_request,
                        selected_scenario,
                        dark,
                    )
                })),
        )
}

fn group_section(
    view: Entity<ApiDebuggerView>,
    group: ApiGroup,
    request_start: usize,
    selected_request: usize,
    selected_scenario: Option<usize>,
    dark: bool,
) -> impl IntoElement {
    let group_name = group.name.clone();
    let group_id = group.id.clone().unwrap_or_default();
    div()
        .px(px(8.0))
        .py(px(2.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .id(("api-group-row", request_start))
                .px(px(6.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .hover({
                    move |style| {
                        style
                            .bg(theme::rgba_with_alpha(api_accent(), 0.05))
                            .cursor_context_menu()
                    }
                })
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_tertiary())
                        .child("▾"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(ui::text_secondary())
                        .truncate()
                        .child(group.name.clone()),
                )
                .child(group_count(group.requests.len()))
                .child(
                    div()
                        .px(px(3.0))
                        .text_size(px(11.0))
                        .text_color(ui::text_tertiary())
                        .child("⋯"),
                )
                .on_mouse_down(MouseButton::Right, {
                    let view = view.clone();
                    let group_name = group_name.clone();
                    let gid = group_id.clone();
                    move |event, window, cx| {
                        view.update(cx, |view, _cx| {
                            view.open_collection_menu(
                                group_name.clone(),
                                Some((event.position.x.into(), event.position.y.into())),
                                gid.clone(),
                            );
                        });
                        cx.stop_propagation();
                        window.refresh();
                    }
                })
                .on_click({
                    let view = view.clone();
                    let group_name = group_name.clone();
                    let gid = group_id.clone();
                    move |event, _window, cx| {
                        if event.is_right_click() {
                            view.update(cx, |view, _cx| {
                                view.open_collection_menu(
                                    group_name.clone(),
                                    Some((event.position().x.into(), event.position().y.into())),
                                    gid.clone(),
                                );
                            });
                            cx.stop_propagation();
                        }
                    }
                }),
        )
        .children(
            group
                .requests
                .into_iter()
                .enumerate()
                .map(move |(offset, request)| {
                    request_tree_block(
                        view.clone(),
                        request_start + offset,
                        request,
                        selected_request,
                        selected_scenario,
                        dark,
                    )
                }),
        )
}

fn request_tree_block(
    view: Entity<ApiDebuggerView>,
    request_index: usize,
    request: ApiRequest,
    selected_request: usize,
    selected_scenario: Option<usize>,
    dark: bool,
) -> impl IntoElement {
    let request_active = selected_request == request_index && selected_scenario.is_none();
    let request_title = request.title.clone();
    let scenario_count = request.scenarios.len();
    div()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .child(
            div()
                .id(("api-request-row", request_index))
                .min_h(px(32.0))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(if request_active {
                    theme::rgba_with_alpha(api_accent(), 0.16)
                } else {
                    transparent_surface()
                })
                .bg(if request_active {
                    theme::rgba_with_alpha(api_accent(), 0.08)
                } else {
                    transparent_surface()
                })
                .hover(move |style| {
                    style
                        .bg(theme::rgba_with_alpha(api_accent(), 0.05))
                        .cursor_context_menu()
                })
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(method_badge(request.method, dark))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(12.0))
                        .text_color(if request_active {
                            api_accent()
                        } else {
                            theme::semantic().text_body
                        })
                        .truncate()
                        .child(request.title.clone()),
                )
                .when(scenario_count > 0, |row| {
                    row.child(scenario_count_badge(scenario_count, dark))
                })
                .on_click({
                    let view = view.clone();
                    move |_, window, cx| {
                        view.update(cx, |view, cx| view.select_request(request_index, cx));
                        window.refresh();
                    }
                })
                .on_mouse_down(MouseButton::Right, {
                    let view = view.clone();
                    let request_title = request_title.clone();
                    let nid = request.node_id.clone();
                    move |event, window, cx| {
                        view.update(cx, |view, _cx| {
                            view.open_collection_menu(
                                request_title.clone(),
                                Some((event.position.x.into(), event.position.y.into())),
                                nid.clone(),
                            );
                        });
                        cx.stop_propagation();
                        window.refresh();
                    }
                }),
        )
        .children(request.scenarios.into_iter().enumerate().map(
            move |(scenario_index, scenario)| {
                let active =
                    selected_request == request_index && selected_scenario == Some(scenario_index);
                div()
                    .id(("api-scenario-row", request_index * 100 + scenario_index))
                    .min_h(px(26.0))
                    .px(px(8.0))
                    .py(px(3.0))
                    .pl(px(28.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(if active {
                        theme::rgba_with_alpha(api_accent(), 0.16)
                    } else {
                        transparent_surface()
                    })
                    .bg(if active {
                        theme::rgba_with_alpha(api_accent(), 0.08)
                    } else {
                        transparent_surface()
                    })
                    .hover(move |style| {
                        style
                            .bg(theme::rgba_with_alpha(api_accent(), 0.05))
                            .cursor_pointer()
                    })
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(status_dot(scenario.status, dark))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_size(px(11.0))
                            .text_color(if active {
                                api_accent()
                            } else {
                                theme::semantic().text_body
                            })
                            .truncate()
                            .child(scenario.name.clone()),
                    )
                    .child(scenario_status_pill(scenario.status, dark))
                    .on_click({
                        let view = view.clone();
                        move |_, window, cx| {
                            view.update(cx, |view, cx| {
                                view.select_scenario(request_index, scenario_index, cx);
                            });
                            window.refresh();
                        }
                    })
            },
        ))
}

fn open_tabs_bar(
    view: Entity<ApiDebuggerView>,
    tabs: Vec<OpenTab>,
    active_tab: OpenTab,
    titles: Vec<String>,
    environment: ApiEnvironment,
    dark: bool,
) -> impl IntoElement {
    let tabs_view = view.clone();
    let env_view = view.clone();
    div()
        .h(px(36.0))
        .px(px(10.0))
        .border_b_1()
        .border_color(ui::border_light())
        .flex()
        .items_center()
        .gap(px(6.0))
        .bg(theme::semantic().bg_surface)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(px(2.0))
                .overflow_x_hidden()
                .children(tabs.into_iter().enumerate().map(move |(index, tab)| {
                    let active = tab == active_tab;
                    let click_view = tabs_view.clone();
                    let close_view = tabs_view.clone();
                    div()
                        .id(("api-open-tab", index))
                        .h(px(28.0))
                        .px(px(10.0))
                        .rounded(px(6.0))
                        .border_b_2()
                        .border_color(if active {
                            api_accent()
                        } else {
                            theme::rgba_with_alpha(theme::semantic().bg_surface, 0.0).into()
                        })
                        .bg(if active {
                            theme::rgba_with_alpha(api_accent(), 0.06)
                        } else {
                            transparent_surface()
                        })
                        .text_size(px(11.0))
                        .font_weight(if active {
                            gpui::FontWeight::SEMIBOLD
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .text_color(if active {
                            api_accent()
                        } else {
                            ui::text_tertiary()
                        })
                        .hover(move |style| {
                            style
                                .bg(glass::hover_bg(dark))
                                .cursor_pointer()
                        })
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div().max_w(px(180.0)).truncate().child(
                                titles
                                    .get(index)
                                    .cloned()
                                    .unwrap_or_else(|| String::from("请求")),
                            ),
                        )
                        .child(
                            Button::new(("api-tab-close", index))
                                .ghost()
                                .icon(IconName::Close)
                                .with_size(Size::XSmall)
                                .on_click({
                                    let view = close_view.clone();
                                    move |_event, _window, cx| {
                                        cx.stop_propagation();
                                        view.update(cx, |view, cx| view.close_open_tab(index, cx));
                                    }
                                }),
                        )
                        .on_click({
                            let view = click_view.clone();
                            move |_, window, cx| {
                                view.update(cx, |view, cx| {
                                    view.select_open_tab(tab.clone(), cx);
                                });
                                window.refresh();
                            }
                        })
                })),
        )
        .child(
            div()
                .id("api-current-env")
                .h(px(28.0))
                .px(px(10.0))
                .rounded(px(999.0))
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.48 } else { 0.74 },
                ))
                .text_size(px(11.0))
                .text_color(ui::text_secondary())
                .hover(move |style| {
                    style
                        .bg(theme::rgba_with_alpha(api_accent(), 0.06))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .size(px(8.0))
                        .rounded(px(999.0))
                        .bg(rgb(environment.color)),
                )
                .child(environment.name.clone())
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(ui::text_tertiary())
                        .child("▾"),
                )
                .on_click({
                    move |_, window, cx| {
                        env_view.update(cx, |view, _cx| {
                            view.show_env_popup = true;
                            view.show_env_manager = false;
                        });
                        window.refresh();
                    }
                }),
        )
}

fn action_bar(
    view: Entity<ApiDebuggerView>,
    _request: ApiRequest,
    environment: ApiEnvironment,
    path_input: Entity<TextInput>,
    in_flight: bool,
    dark: bool,
    method_select: Entity<SelectState<Vec<HttpMethod>>>,
) -> impl IntoElement {
    div()
        .px(px(10.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(ui::border_light())
        .bg(theme::semantic().bg_subtle)
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .w(px(100.0))
                .child(Select::new(&method_select).appearance(false).with_size(Size::Small)),
        )
        .child({
            let url_view = view.clone();
            div()
                .flex_1()
                .h(px(32.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_subtle_2)
                .px(px(10.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .font_family("SF Mono")
                        .text_size(px(11.0))
                        .text_color(ui::text_tertiary())
                        .child(environment.base_url),
                )
                .child(div().flex_1().min_w(px(0.0)).child(path_input))
                .on_key_down(move |event, _window, cx| {
                    if event.keystroke.key == "enter" {
                        url_view.update(cx, |view, cx| view.send_request(cx));
                    }
                })
        })
        .child(if in_flight {
            Button::new("api-cancel-btn")
                .danger()
                .label("取消")
                .with_size(Size::Small)
                .on_click(move |_, _window, cx| {
                    view.update(cx, |view, cx| view.cancel_request(cx));
                })
                .into_any_element()
        } else {
            Button::new("api-send-btn")
                .primary()
                .icon(IconName::ArrowRight)
                .label("发送")
                .with_size(Size::Small)
                .on_click({
                    let view = view.clone();
                    move |_, _window, cx| {
                        view.update(cx, |view, cx| view.send_request(cx));
                    }
                })
                .into_any_element()
        })
}

fn scenario_banner(scenario: ApiScenario, request: ApiRequest, dark: bool) -> impl IntoElement {
    div()
        .px(px(12.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(api_accent(), 0.06))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(api_accent())
                .child(format!("{}", scenario.name)),
        )
        .child(scenario_status_label(scenario.status, dark))
        .child(div().flex_1())
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::semantic().text_secondary)
                .child(format!("基于 {} {}", request.method.label(), request.title)),
        )
}

fn editor_panel(
    view: Entity<ApiDebuggerView>,
    editor_tab: EditorTab,
    text_input: Option<Entity<TextInput>>,
    kv_rows: Vec<KvRow>,
    auth_form: AuthFormInputs,
    body_mode: BodyMode,
    auth_type: AuthType,
    dark: bool,
) -> impl IntoElement {
    let label = editor_tab.label();
    let tabs_view = view.clone();
    let subtoolbar_view = view.clone();
    let mode_row = match editor_tab {
        EditorTab::Body => {
            let bm_view = subtoolbar_view.clone();
            let modes = BodyMode::all();
            let mut row = div()
                .px(px(10.0))
                .py(px(4.0))
                .flex()
                .items_center()
                .gap(px(4.0));
            for (i, mode) in modes.iter().enumerate() {
                let label = mode.label();
                let is_active = mode == &body_mode;
                let bm_click = bm_view.clone();
                let mode_val = mode.as_str().to_string();
                row = row.child(
                    div()
                        .id(("api-body-mode-btn", i))
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded(px(4.0))
                        .text_size(px(10.0))
                        .text_color(if is_active {
                            api_accent()
                        } else {
                            ui::text_tertiary()
                        })
                        .bg(if is_active {
                            theme::rgba_with_alpha(theme::semantic().primary, 0.12)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(move |style| {
                            style.cursor_pointer().text_color(api_accent())
                        })
                        .on_click(move |_, _window, cx| {
                            bm_click.update(cx, |view, cx| {
                                view.sync_models(cx);
                                view.body_mode = BodyMode::from_db(&mode_val);
                                view.persist_current_tab_state(cx);
                            });
                        })
                        .child(label),
                );
            }
            // Mode-specific actions on the right (format JSON / pick file).
            row = row.child(div().flex_1());
            match body_mode {
                BodyMode::Json => {
                    let fmt_view = bm_view.clone();
                    row = row.child(
                        Button::new("api-body-format-json")
                            .ghost()
                            .label("格式化")
                            .with_size(Size::XSmall)
                            .on_click(move |_, _, cx| {
                                fmt_view.update(cx, |view, cx| view.format_json_body(cx));
                            }),
                    );
                }
                BodyMode::Binary => {
                    let pick_view = bm_view.clone();
                    row = row.child(
                        Button::new("api-body-pick-file")
                            .ghost()
                            .label("选择文件")
                            .with_size(Size::XSmall)
                            .on_click(move |_, _, cx| {
                                pick_view.update(cx, |view, cx| view.pick_binary_file(cx));
                            }),
                    );
                }
                _ => {}
            }
            row.into_any_element()
        }
        EditorTab::Auth => {
            let au_view = subtoolbar_view.clone();
            let types = AuthType::all();
            let mut row = div()
                .px(px(10.0))
                .py(px(4.0))
                .flex()
                .items_center()
                .gap(px(4.0));
            for (i, at) in types.iter().enumerate() {
                let label = at.label();
                let is_active = at == &auth_type;
                let au_click = au_view.clone();
                let at_val = at.as_str().to_string();
                row = row.child(
                    div()
                        .id(("api-auth-type-btn", i))
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded(px(4.0))
                        .text_size(px(10.0))
                        .text_color(if is_active {
                            api_accent()
                        } else {
                            ui::text_tertiary()
                        })
                        .bg(if is_active {
                            theme::rgba_with_alpha(theme::semantic().primary, 0.12)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(move |style| {
                            style.cursor_pointer().text_color(api_accent())
                        })
                        .on_click(move |_, _window, cx| {
                            au_click.update(cx, |view, cx| {
                                view.auth_type = AuthType::from_db(&at_val);
                                view.sync_models(cx);
                                view.persist_current_tab_state(cx);
                            });
                        })
                        .child(label),
                );
            }
            row.into_any_element()
        }
        _ => div().into_any_element(),
    };

    let primary_tabs = [EditorTab::Params, EditorTab::Headers, EditorTab::Body];
    let more_tabs = [
        EditorTab::Auth,
        EditorTab::Cookies,
        EditorTab::Path,
        EditorTab::PreOps,
        EditorTab::PostOps,
    ];
    let more_view = view.clone();

    div()
        .flex_1()
        .min_w(px(320.0))
        .border_r_1()
        .border_color(ui::border_light())
        .bg(theme::semantic().bg_surface)
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_subtle)
                .flex()
                .items_center()
                .gap(px(2.0))
                .children(primary_tabs.into_iter().enumerate().map(
                    move |(index, tab)| {
                        let active = tab == editor_tab;
                        let tab_view = tabs_view.clone();
                        div()
                            .id(("api-editor-tab", index))
                            .px(px(10.0))
                            .py(px(6.0))
                            .border_b_2()
                            .border_color(if active {
                                api_accent()
                            } else {
                                theme::rgba_with_alpha(theme::semantic().bg_surface, 0.0).into()
                            })
                            .text_size(px(11.0))
                            .text_color(if active {
                                api_accent()
                            } else {
                                ui::text_tertiary()
                            })
                            .hover(move |style| style.text_color(api_accent()).cursor_pointer())
                            .child(tab.label())
                            .on_click({
                                move |_, _window, cx| {
                                    tab_view.update(cx, |view, cx| {
                                        view.sync_models(cx);
                                        view.persist_current_tab_state(cx);
                                        view.editor_tab = tab;
                                        view.persist_current_tab_state(cx);
                                    });
                                }
                            })
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .id("api-editor-more")
                                .px(px(8.0))
                                .py(px(6.0))
                                .rounded(px(4.0))
                                .text_size(px(11.0))
                                .text_color(ui::text_tertiary())
                                .hover(move |style| {
                                    style.text_color(api_accent()).cursor_pointer()
                                })
                                .flex()
                                .items_center()
                                .gap(px(3.0))
                                .child("更多")
                                .child("▾")
                                .on_click({
                                    let view = more_view.clone();
                                    move |_, _window, cx| {
                                        view.update(cx, |view, cx| {
                                            view.sync_models(cx);
                                            view.persist_current_tab_state(cx);
                                            view.editor_tab = EditorTab::Auth;
                                            view.persist_current_tab_state(cx);
                                        });
                                    }
                                }),
                        ),
                ),
        )
        .child(mode_row)
        .child(
            div()
                .id("api-editor-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(4.0))
                .p(px(10.0))
                .child(match editor_tab {
                    EditorTab::Params
                    | EditorTab::Headers
                    | EditorTab::Path
                    | EditorTab::Cookies => {
                        kv_editor_table(view.clone(), editor_tab, kv_rows, dark).into_any_element()
                    }
                    EditorTab::Auth => {
                        auth_form_panel(view.clone(), auth_type, auth_form, dark).into_any_element()
                    }
                    _ => {
                        let input =
                            text_input.expect("non-KV editor tab must have a text input");
                        // Mode-specific hint for the body editor's text formats.
                        let hint = if editor_tab == EditorTab::Body {
                            match body_mode {
                                BodyMode::FormData => Some(
                                    "每行一个字段：key=value；文件用 key=@/path/to/file。",
                                ),
                                BodyMode::FormUrlEncoded => {
                                    Some("每行一个字段：key=value（发送时拼接为 a=1&b=2）。")
                                }
                                BodyMode::Binary => {
                                    Some("填写文件路径，或点击右上角“选择文件”。")
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(section_micro_label(label, dark))
                            .when_some(hint, |column, text| {
                                column.child(auth_hint(text))
                            })
                            .child(
                                div()
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(ui::border_light())
                                    .bg(theme::semantic().bg_subtle_2)
                                    .overflow_hidden()
                                    .child(input),
                            )
                            .into_any_element()
                    }
                }),
        )
}

/// The Auth tab body — renders a type-specific form (Bearer / Basic / API
/// Key). The type itself is chosen via the buttons in the editor's `mode_row`.
fn auth_form_panel(
    view: Entity<ApiDebuggerView>,
    auth_type: AuthType,
    form: AuthFormInputs,
    dark: bool,
) -> impl IntoElement {
    let body = match auth_type {
        AuthType::None => div()
            .py(px(6.0))
            .text_size(px(11.0))
            .text_color(ui::text_tertiary())
            .child("该请求不附带认证信息。")
            .into_any_element(),
        AuthType::BearerToken => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("Token", form.bearer.clone(), dark))
            .child(auth_hint("发送时自动添加请求头 Authorization: Bearer <token>。"))
            .into_any_element(),
        AuthType::BasicAuth => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("用户名", form.basic_user.clone(), dark))
            .child(labeled_field("密码", form.basic_pass.clone(), dark))
            .child(auth_hint("发送时自动以 Base64 编码为 Authorization: Basic 头。"))
            .into_any_element(),
        AuthType::ApiKey => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("Key", form.apikey_name.clone(), dark))
            .child(labeled_field("Value", form.apikey_value.clone(), dark))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_micro_label("位置", dark))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(auth_location_button(view.clone(), "Header", false, !form.in_query))
                            .child(auth_location_button(view.clone(), "Query", true, form.in_query)),
                    ),
            )
            .into_any_element(),
    };

    div().flex().flex_col().gap(px(12.0)).child(body)
}

fn auth_hint(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(10.0))
        .text_color(ui::text_tertiary())
        .child(text)
}

fn auth_location_button(
    view: Entity<ApiDebuggerView>,
    label: &'static str,
    query: bool,
    active: bool,
) -> impl IntoElement {
    div()
        .id(("api-auth-location", query as usize))
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .text_size(px(10.0))
        .text_color(if active {
            api_accent()
        } else {
            ui::text_tertiary()
        })
        .bg(if active {
            theme::rgba_with_alpha(theme::semantic().primary, 0.12)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_1()
        .border_color(if active {
            api_accent().into()
        } else {
            ui::border_light()
        })
        .hover(|style| style.cursor_pointer().text_color(api_accent()))
        .child(label)
        .on_click(move |_, _, cx| {
            view.update(cx, |view, cx| {
                view.auth_apikey_in_query = query;
                view.sync_models(cx);
                view.persist_current_tab_state(cx);
            });
        })
}

fn response_panel(
    view: Entity<ApiDebuggerView>,
    response_tab: ResponseTab,
    response: ApiResponse,
    response_text: String,
    history_entries: Vec<HttpHistory>,
    code_lang: CodeLanguage,
    notice: String,
    dark: bool,
) -> impl IntoElement {
    let tabs_view = view.clone();

    let content: AnyElement = match response_tab {
        ResponseTab::History => {
            response_history_view(view.clone(), history_entries, dark).into_any_element()
        }
        ResponseTab::Code => {
            response_code_view(view.clone(), code_lang, response_text, dark).into_any_element()
        }
        ResponseTab::Body => response_body_view(
            view.clone(),
            response.content_type.clone(),
            response_text,
            dark,
        )
        .into_any_element(),
        _ => response_text_view(response_text).into_any_element(),
    };

    div()
        .flex_1()
        .min_w(px(320.0))
        .bg(theme::semantic().bg_surface)
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_subtle)
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(status_badge(&response, dark))
                .child(div().flex_1())
                .child(response_metric(
                    format!("{} ms", response.duration_ms),
                    dark,
                ))
                .child(response_metric(format!("{} B", response.size_bytes), dark)),
        )
        .child(
            div()
                .px(px(10.0))
                .py(px(4.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_subtle)
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
                .children(ResponseTab::all().into_iter().enumerate().map(
                    move |(index, tab)| {
                        let active = tab == response_tab;
                        let tab_view = tabs_view.clone();
                        div()
                            .id(("api-response-tab", index))
                            .px(px(9.0))
                            .py(px(5.0))
                            .rounded(px(4.0))
                            .bg(if active {
                                theme::rgba_with_alpha(api_accent(), 0.08)
                            } else {
                                transparent_surface()
                            })
                            .text_size(px(11.0))
                            .text_color(if active {
                                api_accent()
                            } else {
                                ui::text_tertiary()
                            })
                            .hover(move |style| style.bg(glass::hover_bg(dark)).cursor_pointer())
                            .child(tab.label())
                            .on_click(move |_, window, cx| {
                                tab_view.update(cx, |view, _cx| view.set_response_tab(tab));
                                window.refresh();
                            })
                    },
                )),
        )
        .child(content)
        .child(
            div()
                .px(px(12.0))
                .py(px(6.0))
                .border_t_1()
                .border_color(ui::border_light())
                .text_size(px(11.0))
                .text_color(ui::text_secondary())
                .child(notice),
        )
}

/// Scrollable monospace text body — the default response content area.
fn response_text_view(text: String) -> impl IntoElement {
    div()
        .id("api-response-scroll")
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .scrollbar_width(px(4.0))
        .p(px(10.0))
        .bg(theme::semantic().bg_subtle_2)
        .child(
            div()
                .font_family("SF Mono")
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(theme::semantic().text_body)
                .child(text),
        )
}

/// Response Body tab: action bar (复制 / 格式化 / 保存) + content-type hint + body.
fn response_body_view(
    view: Entity<ApiDebuggerView>,
    content_type: String,
    text: String,
    dark: bool,
) -> impl IntoElement {
    let binary = is_binary_content_type(&content_type);
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(response_action_button(
                    view.clone(),
                    "复制",
                    ResponseBodyAction::Copy,
                    dark,
                ))
                .child(response_action_button(
                    view.clone(),
                    "格式化",
                    ResponseBodyAction::Format,
                    dark,
                ))
                .child(response_action_button(
                    view.clone(),
                    "保存",
                    ResponseBodyAction::Save,
                    dark,
                ))
                .child(div().flex_1())
                .when(!content_type.is_empty(), |row| {
                    row.child(
                        div()
                            .text_size(px(10.0))
                            .font_family("SF Mono")
                            .text_color(ui::text_tertiary())
                            .child(content_type.clone()),
                    )
                }),
        )
        .when(binary, |panel| {
            panel.child(
                div()
                    .px(px(10.0))
                    .py(px(6.0))
                    .bg(theme::rgba_with_alpha(theme::semantic().danger, 0.08))
                    .text_size(px(11.0))
                    .text_color(theme::semantic().danger)
                    .child("⚠ 二进制/图片响应，文本预览可能乱码，建议点击「保存」后查看"),
            )
        })
        .child(response_text_view(text))
}

/// Response Code tab: language selector + generated snippet.
fn response_code_view(
    view: Entity<ApiDebuggerView>,
    code_lang: CodeLanguage,
    code_text: String,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
                .children(CodeLanguage::all().into_iter().enumerate().map(
                    move |(index, lang)| {
                        let active = lang == code_lang;
                        let lang_view = view.clone();
                        div()
                            .id(("api-code-lang", index))
                            .px(px(8.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .text_size(px(11.0))
                            .bg(if active {
                                theme::rgba_with_alpha(api_accent(), 0.08)
                            } else {
                                transparent_surface()
                            })
                            .text_color(if active {
                                api_accent()
                            } else {
                                ui::text_tertiary()
                            })
                            .hover(move |style| style.bg(glass::hover_bg(dark)).cursor_pointer())
                            .child(lang.label())
                            .on_click(move |_, window, cx| {
                                lang_view
                                    .update(cx, |view, _cx| view.set_response_code_lang(lang));
                                window.refresh();
                            })
                    },
                )),
        )
        .child(response_text_view(code_text))
}

/// Response History tab: count + 清空 header and a clickable list of past calls.
fn response_history_view(
    view: Entity<ApiDebuggerView>,
    entries: Vec<HttpHistory>,
    dark: bool,
) -> impl IntoElement {
    let clear_view = view.clone();
    let count = entries.len();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(ui::border_light())
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child(format!("共 {count} 条历史记录")),
                )
                .when(count > 0, |row| {
                    row.child(
                        div()
                            .id("api-history-clear")
                            .px(px(8.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .text_size(px(11.0))
                            .text_color(theme::semantic().danger)
                            .hover(move |style| style.bg(glass::hover_bg(dark)).cursor_pointer())
                            .child("清空")
                            .on_click(move |_, window, cx| {
                                clear_view
                                    .update(cx, |view, _cx| view.clear_current_history());
                                window.refresh();
                            }),
                    )
                }),
        )
        .child(
            div()
                .id("api-history-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(4.0))
                .when(count == 0, |list| {
                    list.child(
                        div()
                            .p(px(12.0))
                            .text_size(px(11.0))
                            .text_color(ui::text_tertiary())
                            .child("暂无历史记录，发送请求后会自动追加"),
                    )
                })
                .children(
                    entries
                        .into_iter()
                        .enumerate()
                        .map(move |(index, entry)| history_row(view.clone(), index, entry, dark)),
                ),
        )
}

fn history_row(
    view: Entity<ApiDebuggerView>,
    index: usize,
    entry: HttpHistory,
    dark: bool,
) -> impl IntoElement {
    let status_color = if entry.status == 0 {
        theme::semantic().text_secondary
    } else if (200..300).contains(&entry.status) {
        theme::semantic().success
    } else {
        theme::semantic().danger
    };
    div()
        .id(("api-history-row", index))
        .px(px(10.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(ui::border_light())
        .flex()
        .flex_col()
        .gap(px(2.0))
        .hover(move |style| style.bg(glass::hover_bg(dark)).cursor_pointer())
        .on_click(move |_, window, cx| {
            view.update(cx, |view, _cx| view.view_history_entry(index));
            window.refresh();
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme::semantic().text_primary)
                        .child(entry.method.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(status_color)
                        .child(entry.status.to_string()),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_tertiary())
                        .child(entry.created_at.clone()),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::semantic().text_body)
                .child(entry.url.clone()),
        )
}

#[derive(Clone, Copy)]
enum ResponseBodyAction {
    Copy,
    Format,
    Save,
}

fn response_action_button(
    view: Entity<ApiDebuggerView>,
    label: &'static str,
    action: ResponseBodyAction,
    dark: bool,
) -> impl IntoElement {
    let id_index = match action {
        ResponseBodyAction::Copy => 0usize,
        ResponseBodyAction::Format => 1,
        ResponseBodyAction::Save => 2,
    };
    div()
        .id(("api-response-action", id_index))
        .px(px(8.0))
        .py(px(3.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .text_size(px(11.0))
        .text_color(ui::text_secondary())
        .hover(move |style| style.bg(glass::hover_bg(dark)).cursor_pointer())
        .child(label)
        .on_click(move |_, window, cx| {
            view.update(cx, |view, cx| match action {
                ResponseBodyAction::Copy => view.copy_response_body(cx),
                ResponseBodyAction::Format => view.format_response_body(),
                ResponseBodyAction::Save => view.save_response_body(),
            });
            window.refresh();
        })
}

/// Map a response `Content-Type` to a sensible file extension for saving.
fn content_type_extension(content_type: &str) -> &'static str {
    let ct = content_type.to_ascii_lowercase();
    let ct = ct.split(';').next().unwrap_or("").trim();
    match ct {
        "application/json" => "json",
        "text/html" => "html",
        "application/xml" | "text/xml" => "xml",
        "text/css" => "css",
        "text/csv" => "csv",
        "application/javascript" | "text/javascript" => "js",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        _ if ct.starts_with("text/") => "txt",
        _ => "txt",
    }
}

/// Whether a `Content-Type` denotes binary content unsuitable for text preview.
fn is_binary_content_type(content_type: &str) -> bool {
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
}


fn env_popup(
    view: Entity<ApiDebuggerView>,
    environments: Vec<ApiEnvironment>,
    selected_environment: usize,
    dark: bool,
) -> impl IntoElement {
    div()
        .w(px(340.0))
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
        .rounded(px(8.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .children(
            environments
                .into_iter()
                .enumerate()
                .map(|(index, environment)| {
                    let active = index == selected_environment;
                    div()
                        .id(("api-env-popup-row", index))
                        .min_h(px(64.0))
                        .px(px(14.0))
                        .py(px(10.0))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .bg(if active {
                            theme::rgba_with_alpha(api_accent(), 0.06)
                        } else {
                            transparent_surface()
                        })
                        .hover(move |style| {
                            style
                                .bg(theme::rgba_with_alpha(api_accent(), 0.04))
                                .cursor_pointer()
                        })
                        .child(circle_badge(&environment.badge, environment.color, 36.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme::semantic().text_primary)
                                        .truncate()
                                        .child(environment.name.clone()),
                                )
                                .child(
                                    div()
                                        .font_family("SF Mono")
                                        .text_size(px(11.0))
                                        .text_color(ui::text_tertiary())
                                        .truncate()
                                        .child(environment.base_url.clone()),
                                ),
                        )
                        .when(active, |row| {
                            row.child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(api_accent())
                                    .child("✓"),
                            )
                        })
                        .on_click({
                            let view = view.clone();
                            move |_, window, cx| {
                                view.update(cx, |view, cx| {
                                    view.select_environment(index, cx);
                                });
                                window.refresh();
                            }
                        })
                }),
        )
        .child(
            div()
                .id("api-env-manage")
                .px(px(14.0))
                .py(px(8.0))
                .border_t_1()
                .border_color(ui::border_light())
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(api_accent())
                .hover(move |style| {
                    style
                        .bg(theme::rgba_with_alpha(api_accent(), 0.05))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .justify_center()
                .child("⚙ 管理环境")
                .on_click({
                    let view = view.clone();
                    move |_, window, cx| {
                        view.update(cx, |view, _cx| {
                            view.show_env_popup = false;
                            view.show_env_manager = true;
                        });
                        window.refresh();
                    }
                }),
        )
}

fn env_manager_dialog(
    view: Entity<ApiDebuggerView>,
    selected_environment: usize,
    env_detail_tab: EnvDetailTab,
    environments: Vec<ApiEnvironment>,
    env_name_input: Entity<TextInput>,
    env_base_url_input: Entity<TextInput>,
    env_variables_input: Entity<TextInput>,
    env_headers_input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let current_environment = environments
        .get(selected_environment)
        .cloned()
        .expect("environment should exist");
    let detail_input = if env_detail_tab == EnvDetailTab::Variables {
        env_variables_input
    } else {
        env_headers_input
    };
    let env_tabs_view = view.clone();

    div()
        .w(px(1040.0))
        .max_w(px(1180.0))
        .rounded(px(16.0))
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(52.0))
                .px(px(20.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.34 } else { 0.52 },
                ))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(div().text_size(px(18.0)).child("🌐"))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme::semantic().text_primary)
                                .child("环境管理"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(ui::text_secondary())
                                .child(format!("{} 个环境", environments.len())),
                        ),
                )
                .child(
                    Button::new("api-env-close")
                        .ghost()
                        .label("关闭")
                        .with_size(Size::Small)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, _cx| view.show_env_manager = false);
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .min_h(px(500.0))
                .child(
                    div()
                        .w(px(260.0))
                        .border_r_1()
                        .border_color(ui::border_light())
                        .bg(theme::rgba_with_alpha(
                            theme::semantic().bg_surface,
                            if dark { 0.18 } else { 0.34 },
                        ))
                        .p(px(12.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(section_micro_label("环境", dark))
                                .child(
                                    Button::new("api-env-add")
                                        .ghost()
                                        .icon(IconName::Plus)
                                        .label("新建")
                                        .with_size(Size::XSmall)
                                        .on_click({
                                            let view = view.clone();
                                            move |_, _, cx| {
                                                view.update(cx, |view, _cx| {
                                                    view.create_new_environment();
                                                });
                                            }
                                        }),
                                )
                        )
                        .child(
                            div()
                                .id("api-env-list-scroll")
                                .flex_1()
                                .min_h(px(0.0))
                                .overflow_y_scroll()
                                .scrollbar_width(px(4.0))
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .children(environments.into_iter().enumerate().map(
                                    |(index, environment)| {
                                        let active = index == selected_environment;
                                        div()
                                            .id(("api-env-list-row", index))
                                            .min_h(px(70.0))
                                            .px(px(10.0))
                                            .py(px(8.0))
                                            .rounded(px(8.0))
                                            .border_1()
                                            .border_color(if active {
                                                theme::rgba_with_alpha(api_accent(), 0.18).into()
                                            } else {
                                                transparent_surface()
                                            })
                                            .bg(if active {
                                                theme::rgba_with_alpha(api_accent(), 0.08)
                                            } else {
                                                transparent_surface()
                                            })
                                            .hover(move |style| {
                                                style
                                                    .bg(theme::rgba_with_alpha(api_accent(), 0.05))
                                                    .cursor_pointer()
                                            })
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(circle_badge(
                                                &environment.badge,
                                                environment.color,
                                                34.0,
                                            ))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(4.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                                            .text_color(
                                                                theme::semantic().text_primary,
                                                            )
                                                            .truncate()
                                                            .child(environment.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .font_family("SF Mono")
                                                            .text_size(px(10.0))
                                                            .text_color(ui::text_tertiary())
                                                            .truncate()
                                                            .child(environment.base_url.clone()),
                                                    ),
                                            )
                                            .on_click({
                                                let view = view.clone();
                                                move |_, window, cx| {
                                                    view.update(cx, |view, _cx| {
                                                        view.select_environment(index, _cx);
                                                    });
                                                    view.update(cx, |view, _cx| {
                                                        view.show_env_manager = true;
                                                    });
                                                    window.refresh();
                                                }
                                            })
                                    },
                                )),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .p(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .text_size(px(16.0))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(theme::semantic().text_primary)
                                                .child(current_environment.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .h(px(28.0))
                                                .px(px(10.0))
                                                .rounded(px(999.0))
                                                .border_1()
                                                .border_color(ui::border_light())
                                                .bg(theme::rgba_with_alpha(
                                                    theme::semantic().bg_surface,
                                                    if dark { 0.32 } else { 0.52 },
                                                ))
                                                .font_family("SF Mono")
                                                .text_size(px(12.0))
                                                .text_color(ui::text_secondary())
                                                .flex()
                                                .items_center()
                                                .child(current_environment.base_url.clone()),
                ),
        )
        )
        .child(
                                    div()
                                        .flex()
                                        .gap(px(6.0))
                                        .child(
                                            Button::new("api-env-dup")
                                                .ghost()
                                                .label("复制")
                                                .with_size(Size::Small)
                                                .on_click({
                                                    let view = view.clone();
                                                    move |_, _, cx| {
                                                        view.update(cx, |view, cx| {
                                                            view.duplicate_current_environment(cx);
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new("api-env-del")
                                                .ghost()
                                                .label("删除")
                                                .with_size(Size::Small)
                                                .on_click({
                                                    let view = view.clone();
                                                    move |_, _, cx| {
                                                        view.update(cx, |view, cx| {
                                                            view.delete_current_environment(cx);
                                                        });
                                                    }
                                                }),
                                ),
                        )
                        .child(labeled_field("名称", env_name_input, dark))
                        .child(labeled_field("Base URL", env_base_url_input, dark))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .children(
                                    [EnvDetailTab::Variables, EnvDetailTab::Headers]
                                        .into_iter()
                                        .enumerate()
                                        .map(move |(index, tab)| {
                                            let active = tab == env_detail_tab;
                                            let tab_view = env_tabs_view.clone();
                                            div()
                                                .id(("api-env-detail-tab", index))
                                                .px(px(12.0))
                                                .py(px(6.0))
                                                .rounded(px(6.0))
                                                .border_1()
                                                .border_color(if active {
                                                    theme::rgba_with_alpha(api_accent(), 0.18)
                                                } else {
                                                    ui::border_light()
                                                })
                                                .bg(if active {
                                                    theme::rgba_with_alpha(api_accent(), 0.08)
                                                } else {
                                                    theme::rgba_with_alpha(
                                                        theme::semantic().bg_surface,
                                                        if dark { 0.24 } else { 0.48 },
                                                    )
                                                })
                                                .text_size(px(11.0))
                                                .font_weight(if active {
                                                    gpui::FontWeight::SEMIBOLD
                                                } else {
                                                    gpui::FontWeight::NORMAL
                                                })
                                                .text_color(if active {
                                                    api_accent()
                                                } else {
                                                    ui::text_secondary()
                                                })
                                                .hover(move |style| {
                                                    style
                                                        .bg(theme::rgba_with_alpha(
                                                            api_accent(),
                                                            0.06,
                                                        ))
                                                        .cursor_pointer()
                                                })
                                                .child(tab.label())
                                                .on_click({
                                                    move |_, window, cx| {
                                                        tab_view.update(cx, |view, _cx| {
                                                            view.env_detail_tab = tab;
                                                        });
                                                        window.refresh();
                                                    }
                                                })
                                        }),
                                )
                                .child(
                                    Button::new("api-env-add-row")
                                        .ghost()
                                        .icon(IconName::Plus)
                                        .label("新增")
                                        .with_size(Size::XSmall)
                                        .on_click({
                                            let view = view.clone();
                                            move |_, _, cx| {
                                                view.update(cx, |view, cx| {
                                                    let current =
                                                        if view.env_detail_tab == EnvDetailTab::Variables {
                                                            view.env_variables_input.read(cx).text()
                                                        } else {
                                                            view.env_headers_input.read(cx).text()
                                                        };
                                                    let appended = if current.trim().is_empty() {
                                                        String::from("KEY=VALUE")
                                                    } else {
                                                        format!("{current}\nKEY=VALUE")
                                                    };
                                                    if view.env_detail_tab == EnvDetailTab::Variables {
                                                        view.env_variables_input.update(
                                                            cx,
                                                            |input, input_cx| {
                                                                input.set_text(appended.clone(), input_cx)
                                                            },
                                                        );
                                                    } else {
                                                        view.env_headers_input.update(
                                                            cx,
                                                            |input, input_cx| {
                                                                input.set_text(appended.clone(), input_cx)
                                                            },
                                                        );
                                                    }
                                                });
                                            }
                                        }),
                                )
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .border_1()
                                .border_color(ui::border_light())
                                .bg(theme::rgba_with_alpha(
                                    theme::semantic().bg_surface,
                                    if dark { 0.30 } else { 0.54 },
                                ))
                                .overflow_hidden()
                                .child(detail_input),
                        ),
                ),
        )
        .child(
            div()
                .h(px(48.0))
                .px(px(20.0))
                .border_t_1()
                .border_color(ui::border_light())
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.28 } else { 0.46 },
                ))
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    Button::new("api-env-save")
                        .primary()
                        .label("保存更改")
                        .with_size(Size::Small)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    view.save_environment_changes(cx);
                                });
                            }
                        }),
                )
                .child(
                    Button::new("api-env-reset")
                        .ghost()
                        .label("重置")
                        .with_size(Size::Small)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    view.reset_environment_changes(cx);
                                });
                            }
                        }),
                )
                .child(div().flex_1())
                .child(
                    Button::new("api-env-export")
                        .ghost()
                        .label("导出")
                        .with_size(Size::Small)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, _cx| {
                                    view.notice = String::from("环境导出功能开发中");
                                });
                            }
                        }),
                )
                .child(
                    Button::new("api-env-import")
                        .ghost()
                        .label("导入")
                        .with_size(Size::Small)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, _cx| {
                                    view.notice = String::from("环境导入功能开发中");
                                });
                            }
                        }),
                )
                .child(
                    context_menu_item(
                        "api-env-delete",
                        "删除此环境",
                        "",
                        {
                            let view = view.clone();
                            move |_, cx| {
                                view.update(cx, |view, cx| view.delete_current_environment(cx));
                            }
                        },
                    )
                ),
        )
}

fn labeled_field(label: &'static str, input: Entity<TextInput>, dark: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(section_micro_label(label, dark))
        .child(
            div()
                .h(px(32.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.34 } else { 0.58 },
                ))
                .overflow_hidden()
                .child(input),
        )
}

fn curl_import_dialog(
    view: Entity<ApiDebuggerView>,
    curl_import_input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let import_view = view.clone();
    let cancel_view = view.clone();
    div()
        .w(px(560.0))
        .rounded(px(16.0))
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(44.0))
                .px(px(18.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.34 } else { 0.52 },
                ))
                .flex()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::semantic().text_body)
                        .child("导入 cURL 命令"),
                ),
        )
        .child(
            div()
                .p(px(16.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::semantic().text_secondary)
                        .child("粘贴 cURL 命令以导入请求"),
                )
                .child(
                    div()
                        .id("curl-import-textarea-wrapper")
                        .child(curl_import_input),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("curl-import-cancel-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(theme::rgba_with_alpha(
                                    theme::semantic().bg_surface,
                                    if dark { 0.5 } else { 0.7 },
                                ))
                                .text_xs()
                                .text_color(theme::semantic().text_secondary)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    cancel_view.update(cx, |view, _cx| {
                                        view.show_curl_import = false;
                                    });
                                })
                                .child("取消"),
                        )
                        .child(
                            div()
                                .id("curl-import-ok-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(theme::semantic().primary)
                                .text_xs()
                                .text_color(theme::semantic().text_primary)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    import_view.update(cx, |view, cx| {
                                        view.import_curl(cx);
                                    });
                                })
                                .child("导入"),
                        ),
                ),
        )
}

fn rename_dialog(
    view: Entity<ApiDebuggerView>,
    rename_input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    let confirm_view = view.clone();
    let cancel_view = view.clone();
    div()
        .w(px(420.0))
        .rounded(px(16.0))
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(44.0))
                .px(px(18.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.34 } else { 0.52 },
                ))
                .flex()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::semantic().text_body)
                        .child("重命名"),
                ),
        )
        .child(
            div()
                .p(px(16.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::semantic().text_secondary)
                        .child("输入新的名称"),
                )
                .child(div().id("api-rename-input-wrapper").child(rename_input))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("api-rename-cancel-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(theme::rgba_with_alpha(
                                    theme::semantic().bg_surface,
                                    if dark { 0.5 } else { 0.7 },
                                ))
                                .text_xs()
                                .text_color(theme::semantic().text_secondary)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    cancel_view.update(cx, |view, _cx| {
                                        view.show_rename = false;
                                    });
                                })
                                .child("取消"),
                        )
                        .child(
                            div()
                                .id("api-rename-ok-btn")
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(theme::semantic().primary)
                                .text_xs()
                                .text_color(theme::semantic().text_primary)
                                .cursor_pointer()
                                .on_click(move |_event, _window, cx| {
                                    confirm_view.update(cx, |view, cx| {
                                        view.confirm_rename(cx);
                                    });
                                })
                                .child("确定"),
                        ),
                ),
        )
}

fn overlay_shell(
    dark: bool,
    backdrop_id: &'static str,
    on_close: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .id(backdrop_id)
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(hsla(0.0, 0.0, 0.0, if dark { 0.46 } else { 0.24 }))
                .on_click(move |event, _window, cx| on_close(event, cx)),
        )
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .flex()
                .items_center()
                .justify_center()
                .child(content),
        )
}

fn context_menu_overlay(
    view: Entity<ApiDebuggerView>,
    title: String,
    position: Option<(f32, f32)>,
    node_id: String,
    dark: bool,
) -> impl IntoElement {
    let (x, y) = position.unwrap_or((248.0, 96.0));
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .id("api-collection-menu-backdrop")
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.001))
                .on_click({
                    let view = view.clone();
                    move |_, window, cx| {
                        view.update(cx, |view, _cx| view.close_collection_menu());
                        window.refresh();
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top(px(y))
                .left(px(x))
                .w(px(230.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_surface)
                .rounded(px(8.0))
                .shadow_md()
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(12.0))
                        .py(px(9.0))
                        .border_b_1()
                        .border_color(ui::border_light())
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(api_accent())
                        .child(format!("📂 {title}")),
                )
                .child(context_menu_item(
                    "api-collection-menu-new-request",
                    "新建端点",
                    "⌘N",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.create_new_endpoint());
                        }
                    },
                ))
                .child(context_menu_item(
                    "api-collection-menu-new-group",
                    "新建分组",
                    "⇧⌘N",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.create_new_folder());
                        }
                    },
                ))
                .child(context_menu_item(
                    "api-collection-menu-new-case",
                    "新建用例",
                    "",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.create_new_case());
                        }
                    },
                ))
                .child(context_menu_item(
                    "api-collection-menu-export",
                    "导出为 OpenAPI",
                    "",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.export_openapi());
                        }
                    },
                ))
                .child(context_menu_item(
                    "api-collection-menu-import-curl",
                    "导入 cURL",
                    "",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, cx| {
                                view.show_curl_import = true;
                            });
                        }
                    },
                ))
                .child(context_menu_item(
                    "api-collection-menu-import-openapi",
                    "导入 OpenAPI",
                    "",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.import_openapi_file());
                        }
                    },
                ))
                .child(context_menu_item(
                    "api-collection-menu-import-postman",
                    "导入 Postman",
                    "",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.import_postman_file());
                        }
                    },
                ))
                .child(menu_separator())
                .child(context_menu_item(
                    "api-collection-menu-rename",
                    "重命名",
                    "",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, cx| view.open_rename(cx));
                        }
                    },
                ))
                .child(context_menu_item(
                    "api-collection-menu-duplicate",
                    "复制路径",
                    "",
                    {
                        let view = view.clone();
                        let node_id = node_id.clone();
                        move |_, cx| {
                            let url = if !node_id.is_empty() {
                                let api_view = view.read(cx);
                                if let Ok(Some(node)) = api_view.service.get_collection_node(&node_id) {
                                    Some(node.url.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            if let Some(url) = url {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(url.clone()));
                                view.update(cx, |view, _cx| {
                                    view.notice = format!("已复制: {}", url);
                                    view.close_collection_menu();
                                });
                            } else {
                                view.update(cx, |view, _cx| {
                                    view.notice = String::from("节点未找到");
                                    view.close_collection_menu();
                                });
                            }
                        }
                    },
                ))
                .child(menu_separator())
                .child(context_menu_item(
                    "api-collection-menu-delete",
                    "删除",
                    "",
                    {
                        let view = view.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.delete_selected_collection_item());
                        }
                    },
                )),
        )
}

fn context_menu_item(
    id: &'static str,
    label: &'static str,
    shortcut: &'static str,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(12.0))
        .py(px(8.0))
        .text_size(px(11.0))
        .text_color(theme::semantic().text_body)
        .hover(move |style| {
            style
                .bg(theme::rgba_with_alpha(api_accent(), 0.06))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(div().flex_1().child(label))
        .when(!shortcut.is_empty(), |row| {
            row.child(
                div()
                    .text_size(px(10.0))
                    .text_color(ui::text_tertiary())
                    .child(shortcut),
            )
        })
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn menu_separator() -> impl IntoElement {
    div().h(px(1.0)).bg(ui::border_light())
}

fn transparent_surface() -> gpui::Hsla {
    theme::rgba_with_alpha(theme::semantic().bg_surface, 0.0)
}

fn api_accent() -> gpui::Rgba {
    theme::semantic().primary
}

fn content_split(stacked: bool) -> gpui::Div {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .when(stacked, |layout| layout.flex_col())
        .when(!stacked, |layout| layout.flex_row())
}

fn group_count(count: usize) -> impl IntoElement {
    div()
        .min_w(px(22.0))
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(api_accent(), 0.08))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(api_accent())
        .child(count.to_string())
}

fn scenario_count_badge(count: usize, _dark: bool) -> impl IntoElement {
    div()
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic().text_secondary,
            0.08,
        ))
        .text_size(px(10.0))
        .text_color(theme::semantic().text_secondary)
        .child(count.to_string())
}

fn status_dot(status: ScenarioStatus, dark: bool) -> impl IntoElement {
    div()
        .size(px(7.0))
        .rounded(px(999.0))
        .bg(status_color(status, dark))
}

fn scenario_status_pill(status: ScenarioStatus, dark: bool) -> impl IntoElement {
    div()
        .h(px(18.0))
        .px(px(6.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(status_color(status, dark), 0.1))
        .text_size(px(10.0))
        .text_color(status_color(status, dark))
        .child(status.label())
}

fn scenario_status_label(status: ScenarioStatus, dark: bool) -> impl IntoElement {
    div()
        .h(px(22.0))
        .px(px(8.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(status_color(status, dark), 0.1))
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(status_color(status, dark))
        .flex()
        .items_center()
        .child(status.label())
}

fn section_micro_label(label: impl Into<String>, _dark: bool) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(ui::text_tertiary())
        .child(label.into())
}

fn response_metric(text: String, _dark: bool) -> impl IntoElement {
    div()
        .h(px(22.0))
        .px(px(8.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic().text_secondary,
            0.08,
        ))
        .flex()
        .items_center()
        .text_size(px(10.0))
        .font_family("SF Mono")
        .text_color(theme::semantic().text_secondary)
        .child(text)
}

fn method_badge(method: HttpMethod, _dark: bool) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .font_family("SF Mono")
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(rgb(method.color()))
        .child(method.label())
}

fn circle_badge(label: &str, color: u32, size: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .rounded(px(size / 2.0))
        .bg(rgb(color))
        .text_color(theme::white())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px((size * 0.36).max(10.0)))
        .font_weight(gpui::FontWeight::BOLD)
        .child(label.to_string())
}

fn status_badge(response: &ApiResponse, _dark: bool) -> impl IntoElement {
    let color = if response.status_code == 0 {
        theme::semantic().text_secondary
    } else if response.status_code >= 200 && response.status_code < 300 {
        theme::semantic().success
    } else {
        theme::semantic().danger
    };
    div()
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(color, 0.10))
        .text_size(px(12.0))
        .font_family("SF Mono")
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(color)
        .child(response.status_line.clone())
}

fn status_color(status: ScenarioStatus, _dark: bool) -> gpui::Rgba {
    match status {
        ScenarioStatus::Passed => theme::semantic().success,
        ScenarioStatus::Pending => theme::semantic().warning,
        ScenarioStatus::Failed => theme::semantic().danger,
    }
}

fn kv_input(cx: &mut App, value: &str, placeholder: &str) -> Entity<TextInput> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        let mut input = TextInput::new(cx, placeholder.clone(), value.clone());
        input.set_chrome(false, cx);
        input.set_monospace(true, cx);
        input.set_style(
            TextInputStyle {
                height: 28.0,
                font_size: 11.0,
                padding: 6.0,
            },
            cx,
        );
        input
    })
}

fn single_input(cx: &mut App, value: &str, placeholder: &str) -> Entity<TextInput> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        let mut input = TextInput::new(cx, placeholder.clone(), value.clone());
        input.set_chrome(false, cx);
        input.set_style(
            TextInputStyle {
                height: 32.0,
                font_size: 11.0,
                padding: 8.0,
            },
            cx,
        );
        input.set_monospace(true, cx);
        input
    })
}

fn multiline_input(cx: &mut App, value: &str, placeholder: &str) -> Entity<TextInput> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        let mut input = TextInput::new(cx, placeholder.clone(), value.clone());
        input.set_chrome(false, cx);
        input.set_multiline(true, cx);
        input.set_monospace(true, cx);
        input.set_style(
            TextInputStyle {
                height: 220.0,
                font_size: 11.0,
                padding: 10.0,
            },
            cx,
        );
        input
    })
}

fn request_at(groups: &[ApiGroup], index: usize) -> Option<&ApiRequest> {
    let mut offset = 0usize;
    for group in groups {
        if index < offset + group.requests.len() {
            return group.requests.get(index - offset);
        }
        offset += group.requests.len();
    }
    None
}

fn find_request_index_by_method_url(groups: &[ApiGroup], method: &str, url: &str) -> Option<usize> {
    let mut offset = 0usize;
    let method_upper = method.to_uppercase();
    for group in groups {
        for (i, req) in group.requests.iter().enumerate() {
            if req.method.label() == method_upper && req.path == url {
                return Some(offset + i);
            }
        }
        offset += group.requests.len();
    }
    None
}

fn request_at_mut(groups: &mut [ApiGroup], index: usize) -> Option<&mut ApiRequest> {
    let mut offset = 0usize;
    for group in groups {
        if index < offset + group.requests.len() {
            return group.requests.get_mut(index - offset);
        }
        offset += group.requests.len();
    }
    None
}

fn parse_rows(text: &str) -> Vec<KeyValueRow> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            // A leading `#` marks a disabled row (round-trips with `format_rows`).
            let (enabled, content) = match line.strip_prefix('#') {
                Some(rest) => (false, rest.trim()),
                None => (true, line),
            };
            let (key, value) = content
                .split_once('=')
                .map(|(key, value)| (key.trim(), value.trim()))
                .unwrap_or((content, ""));
            KeyValueRow {
                enabled,
                key: key.to_string(),
                value: value.to_string(),
                description: String::new(),
            }
        })
        .collect()
}

fn format_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .map(|row| {
            let body = format!("{}={}", row.key, row.value);
            if row.enabled {
                body
            } else {
                format!("# {body}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn detect_body_mode(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "none".to_string();
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return "json".to_string();
    }
    "text".to_string()
}

fn kv_editor_table(
    view: Entity<ApiDebuggerView>,
    tab: EditorTab,
    rows: Vec<KvRow>,
    _dark: bool,
) -> impl IntoElement {
    let add_view = view.clone();

    div()
        .flex()
        .flex_col()
        .bg(theme::semantic().bg_surface)
        .child(
            div()
                .id("kv-table-header")
                .h(px(26.0))
                .px(px(8.0))
                .border_b_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_subtle)
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(10.0))
                .text_color(ui::text_tertiary())
                .child(div().w(px(28.0)).child("启用"))
                .child(div().flex_1().child("键"))
                .child(div().flex_1().child("值"))
                .child(div().w(px(24.0))),
        )
        .children(rows.into_iter().enumerate().map(move |(i, row)| {
            let enabled = row.enabled;
            let key_input = row.key.clone();
            let value_input = row.value.clone();
            let toggle_view = view.clone();
            let delete_view = view.clone();

            div()
                .id(("kv-row", i))
                .min_h(px(34.0))
                .px(px(8.0))
                .py(px(3.0))
                .border_b_1()
                .border_color(ui::border_light())
                .hover(|s| s.bg(theme::rgba_with_alpha(theme::semantic().bg_subtle, 0.5)))
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .w(px(28.0))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id(("kv-checkbox", i))
                                .w(px(14.0))
                                .h(px(14.0))
                                .rounded(px(3.0))
                                .border_1()
                                .border_color(if enabled {
                                    theme::semantic().primary.into()
                                } else {
                                    ui::border_light()
                                })
                                .bg(if enabled {
                                    theme::rgba_with_alpha(theme::semantic().primary, 0.14)
                                } else {
                                    theme::semantic().bg_surface.into()
                                })
                                .text_size(px(9.0))
                                .text_color(if enabled {
                                    theme::semantic().primary.into()
                                } else {
                                    hsla(0.0, 0.0, 0.0, 0.0)
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(if enabled { "✓" } else { "" })
                                .on_click(move |_, _, cx| {
                                    toggle_view.update(cx, |view, cx| {
                                        if let Some(editor) = view.kv_editor_mut(tab) {
                                            editor.toggle(i);
                                        }
                                        view.sync_models(cx);
                                        view.persist_current_tab_state(cx);
                                    });
                                }),
                        ),
                )
                .child(kv_cell(key_input, enabled))
                .child(kv_cell(value_input, enabled))
                .child(
                    Button::new(("kv-del", i))
                        .ghost()
                        .icon(IconName::Close)
                        .with_size(Size::XSmall)
                        .on_click(move |_, _, cx| {
                            delete_view.update(cx, |view, cx| {
                                if let Some(editor) = view.kv_editor_mut(tab) {
                                    editor.remove_row(i);
                                }
                                view.sync_models(cx);
                                view.persist_current_tab_state(cx);
                            });
                        }),
                )
        }))
        .child(
            div().px(px(8.0)).py(px(6.0)).child(
                Button::new("kv-add-row")
                    .ghost()
                    .icon(IconName::Plus)
                    .label("新增")
                    .with_size(Size::XSmall)
                    .on_click(move |_, _, cx| {
                        add_view.update(cx, |view, cx| {
                            if let Some(editor) = view.kv_editor_mut(tab) {
                                editor.add_row(cx);
                            }
                            view.persist_current_tab_state(cx);
                        });
                    }),
            ),
        )
}

/// A single editable cell wrapping a key/value `TextInput`. Dimmed when the
/// row is disabled.
fn kv_cell(input: Entity<TextInput>, enabled: bool) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(0.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(ui::border_light())
        .bg(theme::semantic().bg_subtle_2)
        .overflow_hidden()
        .when(!enabled, |cell| cell.opacity(0.5))
        .child(input)
}

fn sample_response() -> ApiResponse {
    ApiResponse {
        status_line: String::from("等待请求"),
        status_code: 0,
        duration_ms: 0,
        size_bytes: 0,
        body: String::from("{\n  \"_notice\": \"发送请求后，响应内容将显示在此处\"\n}"),
        headers: String::new(),
        cookies: String::new(),
        content_type: String::new(),
        request_dump: String::new(),
        curl: String::new(),
        logs: vec![String::from("尚未发送请求")],
        assertion_results: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rows_decodes_disabled_prefix() {
        let rows = parse_rows("Accept=application/json\n# X-Debug=1\nempty");
        assert_eq!(rows.len(), 3);

        assert!(rows[0].enabled);
        assert_eq!(rows[0].key, "Accept");
        assert_eq!(rows[0].value, "application/json");

        assert!(!rows[1].enabled);
        assert_eq!(rows[1].key, "X-Debug");
        assert_eq!(rows[1].value, "1");

        assert!(rows[2].enabled);
        assert_eq!(rows[2].key, "empty");
        assert_eq!(rows[2].value, "");
    }

    #[test]
    fn format_rows_encodes_disabled_prefix() {
        let rows = vec![
            KeyValueRow::new("a", "1"),
            KeyValueRow {
                enabled: false,
                key: "b".into(),
                value: "2".into(),
                description: String::new(),
            },
        ];
        assert_eq!(format_rows(&rows), "a=1\n# b=2");
    }

    #[test]
    fn rows_text_roundtrip_preserves_enabled() {
        let original = vec![
            KeyValueRow::new("page", "1"),
            KeyValueRow {
                enabled: false,
                key: "limit".into(),
                value: "10".into(),
                description: String::new(),
            },
            KeyValueRow::new("sort", "desc"),
        ];
        let restored = parse_rows(&format_rows(&original));
        assert_eq!(restored.len(), original.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert_eq!(a.enabled, b.enabled);
            assert_eq!(a.key, b.key);
            assert_eq!(a.value, b.value);
        }
    }

    #[test]
    fn value_with_hash_is_not_treated_as_disabled() {
        // A `#` only disables when it is the first character of the line.
        let rows = parse_rows("color=#fff");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].enabled);
        assert_eq!(rows[0].key, "color");
        assert_eq!(rows[0].value, "#fff");
    }

    #[test]
    fn content_type_extension_maps_known_types() {
        assert_eq!(
            content_type_extension("application/json; charset=utf-8"),
            "json"
        );
        assert_eq!(content_type_extension("image/png"), "png");
        assert_eq!(content_type_extension("text/html"), "html");
        assert_eq!(content_type_extension("application/octet-stream"), "txt");
        assert_eq!(content_type_extension(""), "txt");
    }

    #[test]
    fn binary_content_types_are_flagged() {
        assert!(is_binary_content_type("image/jpeg"));
        assert!(is_binary_content_type("application/pdf"));
        assert!(is_binary_content_type("video/mp4"));
        assert!(!is_binary_content_type("application/json"));
        assert!(!is_binary_content_type("text/plain"));
    }
}
