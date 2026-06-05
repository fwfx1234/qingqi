use std::sync::Arc;

use gpui::{
    App, AppContext, BoxShadow, Context, Entity, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, hsla, point,
    prelude::FluentBuilder, px, rgb,
};
use uuid::Uuid;

use crate::service::{
    self, ApiEnvironment, ApiGroup, ApiRequest, ApiResponse, ApiScenario, ApiService, AuthType,
    BodyMode, EditorTab, EnvDetailTab, HttpMethod, KeyValueRow, ResponseTab, ScenarioStatus,
    TabDraft,
};
use qingqi_ui::{
    text_input::{TextInput, TextInputStyle},
    theme, ui,
    ui::glass,
};

const STACK_BREAKPOINT_PX: f32 = 980.0;

#[derive(Clone, Debug, PartialEq, Eq)]
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
    env_detail_tab: EnvDetailTab,
    body_mode: BodyMode,
    auth_type: AuthType,
    show_env_popup: bool,
    show_env_manager: bool,
    show_collection_menu: bool,
    collection_menu_title: String,
    collection_menu_position: Option<(f32, f32)>,
    collection_menu_node_id: String,
    show_method_popup: bool,
    show_curl_import: bool,
    curl_import_input: Entity<TextInput>,
    path_input: Entity<TextInput>,
    params_input: Entity<TextInput>,
    path_rows_input: Entity<TextInput>,
    body_input: Entity<TextInput>,
    headers_input: Entity<TextInput>,
    cookies_input: Entity<TextInput>,
    auth_input: Entity<TextInput>,
    pre_ops_input: Entity<TextInput>,
    post_ops_input: Entity<TextInput>,
    env_name_input: Entity<TextInput>,
    env_base_url_input: Entity<TextInput>,
    env_variables_input: Entity<TextInput>,
    env_headers_input: Entity<TextInput>,
    response: ApiResponse,
    notice: String,
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
            env_detail_tab: EnvDetailTab::Variables,
            body_mode: BodyMode::from_db(&detect_body_mode(&init_body)),
            auth_type: AuthType::from_db(&detect_auth_type_str(&init_auth)),
            show_env_popup: false,
            show_env_manager: false,
            show_collection_menu: false,
            collection_menu_title: String::from("集合"),
            collection_menu_position: None,
            collection_menu_node_id: String::new(),
            show_method_popup: false,
            show_curl_import: false,
            curl_import_input: multiline_input(cx, "", "粘贴 cURL 命令..."),
            path_input: single_input(cx, &init_path, "/api/v1/user/info"),
            params_input: multiline_input(cx, &init_params, "KEY=VALUE"),
            path_rows_input: multiline_input(cx, &init_path_rows, "segment"),
            body_input: multiline_input(cx, &init_body, "{ }"),
            headers_input: multiline_input(cx, &init_headers, "KEY=VALUE"),
            cookies_input: multiline_input(cx, &init_cookies, "KEY=VALUE"),
            auth_input: multiline_input(cx, &init_auth, "KEY=VALUE"),
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
        }
    }

    fn sync_service_updates(&mut self) {
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
        let params = parse_rows(&self.params_input.read(cx).text());
        let path_rows = parse_rows(&self.path_rows_input.read(cx).text());
        let body = self.body_input.read(cx).text();
        let headers = parse_rows(&self.headers_input.read(cx).text());
        let cookies = parse_rows(&self.cookies_input.read(cx).text());
        let auth = parse_rows(&self.auth_input.read(cx).text());
        let pre_ops = self.pre_ops_input.read(cx).text();
        let post_ops = self.post_ops_input.read(cx).text();

        {
            let request = self.selected_request_mut();
            request.path = path;
            request.params = params;
            request.path_rows = path_rows;
            request.body = body;
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
            eprintln!("持久化端点失败: {error}");
        }
    }

    fn persist_workspace(&mut self) {
        self.service.save_workspace_async(self.environments.clone());
        self.persist_endpoint_if_needed();
    }

    fn reload_request_inputs(&mut self, cx: &mut App) {
        let request = self.selected_request().clone();
        self.path_input.update(cx, |input, input_cx| {
            input.set_text(request.path.clone(), input_cx)
        });
        self.params_input.update(cx, |input, input_cx| {
            input.set_text(format_rows(&request.params), input_cx)
        });
        self.path_rows_input.update(cx, |input, input_cx| {
            input.set_text(format_rows(&request.path_rows), input_cx)
        });
        self.body_input.update(cx, |input, input_cx| {
            input.set_text(request.body.clone(), input_cx)
        });
        self.headers_input.update(cx, |input, input_cx| {
            input.set_text(format_rows(&request.headers), input_cx)
        });
        self.cookies_input.update(cx, |input, input_cx| {
            input.set_text(format_rows(&request.cookies), input_cx)
        });
        self.auth_input.update(cx, |input, input_cx| {
            input.set_text(format_rows(&request.auth), input_cx)
        });
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
            params_text: self.params_input.read(cx).text(),
            path_params_text: self.path_rows_input.read(cx).text(),
            body_text: self.body_input.read(cx).text(),
            headers_text: self.headers_input.read(cx).text(),
            cookies_text: self.cookies_input.read(cx).text(),
            auth_text: self.auth_input.read(cx).text(),
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

    fn restore_inputs_from_tab(&self, tab: &crate::model::HttpTab, cx: &mut App) {
        let draft = service::restore_tab_draft(tab);
        self.path_input
            .update(cx, |input, input_cx| input.set_text(draft.url, input_cx));
        self.params_input.update(cx, |input, input_cx| {
            input.set_text(draft.params_text, input_cx)
        });
        self.path_rows_input.update(cx, |input, input_cx| {
            input.set_text(draft.path_params_text, input_cx)
        });
        self.body_input.update(cx, |input, input_cx| {
            input.set_text(draft.body_text, input_cx)
        });
        self.headers_input.update(cx, |input, input_cx| {
            input.set_text(draft.headers_text, input_cx)
        });
        self.cookies_input.update(cx, |input, input_cx| {
            input.set_text(draft.cookies_text, input_cx)
        });
        self.pre_ops_input.update(cx, |input, input_cx| {
            input.set_text(draft.pre_ops_text, input_cx)
        });
        self.post_ops_input.update(cx, |input, input_cx| {
            input.set_text(draft.post_ops_text, input_cx)
        });
        self.auth_input.update(cx, |input, input_cx| {
            input.set_text(draft.auth_text, input_cx)
        });
    }

    fn close_open_tab(&mut self, tab_index: usize, cx: &mut App) {
        if tab_index >= self.open_tabs.len() {
            return;
        }
        let tab_id = self.open_tabs[tab_index].tab_id().to_string();
        self.service.delete_persisted_tab_async(tab_id);
        self.open_tabs.remove(tab_index);
        if self.open_tabs.is_empty() {
            self.active_tab = OpenTab::new_request(0);
            self.selected_request = 0;
            self.selected_scenario = None;
            self.open_tabs.push(self.active_tab.clone());
            self.reload_request_inputs(cx);
        } else if tab_index <= self.active_index() {
            let new_active = self.active_index().min(self.open_tabs.len() - 1);
            let tab = self.open_tabs[new_active].clone();
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
            // Try restoring persisted draft for the new active tab
            let tab_id = self.active_tab.tab_id().to_string();
            if let Some(persisted) = self.service.load_persisted_tab_by_id(&tab_id) {
                self.restore_inputs_from_tab(&persisted, cx);
                let tab_idx = persisted.active_request_tab;
                if let Some(et) = service::index_to_editor_tab(tab_idx) {
                    self.editor_tab = et;
                }
            } else {
                self.reload_request_inputs(cx);
            }
        }
    }

    fn active_index(&self) -> usize {
        self.open_tabs
            .iter()
            .position(|t| t == &self.active_tab)
            .unwrap_or(0)
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
        self.show_method_popup = false;
        self.persist_workspace();
        self.persist_current_tab_state(cx);
    }

    fn toggle_method_popup(&mut self) {
        self.show_method_popup = !self.show_method_popup;
    }

    fn close_method_popup(&mut self) {
        self.show_method_popup = false;
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
            ResponseTab::History => String::from("历史记录（点击请求自动追加）"),
            ResponseTab::Code => {
                let request = self.selected_request();
                let environment = self.selected_environment();
                let all_headers: Vec<KeyValueRow> = request.headers.iter()
                    .chain(environment.headers.iter())
                    .cloned()
                    .collect();
                let code_req = crate::code_gen::CodeGenRequest {
                    method: request.method.label().to_string(),
                    url: format!("{}/{}", environment.base_url.trim_end_matches('/'), request.path.trim_start_matches('/')),
                    headers: all_headers,
                    body: request.body.clone(),
                    body_mode: crate::model::BodyMode::Json,
                    form_data: vec![],
                };
                format!("// cURL\n{}\n\n// Python\n{}",
                    crate::code_gen::generate(crate::code_gen::CodeLanguage::Curl, &code_req),
                    crate::code_gen::generate(crate::code_gen::CodeLanguage::Python, &code_req),
                )
            }
        }
    }

    fn editor_input(&self) -> Entity<TextInput> {
        match self.editor_tab {
            EditorTab::Params => self.params_input.clone(),
            EditorTab::Path => self.path_rows_input.clone(),
            EditorTab::Body => self.body_input.clone(),
            EditorTab::Headers => self.headers_input.clone(),
            EditorTab::Cookies => self.cookies_input.clone(),
            EditorTab::Auth => self.auth_input.clone(),
            EditorTab::PreOps => self.pre_ops_input.clone(),
            EditorTab::PostOps => self.post_ops_input.clone(),
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
        match self.service.export_collection_as_openapi() {
            Ok(_json) => {
                self.notice = String::from("OpenAPI 导出成功");
            }
            Err(e) => self.notice = format!("导出失败: {e}"),
        }
        self.close_collection_menu();
    }

    fn cycle_body_mode(&mut self, cx: &App) {
        self.body_mode = match self.body_mode {
            BodyMode::None => BodyMode::Json,
            BodyMode::Json => BodyMode::Text,
            BodyMode::Text => BodyMode::Xml,
            BodyMode::Xml => BodyMode::FormUrlEncoded,
            BodyMode::FormUrlEncoded => BodyMode::FormData,
            BodyMode::FormData => BodyMode::Binary,
            BodyMode::Binary => BodyMode::None,
        };
        self.persist_current_tab_state(cx);
    }

    fn cycle_auth_type(&mut self, cx: &App) {
        self.auth_type = match self.auth_type {
            AuthType::None => AuthType::BearerToken,
            AuthType::BearerToken => AuthType::BasicAuth,
            AuthType::BasicAuth => AuthType::ApiKey,
            AuthType::ApiKey => AuthType::None,
        };
        self.sync_models(cx);
        self.persist_current_tab_state(cx);
    }
}

impl Render for ApiDebuggerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_service_updates();

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
        let show_method_popup = self.show_method_popup;
        let show_curl_import = self.show_curl_import;
        let collection_menu_title = self.collection_menu_title.clone();
        let collection_menu_position = self.collection_menu_position;
        let collection_menu_node_id = self.collection_menu_node_id.clone();
        let path_input = self.path_input.clone();
        let editor_input = self.editor_input();
        let env_name_input = self.env_name_input.clone();
        let curl_import_input = self.curl_import_input.clone();
        let env_base_url_input = self.env_base_url_input.clone();
        let env_variables_input = self.env_variables_input.clone();
        let env_headers_input = self.env_headers_input.clone();
        let response = self.response.clone();
        let response_text = self.response_text();
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
                        view.show_method_popup = false;
                        view.show_curl_import = false;
                    });
                }
            })
            .child(
                div()
                    .size_full()
                    .relative()
                    .pt(px(chrome.metrics().content_top_padding + 6.0))
                    .pl(px(6.0))
                    .pr(px(6.0))
                    .pb(px(6.0))
                    .flex()
                    .gap(px(if stacked { 8.0 } else { 10.0 }))
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
                            .bg(glass::bg(dark))
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
                                        editor_input,
                                        body_mode,
                                        auth_type,
                                        dark,
                                    ))
                                    .child(response_panel(
                                        entity.clone(),
                                        response_tab,
                                        response,
                                        response_text,
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
            .child(if show_method_popup {
                overlay_shell(
                    dark,
                    "api-method-popup-backdrop",
                    {
                        let view = entity.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.close_method_popup());
                        }
                    },
                    method_popup(entity.clone(), current_request.method, dark),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(ui::popup_window_chrome_with_titlebar_slot(chrome, None))
    }
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
        .w(px(250.0))
        .min_h(px(220.0))
        .flex_none()
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(38.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(glass::divider(dark))
                .bg(glass::bar(dark))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(if dark {
                            theme::semantic().text_primary
                        } else {
                            theme::semantic().text_body
                        })
                        .child("集合"),
                )
                .child(icon_button("api-tree-add", "+", dark, {
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |view, _cx| {
                            view.open_collection_menu("集合", Some((418.0, 86.0)), String::new());
                        });
                    }
                })),
        )
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
        .px(px(6.0))
        .py(px(2.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .id(("api-group-row", request_start))
                .px(px(4.0))
                .py(px(3.0))
                .rounded(px(5.0))
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
                        .text_size(px(9.0))
                        .text_color(ui::text_tertiary())
                        .child("▾"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(ui::text_secondary())
                        .truncate()
                        .child(group.name.clone()),
                )
                .child(group_count(group.requests.len()))
                .child(
                    div()
                        .px(px(3.0))
                        .text_size(px(10.0))
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
                .min_h(px(30.0))
                .px(px(8.0))
                .py(px(3.0))
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
                .gap(px(4.0))
                .child(method_badge(request.method, dark))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(11.0))
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
                    .min_h(px(24.0))
                    .px(px(7.0))
                    .py(px(2.0))
                    .pl(px(24.0))
                    .rounded(px(5.0))
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
                            .text_size(px(10.0))
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
        .h(px(32.0))
        .px(px(8.0))
        .border_b_1()
        .border_color(glass::divider(dark))
        .flex()
        .items_center()
        .gap(px(4.0))
        .bg(glass::bar(dark))
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
                        .h(px(24.0))
                        .px(px(8.0))
                        .rounded(px(4.0))
                        .border_b_1()
                        .border_color(if active {
                            api_accent()
                        } else {
                            theme::rgba_with_alpha(theme::semantic().bg_surface, 0.0).into()
                        })
                        .bg(if active {
                            theme::rgba_with_alpha(api_accent(), 0.08)
                        } else {
                            transparent_surface()
                        })
                        .text_size(px(9.0))
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
                        .gap(px(3.0))
                        .child(
                            div().max_w(px(160.0)).truncate().child(
                                titles
                                    .get(index)
                                    .cloned()
                                    .unwrap_or_else(|| String::from("请求")),
                            ),
                        )
                        .child(
                            div()
                                .id(("api-tab-close", index))
                                .text_size(px(9.0))
                                .text_color(theme::semantic().text_secondary)
                                .hover(move |style| {
                                    style.text_color(theme::semantic().danger).cursor_pointer()
                                })
                                .child("✕")
                                .on_click({
                                    let view = close_view.clone();
                                    move |event, window, cx| {
                                        cx.stop_propagation();
                                        view.update(cx, |view, cx| view.close_open_tab(index, cx));
                                        window.refresh();
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
                .h(px(26.0))
                .px(px(10.0))
                .rounded(px(999.0))
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.48 } else { 0.74 },
                ))
                .text_size(px(10.0))
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
                        .size(px(7.0))
                        .rounded(px(999.0))
                        .bg(rgb(environment.color)),
                )
                .child(environment.name.clone())
                .child(
                    div()
                        .text_size(px(8.0))
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
    request: ApiRequest,
    environment: ApiEnvironment,
    path_input: Entity<TextInput>,
    in_flight: bool,
    dark: bool,
) -> impl IntoElement {
    div()
        .px(px(8.0))
        .py(px(5.0))
        .border_b_1()
        .border_color(glass::divider(dark))
        .bg(glass::bar(dark))
        .flex()
        .items_center()
        .gap(px(5.0))
        .child(
            div()
                .id("api-method-selector")
                .h(px(28.0))
                .px(px(8.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme::rgba_with_alpha(rgb(request.method.color()), 0.16))
                .bg(theme::rgba_with_alpha(rgb(request.method.color()), 0.08))
                .hover(move |style| {
                    style
                        .bg(theme::rgba_with_alpha(rgb(request.method.color()), 0.12))
                        .cursor_pointer()
                })
                .flex()
                .items_center()
                .gap(px(2.0))
                .child(
                    div()
                        .font_family("SF Mono")
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_size(px(10.0))
                        .text_color(rgb(request.method.color()))
                        .child(request.method.label()),
                )
                .child(
                    div()
                        .text_size(px(6.0))
                        .text_color(theme::semantic().text_secondary)
                        .child("▾"),
                )
                .on_click({
                    let view = view.clone();
                    move |_, window, cx| {
                        view.update(cx, |view, _cx| view.toggle_method_popup());
                        window.refresh();
                    }
                }),
        )
        .child(
            div()
                .flex_1()
                .h(px(28.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(glass::border(dark))
                .bg(glass::inset(dark))
                .px(px(8.0))
                .flex()
                .items_center()
                .gap(px(3.0))
                .child(
                    div()
                        .font_family("SF Mono")
                        .text_size(px(9.0))
                        .text_color(ui::text_tertiary())
                        .child(environment.base_url),
                )
                .child(div().flex_1().min_w(px(0.0)).child(path_input)),
        )
.child(if in_flight {
            let cancel_view = view.clone();
            div()
                .id("api-cancel-btn")
                .h(px(32.0))
                .px(px(14.0))
                .rounded(px(8.0))
                .bg(hsla(0.0, 0.7, 0.5, 0.85))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                .cursor_pointer()
                .on_click(move |_, _window, cx| {
                    cancel_view.update(cx, |view, cx| view.cancel_request(cx));
                })
                .child("取消")
                .into_any_element()
        } else {
            primary_button(
                "api-send",
                "📤 发送",
                dark,
                {
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |view, cx| view.send_request(cx));
                    }
                },
            )
            .into_any_element()
        })
}

fn scenario_banner(scenario: ApiScenario, request: ApiRequest, dark: bool) -> impl IntoElement {
    div()
        .px(px(10.0))
        .py(px(7.0))
        .border_b_1()
        .border_color(ui::border_light())
        .bg(theme::rgba_with_alpha(api_accent(), 0.06))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(api_accent())
                .child(format!("{}", scenario.name)),
        )
        .child(scenario_status_label(scenario.status, dark))
        .child(div().flex_1())
        .child(
            div()
                .text_size(px(9.0))
                .text_color(theme::semantic().text_secondary)
                .child(format!("基于 {} {}", request.method.label(), request.title)),
        )
}

fn editor_panel(
    view: Entity<ApiDebuggerView>,
    editor_tab: EditorTab,
    editor_input: Entity<TextInput>,
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
            let current = body_mode.label();
            let modes = BodyMode::all();
            let mut row = div()
                .px(px(8.0))
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
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .text_size(px(8.0))
                        .text_color(if is_active {
                            api_accent()
                        } else {
                            ui::text_tertiary()
                        })
                        .bg(if is_active {
                            theme::rgba_with_alpha(theme::semantic().primary, 0.18)
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
            row.into_any_element()
        }
        EditorTab::Auth => {
            let au_view = subtoolbar_view.clone();
            let current = auth_type.label();
            let types = AuthType::all();
            let mut row = div()
                .px(px(8.0))
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
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .text_size(px(8.0))
                        .text_color(if is_active {
                            api_accent()
                        } else {
                            ui::text_tertiary()
                        })
                        .bg(if is_active {
                            theme::rgba_with_alpha(theme::semantic().primary, 0.18)
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
    div()
        .flex_1()
        .min_w(px(320.0))
        .border_r_1()
        .border_color(glass::divider(dark))
        .bg(glass::panel(dark))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(8.0))
                .border_b_1()
                .border_color(glass::divider(dark))
                .bg(glass::bar(dark))
                .flex()
                .items_center()
                .gap(px(2.0))
                .children(
                    editor_tabs()
                        .into_iter()
                        .enumerate()
                        .map(move |(index, tab)| {
                            let active = tab == editor_tab;
                            let tab_view = tabs_view.clone();
                            div()
                                .id(("api-editor-tab", index))
                                .px(px(7.0))
                                .py(px(4.0))
                                .border_b_1()
                                .border_color(if active {
                                    api_accent()
                                } else {
                                    theme::rgba_with_alpha(theme::semantic().bg_surface, 0.0).into()
                                })
                                .text_size(px(8.0))
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
                        }),
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
                .p(px(8.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(section_micro_label(label, dark))
                        .child(
                            div()
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(glass::border(dark))
                                .bg(glass::inset(dark))
                                .overflow_hidden()
                                .child(editor_input),
                        ),
                ),
        )
}

fn response_panel(
    view: Entity<ApiDebuggerView>,
    response_tab: ResponseTab,
    response: ApiResponse,
    response_text: String,
    notice: String,
    dark: bool,
) -> impl IntoElement {
    let tabs_view = view.clone();
    div()
        .flex_1()
        .min_w(px(320.0))
        .bg(glass::panel(dark))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(7.0))
                .border_b_1()
                .border_color(glass::divider(dark))
                .bg(glass::bar(dark))
                .flex()
                .items_center()
                .gap(px(6.0))
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
                .px(px(8.0))
                .py(px(4.0))
                .border_b_1()
                .border_color(glass::divider(dark))
                .bg(glass::bar(dark))
                .flex()
                .items_center()
                .gap(px(3.0))
                .children(
                    response_tabs()
                        .into_iter()
                        .enumerate()
                        .map(move |(index, tab)| {
                            let active = tab == response_tab;
                            let tab_view = tabs_view.clone();
                            div()
                                .id(("api-response-tab", index))
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded(px(4.0))
                                .bg(if active {
                                    theme::rgba_with_alpha(api_accent(), 0.08)
                                } else {
                                    theme::rgba_with_alpha(theme::semantic().bg_surface, 0.0)
                                })
                                .text_size(px(9.0))
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
                                .child(tab.label())
                                .on_click({
                                    move |_, window, cx| {
                                        tab_view.update(cx, |view, _cx| {
                                            view.response_tab = tab;
                                        });
                                        window.refresh();
                                    }
                                })
                        }),
                ),
        )
        .child(
            div()
                .id("api-response-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(4.0))
                .p(px(8.0))
                .bg(glass::inset(dark))
                .child(
                    div()
                        .font_family("SF Mono")
                        .text_size(px(10.0))
                        .line_height(px(16.0))
                        .text_color(theme::semantic().text_body)
                        .child(response_text),
                ),
        )
        .child(
            div()
                .px(px(10.0))
                .py(px(5.0))
                .border_t_1()
                .border_color(ui::border_light())
                .text_size(px(10.0))
                .text_color(ui::text_secondary())
                .child(notice),
        )
}

fn env_popup(
    view: Entity<ApiDebuggerView>,
    environments: Vec<ApiEnvironment>,
    selected_environment: usize,
    dark: bool,
) -> impl IntoElement {
    div()
        .w(px(318.0))
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
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
                        .py(px(8.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
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
                        .child(circle_badge(&environment.badge, environment.color, 34.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(px(3.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme::semantic().text_primary)
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
                        .when(active, |row| {
                            row.child(
                                div()
                                    .text_size(px(12.0))
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
                .px(px(18.0))
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
                        .gap(px(8.0))
                        .child(div().text_size(px(18.0)).child("🌐"))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme::semantic().text_primary)
                                .child("环境管理"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(ui::text_secondary())
                                .child(format!("{} 个环境", environments.len())),
                        ),
                )
                .child(soft_button("api-env-close", "关闭", dark, {
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |view, _cx| view.show_env_manager = false);
                    }
                })),
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
                                .child(soft_button("api-env-add", "+ 新建", dark, {
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |view, _cx| {
                                            view.create_new_environment();
                                        });
                                    }
                                })),
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
                                                    view.update(cx, |view, cx| {
                                                        view.select_environment(index, cx);
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
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .text_size(px(14.0))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(theme::semantic().text_primary)
                                                .child(current_environment.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .h(px(24.0))
                                                .px(px(8.0))
                                                .rounded(px(999.0))
                                                .border_1()
                                                .border_color(ui::border_light())
                                                .bg(theme::rgba_with_alpha(
                                                    theme::semantic().bg_surface,
                                                    if dark { 0.32 } else { 0.52 },
                                                ))
                                                .font_family("SF Mono")
                                                .text_size(px(11.0))
                                                .text_color(ui::text_secondary())
                                                .flex()
                                                .items_center()
                                                .child(current_environment.base_url.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.0))
                                        .child(soft_button("api-env-dup", "复制", dark, {
                                            let view = view.clone();
                                            move |_, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.duplicate_current_environment(cx);
                                                });
                                            }
                                        }))
                                        .child(soft_button("api-env-del", "删除", dark, {
                                            let view = view.clone();
                                            move |_, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.delete_current_environment(cx);
                                                });
                                            }
                                        })),
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
                                                .px(px(10.0))
                                                .py(px(5.0))
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
                                .child(soft_button("api-env-add-row", "+ 新增变量", dark, {
                                    let view = view.clone();
                                    move |_, cx| {
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
                                })),
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
                .px(px(18.0))
                .border_t_1()
                .border_color(ui::border_light())
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.28 } else { 0.46 },
                ))
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(action_link("api-env-save", "💾 保存更改", true, {
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |view, cx| {
                            view.save_environment_changes(cx);
                        });
                    }
                }))
                .child(action_link("api-env-reset", "↩ 重置", false, {
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |view, cx| {
                            view.reset_environment_changes(cx);
                        });
                    }
                }))
                .child(div().flex_1())
                .child(action_link("api-env-export", "📤 导出", false, {
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |view, _cx| {
                            view.notice = String::from("环境导出入口已预留");
                        });
                    }
                }))
                .child(action_link("api-env-import", "📥 导入", false, {
                    let view = view.clone();
                    move |_, cx| {
                        view.update(cx, |view, _cx| {
                            view.notice = String::from("环境导入入口已预留");
                        });
                    }
                }))
                .child(
                    {
                        let view = view.clone();
                        div()
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme::semantic().danger)
                            .hover(move |style| style.cursor_pointer())
                            .on_click(move |_, _window, cx| {
                                view.update(cx, |view, cx| view.delete_current_environment(cx));
                            })
                            .child("删除此环境")
                    }
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

fn method_popup(
    view: Entity<ApiDebuggerView>,
    current_method: HttpMethod,
    dark: bool,
) -> impl IntoElement {
    div()
        .w(px(120.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(glass::border(dark))
        .bg(glass::bg(dark))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(glass::divider(dark))
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_primary)
                .child("选择方法"),
        )
        .children(
            HttpMethod::all()
                .into_iter()
                .enumerate()
                .map(move |(index, method)| {
                    let label = method.label();
                    let is_active = method == current_method;
                    let method_view = view.clone();
                    div()
                        .id(("api-method-option", index))
                        .px(px(10.0))
                        .py(px(7.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .bg(if is_active {
                            theme::rgba_with_alpha(theme::semantic().primary, 0.12)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(move |style| {
                            style
                                .bg(glass::hover_bg(dark))
                                .cursor_pointer()
                        })
                        .child(
                            div()
                                .w(px(8.0))
                                .h(px(8.0))
                                .rounded_full()
                                .bg(rgb(method.color())),
                        )
                        .child(
                            div()
                                .font_family("SF Mono")
                                .text_size(px(11.0))
                                .font_weight(if is_active {
                                    gpui::FontWeight::SEMIBOLD
                                } else {
                                    gpui::FontWeight::NORMAL
                                })
                                .text_color(if is_active {
                                    api_accent()
                                } else {
                                    ui::text_primary()
                                })
                                .child(label),
                        )
                        .on_click(move |_, _window, cx| {
                            method_view.update(cx, |view, cx| {
                                view.set_method(method, cx);
                            });
                        })
                }),
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
.border_color(glass::border(dark))
        .bg(glass::bg(dark))
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
                .child(menu_separator())
                .child(context_menu_item(
                    "api-collection-menu-duplicate",
                    "复制路径",
                    "",
                    {
                        let view = view.clone();
let node_id = node_id.clone();
                    move |_, cx| {
                        view.update(cx, |view, _cx| {
                            if !node_id.is_empty() {
                                if let Ok(Some(node)) = view.service.get_collection_node(&node_id) {
                                        view.notice = format!("已复制: {}", node.url);
                                    } else {
                                        view.notice = String::from("节点未找到");
                                    }
                                }
                                view.close_collection_menu();
                            });
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

fn icon_button(
    id: &'static str,
    label: &'static str,
    dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(px(24.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::rgba_with_alpha(
            theme::semantic().bg_surface,
            if dark { 0.52 } else { 0.78 },
        ))
        .hover(move |style| {
            style
                .bg(theme::rgba_with_alpha(api_accent(), 0.08))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(api_accent())
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn group_count(count: usize) -> impl IntoElement {
    div()
        .min_w(px(20.0))
        .h(px(18.0))
        .px(px(5.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(api_accent(), 0.08))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(api_accent())
        .child(count.to_string())
}

fn scenario_count_badge(count: usize, _dark: bool) -> impl IntoElement {
    div()
        .h(px(18.0))
        .px(px(5.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic().text_secondary,
            0.08,
        ))
        .text_size(px(8.0))
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
        .h(px(16.0))
        .px(px(5.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(status_color(status, dark), 0.1))
        .text_size(px(8.0))
        .text_color(status_color(status, dark))
        .child(status.label())
}

fn scenario_status_label(status: ScenarioStatus, dark: bool) -> impl IntoElement {
    div()
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(status_color(status, dark), 0.1))
        .text_size(px(9.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(status_color(status, dark))
        .flex()
        .items_center()
        .child(status.label())
}

fn section_micro_label(label: impl Into<String>, _dark: bool) -> impl IntoElement {
    div()
        .text_size(px(9.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(ui::text_tertiary())
        .child(label.into())
}

fn response_metric(text: String, _dark: bool) -> impl IntoElement {
    div()
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(999.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic().text_secondary,
            0.08,
        ))
        .flex()
        .items_center()
        .text_size(px(9.0))
        .font_family("SF Mono")
        .text_color(theme::semantic().text_secondary)
        .child(text)
}

fn soft_button(
    id: &'static str,
    label: &'static str,
    dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(26.0))
        .px(px(10.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(theme::semantic().border_default)
        .bg(theme::rgba_with_alpha(
            theme::semantic().bg_surface,
            if dark { 0.42 } else { 0.72 },
        ))
        .hover(move |style| {
            style
                .bg(theme::rgba_with_alpha(api_accent(), 0.08))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(theme::semantic().text_secondary)
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn primary_button(
    id: &'static str,
    label: &'static str,
    dark: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(32.0))
        .px(px(16.0))
        .rounded(px(8.0))
        .bg(theme::rgba_with_alpha(
            api_accent(),
            if dark { 0.22 } else { 0.12 },
        ))
        .border_1()
        .border_color(theme::rgba_with_alpha(api_accent(), 0.18))
        .text_color(api_accent())
        .hover(move |style| {
            style
                .bg(theme::rgba_with_alpha(
                    api_accent(),
                    if dark { 0.30 } else { 0.18 },
                ))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn method_badge(method: HttpMethod, _dark: bool) -> impl IntoElement {
    div()
        .px(px(4.0))
        .py(px(2.0))
        .rounded(px(5.0))
        .bg(theme::rgba_with_alpha(rgb(method.color()), 0.1))
        .text_size(px(9.0))
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
        .px(px(8.0))
        .py(px(3.0))
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(color, 0.10))
        .text_size(px(11.0))
        .font_family("SF Mono")
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(color)
        .child(response.status_line.clone())
}

fn action_link(
    id: &'static str,
    label: &'static str,
    primary: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .text_size(px(11.0))
        .font_weight(if primary {
            gpui::FontWeight::SEMIBOLD
        } else {
            gpui::FontWeight::NORMAL
        })
        .text_color(if primary {
            api_accent()
        } else {
            ui::text_secondary()
        })
        .hover(move |style| {
            style
                .text_color(if primary {
                    api_accent()
                } else {
                    theme::semantic().text_primary
                })
                .cursor_pointer()
        })
        .child(label)
        .on_click(move |event, _window, cx| on_click(event, cx))
}

fn status_color(status: ScenarioStatus, _dark: bool) -> gpui::Rgba {
    match status {
        ScenarioStatus::Passed => theme::semantic().success,
        ScenarioStatus::Pending => theme::semantic().warning,
        ScenarioStatus::Failed => theme::semantic().danger,
    }
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
            let (key, value) = line
                .split_once('=')
                .map(|(key, value)| (key.trim(), value.trim()))
                .unwrap_or((line, ""));
            KeyValueRow {
                enabled: true,
                key: key.to_string(),
                value: value.to_string(),
            }
        })
        .collect()
}

fn format_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .map(|row| format!("{}={}", row.key, row.value))
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

fn detect_auth_type_str(auth_text: &str) -> String {
    let text = auth_text.trim();
    if text.is_empty() {
        return "none".to_string();
    }
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, _)) = line.split_once('=') {
            let key = key.trim().to_lowercase();
            if key == "authorization" {
                let val = line.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
                if val.starts_with("Bearer ") || val.starts_with("bearer ") {
                    return "bearer".to_string();
                }
                if val.starts_with("Basic ") || val.starts_with("basic ") {
                    return "basic".to_string();
                }
            }
            if key == "x-api-key" {
                return "apikey".to_string();
            }
        }
    }
    "none".to_string()
}

fn editor_tabs() -> [EditorTab; 8] {
    [
        EditorTab::Params,
        EditorTab::Path,
        EditorTab::Body,
        EditorTab::Headers,
        EditorTab::Cookies,
        EditorTab::Auth,
        EditorTab::PreOps,
        EditorTab::PostOps,
    ]
}

fn response_tabs() -> [ResponseTab; 7] {
    [
        ResponseTab::Body,
        ResponseTab::Headers,
        ResponseTab::Request,
        ResponseTab::Curl,
        ResponseTab::Logs,
        ResponseTab::History,
        ResponseTab::Code,
    ]
}

fn sample_response() -> ApiResponse {
    ApiResponse {
        status_line: String::from("等待请求"),
        status_code: 0,
        duration_ms: 0,
        size_bytes: 0,
        body: String::from("{\n  \"_notice\": \"发送请求后，响应内容将显示在此处\"\n}"),
        headers: String::new(),
        request_dump: String::new(),
        curl: String::new(),
        logs: vec![String::from("尚未发送请求")],
        assertion_results: Vec::new(),
    }
}
