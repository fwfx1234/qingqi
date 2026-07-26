use super::ApiDebuggerView;
use super::components::collection_tree::MenuKind;
use crate::model::NodeKind;
use gpui::{App, Context};
use std::time::Instant;

impl ApiDebuggerView {
    #[allow(dead_code)]
    pub(crate) fn open_collection_menu(
        &mut self,
        title: impl Into<String>,
        position: Option<(f32, f32)>,
        node_id: String,
        kind: MenuKind,
    ) {
        self.collection_menu_title = title.into();
        self.collection_menu_position = position;
        self.collection_menu_node_id = node_id;
        self.collection_menu_kind = Some(kind);
        self.show_collection_menu = true;
    }

    pub(crate) fn close_collection_menu(&mut self) {
        self.show_collection_menu = false;
        self.collection_menu_position = None;
        self.collection_menu_node_id = String::new();
        self.collection_menu_kind = None;
    }

    pub(crate) fn create_new_endpoint(&mut self) {
        let parent_id = self.find_parent_id_for_new_node(NodeKind::Endpoint);
        let title = String::from("新请求");
        self.service
            .create_endpoint_async(parent_id, title, "GET".into(), "/".into());
        self.close_collection_menu();
    }

    pub(crate) fn create_new_folder(&mut self) {
        let parent_id = self.find_parent_id_for_new_node(NodeKind::Folder);
        let title = String::from("新分组");
        self.service.create_folder_async(parent_id, title);
        self.close_collection_menu();
    }

    pub(crate) fn create_new_case(&mut self) {
        let parent_id = self
            .find_parent_id_for_new_node(NodeKind::Case)
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

    pub(crate) fn delete_selected_collection_item(&mut self) {
        let node_id = self.collection_menu_node_id.clone();
        if !node_id.is_empty() {
            self.service.delete_collection_item_async(node_id);
        }
        self.close_collection_menu();
    }

    pub(crate) fn find_parent_id_for_new_node(&self, new_kind: NodeKind) -> Option<String> {
        let menu_node_id = self.collection_menu_node_id.trim();
        if menu_node_id.is_empty() {
            return None;
        }
        let Ok(Some(node)) = self.service.get_collection_node(menu_node_id) else {
            return None;
        };
        match (new_kind, node.kind) {
            (NodeKind::Endpoint | NodeKind::Folder, NodeKind::Folder) => Some(node.id),
            (NodeKind::Endpoint | NodeKind::Folder, NodeKind::Endpoint) => node.parent_id,
            (NodeKind::Endpoint | NodeKind::Folder, NodeKind::Case) => node
                .parent_id
                .and_then(|endpoint_id| {
                    self.service
                        .get_collection_node(&endpoint_id)
                        .ok()
                        .flatten()
                })
                .and_then(|endpoint| endpoint.parent_id),
            (NodeKind::Case, NodeKind::Endpoint) => Some(node.id),
            (NodeKind::Case, NodeKind::Case) => node.parent_id,
            (NodeKind::Case, NodeKind::Folder) => None,
        }
    }

    pub(crate) fn import_curl(&mut self, cx: &App) {
        let curl_text = self.curl_import_input.read(cx).value().to_string();
        if !curl_text.is_empty() {
            self.service.import_from_curl_async(curl_text);
        }
        self.show_curl_import = false;
    }

    pub(crate) fn export_openapi(&mut self) {
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

    pub(crate) fn import_openapi_file(&mut self) {
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

    pub(crate) fn import_postman_file(&mut self) {
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

    pub(crate) fn open_rename(&mut self, cx: &mut App) {
        let started = Instant::now();
        let node_id = self.collection_menu_node_id.clone();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            node_id,
            step = "ui_open_rename_started",
            "API 调试器重命名：开始打开重命名输入框"
        );
        if node_id.is_empty() {
            tracing::warn!(
                target: "qingqi::api_debugger::rename",
                duration_ms = started.elapsed().as_millis(),
                step = "ui_open_rename_rejected",
                "API 调试器重命名：未选择具体节点"
            );
            self.notice = String::from("请在具体节点上重命名");
            self.close_collection_menu();
            return;
        }
        let lookup_started = Instant::now();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            node_id,
            step = "ui_node_lookup_started",
            "API 调试器重命名：开始读取当前节点名称"
        );
        let current_name = match self.service.get_collection_node(&node_id) {
            Ok(Some(node)) => {
                tracing::info!(
                    target: "qingqi::api_debugger::rename",
                    node_id,
                    step_duration_ms = lookup_started.elapsed().as_millis(),
                    total_duration_ms = started.elapsed().as_millis(),
                    step = "ui_node_lookup_completed",
                    "API 调试器重命名：当前节点名称读取完成"
                );
                node.name
            }
            Ok(None) => {
                tracing::warn!(
                    target: "qingqi::api_debugger::rename",
                    node_id,
                    step_duration_ms = lookup_started.elapsed().as_millis(),
                    total_duration_ms = started.elapsed().as_millis(),
                    step = "ui_node_lookup_missing",
                    "API 调试器重命名：当前节点不存在"
                );
                self.notice = String::from("节点不存在，无法重命名");
                self.close_collection_menu();
                return;
            }
            Err(error) => {
                tracing::warn!(
                    target: "qingqi::api_debugger::rename",
                    node_id,
                    error = %error,
                    step_duration_ms = lookup_started.elapsed().as_millis(),
                    total_duration_ms = started.elapsed().as_millis(),
                    step = "ui_node_lookup_failed",
                    "API 调试器重命名：读取当前节点名称失败"
                );
                self.notice = format!("读取节点失败: {error}");
                self.close_collection_menu();
                return;
            }
        };
        self.renaming_node_id = node_id;
        self.focus_inline_rename = true;
        self.rename_inline_input.update(cx, |input, input_cx| {
            input.reset_value(current_name, input_cx);
        });
        self.close_collection_menu();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            node_id = self.renaming_node_id,
            duration_ms = started.elapsed().as_millis(),
            step = "ui_open_rename_completed",
            "API 调试器重命名：重命名输入框打开完成"
        );
    }

    pub(crate) fn confirm_inline_rename(&mut self, cx: &mut Context<Self>) {
        // The input is focused on the next GPUI turn. Ignore the initial blur
        // generated while that focus transition is still pending.
        if self.focus_inline_rename {
            tracing::info!(
                target: "qingqi::api_debugger::rename",
                node_id = self.renaming_node_id,
                step = "ui_confirm_deferred_until_focus",
                "API 调试器重命名：输入框仍在等待聚焦，忽略初始失焦"
            );
            return;
        }
        self.focus_inline_rename = false;
        let new_name = self
            .rename_inline_input
            .read(cx)
            .value()
            .to_string()
            .trim()
            .to_string();
        let node_id = std::mem::take(&mut self.renaming_node_id);
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            node_id,
            new_name_len = new_name.chars().count(),
            step = "ui_confirm_received",
            "API 调试器重命名：收到内联确认"
        );
        if node_id.is_empty() {
            tracing::warn!(
                target: "qingqi::api_debugger::rename",
                step = "ui_confirm_ignored",
                "API 调试器重命名：没有待重命名节点，忽略重复确认"
            );
            return;
        }
        if new_name.is_empty() {
            tracing::warn!(
                target: "qingqi::api_debugger::rename",
                node_id,
                step = "ui_confirm_rejected",
                "API 调试器重命名：新名称为空"
            );
            self.notice = String::from("名称不能为空");
            self.renaming_node_id = node_id;
            return;
        }
        let dispatch_started = Instant::now();
        self.service.rename_collection_item_async(node_id, new_name);
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            dispatch_duration_ms = dispatch_started.elapsed().as_millis(),
            step = "ui_dispatch_completed",
            "API 调试器重命名：后台任务提交完成"
        );
        self.notice = String::from("正在重命名...");
        cx.notify();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            step = "ui_notify_completed",
            "API 调试器重命名：界面刷新通知完成"
        );
    }

    pub(crate) fn cancel_inline_rename(&mut self) {
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            node_id = self.renaming_node_id,
            step = "ui_rename_cancelled",
            "API 调试器重命名：用户取消内联重命名"
        );
        self.renaming_node_id = String::new();
        self.focus_inline_rename = false;
    }

    pub(crate) fn toggle_expansion(&mut self, node_id: String, cx: &mut App) {
        {
            let mut collapsed = self.collapsed_nodes.borrow_mut();
            if collapsed.contains(&node_id) {
                collapsed.remove(&node_id);
            } else {
                collapsed.insert(node_id);
            }
        }
        let items = super::components::collection_tree::build_tree_items(
            &self.groups,
            &mut 0,
            &self.collapsed_nodes.borrow(),
        );
        let saved_ix = self.tree_state.read(cx).selected_index();
        self.tree_state.update(cx, |tree, cx| {
            tree.set_items(items, cx);
            tree.set_selected_index(saved_ix, cx);
        });
    }

    pub(crate) fn confirm_rename(&mut self, cx: &App) {
        let started = Instant::now();
        let new_name = self
            .rename_input
            .read(cx)
            .value()
            .to_string()
            .trim()
            .to_string();
        let node_id = self.rename_node_id.clone();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            node_id,
            new_name_len = new_name.chars().count(),
            step = "ui_dialog_confirm_received",
            "API 调试器重命名：收到弹窗确认"
        );
        if node_id.is_empty() {
            tracing::warn!(
                target: "qingqi::api_debugger::rename",
                duration_ms = started.elapsed().as_millis(),
                step = "ui_dialog_confirm_rejected",
                "API 调试器重命名：弹窗未指定节点"
            );
            self.notice = String::from("请先选择要重命名的节点");
            self.show_rename = false;
            return;
        } else if new_name.is_empty() {
            tracing::warn!(
                target: "qingqi::api_debugger::rename",
                node_id,
                duration_ms = started.elapsed().as_millis(),
                step = "ui_dialog_confirm_rejected",
                "API 调试器重命名：弹窗新名称为空"
            );
            self.notice = String::from("名称不能为空");
            return;
        } else {
            self.service.rename_collection_item_async(node_id, new_name);
        }
        self.show_rename = false;
        self.rename_node_id = String::new();
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            duration_ms = started.elapsed().as_millis(),
            step = "ui_dialog_dispatch_completed",
            "API 调试器重命名：弹窗后台任务提交完成"
        );
    }
}
