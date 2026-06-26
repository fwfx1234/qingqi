use super::ApiDebuggerView;
use super::types::format_rows;
use crate::service::ApiEnvironment;
use gpui::App;

impl ApiDebuggerView {
    pub(crate) fn selected_environment(&self) -> &ApiEnvironment {
        self.environments
            .get(self.selected_environment)
            .expect("environment should exist")
    }

    pub(crate) fn selected_environment_mut(&mut self) -> &mut ApiEnvironment {
        self.environments
            .get_mut(self.selected_environment)
            .expect("environment should exist")
    }

    pub(crate) fn select_environment(&mut self, index: usize, cx: &mut App) {
        self.sync_models(cx);
        self.persist_workspace();
        self.selected_environment = index;
        self.reload_environment_inputs(cx);
        self.notice = format!("已切换到 {}", self.selected_environment().name);
    }

    pub(crate) fn reload_environment_inputs(&mut self, cx: &mut App) {
        let environment = self.selected_environment().clone();
        self.env_name_input.update(cx, |input, input_cx| {
            input.reset_value(environment.name.clone(), input_cx)
        });
        self.env_base_url_input.update(cx, |input, input_cx| {
            input.reset_value(environment.base_url.clone(), input_cx)
        });
        self.env_variables_input.update(cx, |input, input_cx| {
            input.reset_value(format_rows(&environment.variables), input_cx)
        });
        self.env_headers_input.update(cx, |input, input_cx| {
            input.reset_value(format_rows(&environment.headers), input_cx)
        });
    }

    pub(crate) fn save_environment_changes(&mut self, cx: &mut App) {
        self.sync_models(cx);
        let env = self.selected_environment().clone();
        self.service.save_environment_fields_async(
            self.selected_environment,
            env.name.clone(),
            env.base_url.clone(),
            format_rows(&env.variables),
            format_rows(&env.headers),
        );
        self.notice = String::from("正在保存环境...");
    }

    pub(crate) fn reset_environment_changes(&mut self, cx: &mut App) {
        self.reload_environment_inputs(cx);
        self.notice = String::from("已重置环境编辑内容");
    }

    pub(crate) fn create_new_environment(&mut self) {
        self.service
            .create_environment_async(String::from("新环境"), String::new());
        self.notice = String::from("正在创建环境...");
    }

    pub(crate) fn duplicate_current_environment(&mut self, cx: &mut App) {
        self.sync_models(cx);
        self.service
            .duplicate_environment_async(self.selected_environment);
        self.notice = String::from("正在复制环境...");
    }

    pub(crate) fn delete_current_environment(&mut self, cx: &mut App) {
        self.sync_models(cx);
        self.service
            .delete_environment_by_index_async(self.selected_environment);
        self.notice = String::from("正在删除环境...");
    }

    pub(crate) fn export_environments(&mut self) {
        let json = match self.service.export_environments_json() {
            Ok(json) => json,
            Err(error) => {
                self.notice = format!("环境导出失败: {error}");
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title("导出 API 环境")
            .set_file_name("qingqi-api-environments.json")
            .save_file()
        else {
            self.notice = String::from("已取消导出");
            return;
        };
        match std::fs::write(&path, json) {
            Ok(()) => self.notice = format!("环境已导出: {}", path.display()),
            Err(error) => self.notice = format!("环境导出失败: {error}"),
        }
    }

    pub(crate) fn import_environments(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("导入 API 环境 JSON")
            .add_filter("JSON", &["json"])
            .pick_file()
        else {
            self.notice = String::from("已取消导入");
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.service.import_environments_json_async(content);
                self.notice = format!("正在导入环境: {}", path.display());
            }
            Err(error) => self.notice = format!("环境导入失败: {error}"),
        }
    }

    pub(crate) fn close_env_editor_window(&mut self, cx: &mut App) {
        let Some(handle) = self.env_editor_window.take() else {
            return;
        };
        cx.defer(move |cx| {
            if let Err(error) = handle.update(cx, |_, window, _| window.remove_window()) {
                tracing::warn!(
                    target: "qingqi_api_debugger",
                    error = %error,
                    "关闭环境编辑窗口失败"
                );
            }
        });
    }
}
