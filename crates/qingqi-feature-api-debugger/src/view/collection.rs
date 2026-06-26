use super::ApiDebuggerView;
use super::components::collection_tree::MenuKind;
use crate::model::NodeKind;
use gpui::App;

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
        self.renaming_node_id = node_id;
        self.rename_inline_input.update(cx, |input, input_cx| {
            input.reset_value(current_name, input_cx);
        });
        self.close_collection_menu();
    }

    pub(crate) fn confirm_inline_rename(&mut self, cx: &App) {
        let new_name = self.rename_inline_input.read(cx).value().to_string().trim().to_string();
        let node_id = std::mem::take(&mut self.renaming_node_id);
        if node_id.is_empty() {
            return;
        }
        if new_name.is_empty() {
            self.notice = String::from("名称不能为空");
            self.renaming_node_id = node_id;
            return;
        }
        self.service.rename_collection_item_async(node_id, new_name);
        self.notice = String::from("正在重命名...");
    }

    pub(crate) fn cancel_inline_rename(&mut self) {
        self.renaming_node_id = String::new();
    }

    pub(crate) fn confirm_rename(&mut self, cx: &App) {
        let new_name = self.rename_input.read(cx).value().to_string().trim().to_string();
        let node_id = self.rename_node_id.clone();
        if node_id.is_empty() {
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
