use super::ApiDebuggerView;
use super::components::collection_tree::build_tree_items;
use super::types::{OpenTab, request_at, request_at_mut};
use crate::service::{ApiRequest, BodyMode, HttpMethod};
use gpui::{App, Window};

impl ApiDebuggerView {
    pub(crate) fn placeholder_request() -> ApiRequest {
        ApiRequest {
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
        }
    }

    pub(crate) fn placeholder_environment() -> crate::service::ApiEnvironment {
        crate::service::ApiEnvironment {
            name: String::from("默认环境"),
            badge: String::from("默"),
            color: 0x338855,
            base_url: String::from("http://127.0.0.1:8000"),
            variables: Vec::new(),
            headers: Vec::new(),
        }
    }

    pub(crate) fn ensure_renderable_workspace(&mut self) {
        if self.groups.is_empty() || self.groups.iter().all(|group| group.is_empty()) {
            self.groups = vec![crate::service::ApiGroup {
                id: None,
                name: String::from("集合"),
                folders: Vec::new(),
                requests: vec![Self::placeholder_request()],
            }];
            self.selected_request = 0;
            self.selected_scenario = None;
            self.active_tab = OpenTab::new_request(0);
            self.open_tabs = vec![self.active_tab.clone()];
        } else {
            let request_count: usize = self
                .groups
                .iter()
                .map(|group| group.total_request_count())
                .sum();
            if self.selected_request >= request_count {
                self.selected_request = 0;
                self.selected_scenario = None;
                self.active_tab = self.request_tab_for_index(0);
            }
            if let Some(scenario_index) = self.selected_scenario {
                let valid = request_at(&self.groups, self.selected_request)
                    .and_then(|request| request.scenarios.get(scenario_index))
                    .is_some();
                if !valid {
                    self.selected_scenario = None;
                    self.active_tab = self.request_tab_for_index(self.selected_request);
                }
            }
            self.open_tabs.retain(|tab| match tab {
                OpenTab::Request { index, .. } => *index < request_count,
                OpenTab::Scenario {
                    request_index,
                    scenario_index,
                    ..
                } => request_at(&self.groups, *request_index)
                    .and_then(|request| request.scenarios.get(*scenario_index))
                    .is_some(),
            });
            if self.open_tabs.is_empty() {
                self.active_tab = OpenTab::new_request(self.selected_request);
                self.open_tabs.push(self.active_tab.clone());
            }
        }

        if self.environments.is_empty() {
            self.environments = vec![Self::placeholder_environment()];
        }
        self.selected_environment = self
            .selected_environment
            .min(self.environments.len().saturating_sub(1));
    }

    pub(crate) fn sync_service_updates(&mut self, cx: &mut App) {
        let had_pending_groups = self.service.take_pending_groups();
        let had_pending_environments = self.service.take_pending_environments();

        if let Some(groups) = had_pending_groups {
            self.groups = groups;
            self.last_revision = self.service.revision();
            self.ensure_renderable_workspace();
            let items = build_tree_items(&self.groups, &mut 0, &self.collapsed_nodes.borrow());
            let saved_ix = self.tree_state.read(cx).selected_index();
            self.tree_state.update(cx, |tree, cx| {
                tree.set_items(items, cx);
                tree.set_selected_index(saved_ix, cx);
            });
        } else {
            let current_revision = self.service.revision();
            if current_revision != self.last_revision {
                if let Ok(workspace) = self.service.load_workspace() {
                    self.groups = workspace.groups;
                    self.environments = workspace.environments;
                    self.ensure_renderable_workspace();
                    let items =
                        build_tree_items(&self.groups, &mut 0, &self.collapsed_nodes.borrow());
                    let saved_ix = self.tree_state.read(cx).selected_index();
                    self.tree_state.update(cx, |tree, cx| {
                        tree.set_items(items, cx);
                        tree.set_selected_index(saved_ix, cx);
                    });
                }
                self.last_revision = current_revision;
            }
        }

        if let Some(environments) = had_pending_environments {
            self.environments = environments;
            self.ensure_renderable_workspace();
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
            if self.response_tab == crate::service::ResponseTab::History {
                self.refresh_history();
            }
        }
        if let Some(error) = self.service.take_pending_error() {
            self.notice = format!("请求失败: {error}");
        }
        if let Some(notice) = self.service.take_pending_notice() {
            self.notice = notice;
        }

        self.flush_pending_persist(cx);
    }

    pub(crate) fn selected_base_request(&self) -> &ApiRequest {
        request_at(&self.groups, self.selected_request).expect("request should exist")
    }

    pub fn selected_request(&self) -> &ApiRequest {
        let request = self.selected_base_request();
        if let Some(scenario_index) = self.selected_scenario {
            if let Some(scenario_request) = request
                .scenarios
                .get(scenario_index)
                .and_then(|scenario| scenario.request.as_deref())
            {
                return scenario_request;
            }
        }
        request
    }

    pub(crate) fn selected_request_mut(&mut self) -> &mut ApiRequest {
        let request =
            request_at_mut(&mut self.groups, self.selected_request).expect("request should exist");
        if let Some(scenario_index) = self.selected_scenario {
            let base_request = request.clone();
            let scenario = request
                .scenarios
                .get_mut(scenario_index)
                .expect("scenario should exist");
            if scenario.request.is_none() {
                let mut scenario_request = base_request;
                scenario_request.node_id = scenario.node_id.clone();
                scenario_request.title = scenario.name.clone();
                scenario_request.scenarios.clear();
                scenario.request = Some(Box::new(scenario_request));
            }
            return scenario
                .request
                .as_deref_mut()
                .expect("scenario request should exist");
        }
        request
    }

    pub(crate) fn select_request(&mut self, index: usize, window: &mut Window, cx: &mut App) {
        self.sync_models(cx);
        self.flush_pending_persist(cx);
        self.selected_request = index;
        self.selected_scenario = None;
        let new_tab = self.request_tab_for_index(index);
        let new_tab_id = new_tab.tab_id().to_string();
        self.active_tab = new_tab.clone();

        if let Some(persisted) = self.service.load_persisted_tab_by_id(&new_tab_id) {
            self.restore_inputs_from_tab(&persisted, window, cx);
            let tab_idx = persisted.active_request_tab;
            if let Some(et) = crate::service::index_to_editor_tab(tab_idx) {
                self.editor_tab = et;
            }
        } else {
            self.reload_request_inputs(window, cx);
        }

        self.persist_current_tab_state(cx);
        self.notice = format!("已切换到 {}", self.selected_request().title);
    }

    pub(crate) fn select_scenario(
        &mut self,
        request_index: usize,
        scenario_index: usize,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.sync_models(cx);
        self.flush_pending_persist(cx);
        self.selected_request = request_index;
        self.selected_scenario = Some(scenario_index);
        let new_tab = self.scenario_tab_for_index(request_index, scenario_index);
        let new_tab_id = new_tab.tab_id().to_string();
        self.active_tab = new_tab.clone();

        if let Some(persisted) = self.service.load_persisted_tab_by_id(&new_tab_id) {
            self.restore_inputs_from_tab(&persisted, window, cx);
            let tab_idx = persisted.active_request_tab;
            if let Some(et) = crate::service::index_to_editor_tab(tab_idx) {
                self.editor_tab = et;
            }
        } else {
            self.reload_request_inputs(window, cx);
        }

        self.persist_current_tab_state(cx);
        self.notice = format!("已切换到场景 {}", self.current_title());
    }

    pub(crate) fn sync_models(&mut self, cx: &App) {
        let path = self.path_input.read(cx).value().to_string();
        let params = self.params_kv.to_rows(cx);
        let path_rows = self.path_kv.to_rows(cx);
        let body = self.body_input.read(cx).value().to_string();
        let headers = self.headers_kv.to_rows(cx);
        let cookies = self.cookies_kv.to_rows(cx);
        let auth = self.auth_rows(cx);
        let pre_ops = self.pre_ops_input.read(cx).value().to_string();
        let post_ops = self.post_ops_input.read(cx).value().to_string();
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

        let env_name = self.env_name_input.read(cx).value().to_string();
        let env_base_url = self.env_base_url_input.read(cx).value().to_string();
        let env_variables = super::types::parse_rows(&self.env_variables_input.read(cx).value().to_string());
        let env_headers = super::types::parse_rows(&self.env_headers_input.read(cx).value().to_string());

        {
            let environment = self.selected_environment_mut();
            environment.name = env_name;
            environment.base_url = env_base_url;
            environment.variables = env_variables;
            environment.headers = env_headers;
        }
    }

    pub(crate) fn reload_request_inputs(&mut self, window: &mut Window, cx: &mut App) {
        let request = self.selected_request().clone();
        self.path_input.update(cx, |input, input_cx| {
            input.reset_value(request.path.clone(), input_cx)
        });
        self.params_kv.set_rows(window, cx, &request.params);
        self.path_kv.set_rows(window, cx, &request.path_rows);
        self.body_input.update(cx, |input, input_cx| {
            input.reset_value(request.body.clone(), input_cx)
        });
        self.headers_kv.set_rows(window, cx, &request.headers);
        self.cookies_kv.set_rows(window, cx, &request.cookies);
        self.load_auth_form(cx, &request.auth);
        self.pre_ops_input.update(cx, |input, input_cx| {
            input.reset_value(request.pre_ops.clone(), input_cx)
        });
        self.post_ops_input.update(cx, |input, input_cx| {
            input.reset_value(request.post_ops.clone(), input_cx)
        });
    }

    pub(crate) fn set_method(&mut self, method: HttpMethod, cx: &App) {
        self.sync_models(cx);
        let request = self.selected_request_mut();
        request.method = method;
        self.notice = format!("请求方法已切换为 {}", request.method.label());
        self.persist_workspace();
        self.persist_current_tab_state(cx);
    }

    pub(crate) fn send_request(&mut self, cx: &mut App) {
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

    pub(crate) fn cancel_request(&mut self, _cx: &App) {
        self.service.cancel_request();
        self.notice = String::from("请求已取消");
    }

    pub(crate) fn persist_endpoint_if_needed(&self) {
        if self.groups.is_empty() {
            return;
        }
        let request = self.selected_request();
        if request.node_id.is_empty() {
            return;
        }
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

    pub(crate) fn current_node_id(&self) -> &str {
        self.selected_request().node_id.as_str()
    }

    pub(crate) fn persist_workspace(&mut self) {
        self.persist_endpoint_if_needed();
    }
}
