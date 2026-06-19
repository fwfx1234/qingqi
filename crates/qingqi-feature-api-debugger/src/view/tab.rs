use gpui::App;

use super::ApiDebuggerView;
use super::types::{OpenTab, request_at};

impl ApiDebuggerView {
    pub(crate) fn ensure_open_tab(&mut self, tab: OpenTab) -> OpenTab {
        if let Some(existing) = self
            .open_tabs
            .iter()
            .find(|open_tab| **open_tab == tab)
            .cloned()
        {
            existing
        } else {
            self.open_tabs.push(tab.clone());
            tab
        }
    }

    pub(crate) fn request_tab_for_index(&mut self, index: usize) -> OpenTab {
        if let Some(existing) = self
            .open_tabs
            .iter()
            .find(|tab| tab.matches_request_index(index))
            .cloned()
        {
            return existing;
        }
        self.ensure_open_tab(OpenTab::new_request(index))
    }

    pub(crate) fn scenario_tab_for_index(
        &mut self,
        request_index: usize,
        scenario_index: usize,
    ) -> OpenTab {
        if let Some(existing) = self
            .open_tabs
            .iter()
            .find(|tab| tab.matches_scenario_index(request_index, scenario_index))
            .cloned()
        {
            return existing;
        }
        let node_id = self.scenario_node_id(request_index, scenario_index);
        self.ensure_open_tab(OpenTab::new_scenario_with_node(
            request_index,
            scenario_index,
            node_id,
        ))
    }

    pub(crate) fn scenario_node_id(&self, request_index: usize, scenario_index: usize) -> String {
        request_at(&self.groups, request_index)
            .and_then(|request| request.scenarios.get(scenario_index))
            .map(|scenario| scenario.node_id.clone())
            .unwrap_or_default()
    }

    fn collect_tab_draft(&self, cx: &App) -> crate::service::TabDraft {
        crate::service::TabDraft {
            url: self.path_input.read(cx).text(),
            params_text: self.params_kv.to_text(cx),
            path_params_text: self.path_kv.to_text(cx),
            body_text: self.body_input.read(cx).text(),
            headers_text: self.headers_kv.to_text(cx),
            cookies_text: self.cookies_kv.to_text(cx),
            auth_text: super::types::format_rows(&self.auth_rows(cx)),
            pre_ops_text: self.pre_ops_input.read(cx).text(),
            post_ops_text: self.post_ops_input.read(cx).text(),
            active_request_tab: crate::service::editor_tab_index(self.editor_tab),
        }
    }

    pub(crate) fn persist_current_tab_state(&mut self, _cx: &App) {
        let tab_id = self.active_tab.tab_id().to_string();
        if tab_id.is_empty() {
            return;
        }
        self.pending_persist = true;
    }

    pub(crate) fn flush_pending_persist(&mut self, cx: &App) {
        if !self.pending_persist {
            return;
        }
        let tab_id = self.active_tab.tab_id().to_string();
        if tab_id.is_empty() {
            self.pending_persist = false;
            return;
        }
        let request = self.selected_request();
        let draft = self.collect_tab_draft(cx);
        let tab = crate::service::build_http_tab(
            &tab_id,
            super::types::first_non_empty(
                self.current_node_id(),
                self.active_tab.fallback_node_id(),
            ),
            &request.title,
            request.method.label(),
            &draft,
        );
        self.service.save_tab_state_async(tab);
        self.pending_persist = false;
    }

    pub(crate) fn restore_inputs_from_tab(&mut self, tab: &crate::model::HttpTab, cx: &mut App) {
        let draft = crate::service::restore_tab_draft(tab);
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
        let auth_rows = super::types::parse_rows(&draft.auth_text);
        self.load_auth_form(cx, &auth_rows);
    }

    pub(crate) fn close_open_tab(&mut self, tab_index: usize, cx: &mut App) {
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

    pub(crate) fn select_open_tab(&mut self, tab: OpenTab, cx: &mut App) {
        self.sync_models(cx);
        self.flush_pending_persist(cx);
        self.active_tab = tab.clone();
        match tab {
            OpenTab::Request { index, tab_id, .. } => {
                self.selected_request = index;
                self.selected_scenario = None;
                if let Some(persisted) = self.service.load_persisted_tab_by_id(&tab_id) {
                    self.restore_inputs_from_tab(&persisted, cx);
                    if let Some(et) =
                        crate::service::index_to_editor_tab(persisted.active_request_tab)
                    {
                        self.editor_tab = et;
                    }
                } else {
                    self.reload_request_inputs(cx);
                }
            }
            OpenTab::Scenario {
                request_index,
                scenario_index,
                tab_id,
                ..
            } => {
                self.selected_request = request_index;
                self.selected_scenario = Some(scenario_index);
                if let Some(persisted) = self.service.load_persisted_tab_by_id(&tab_id) {
                    self.restore_inputs_from_tab(&persisted, cx);
                    if let Some(et) =
                        crate::service::index_to_editor_tab(persisted.active_request_tab)
                    {
                        self.editor_tab = et;
                    }
                } else {
                    self.reload_request_inputs(cx);
                }
            }
        }
        self.notice = format!("已切换到 {}", self.current_title());
        self.persist_current_tab_state(cx);
    }

    pub(crate) fn tab_title(&self, tab: &OpenTab) -> String {
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

    pub(crate) fn current_title(&self) -> String {
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
}
