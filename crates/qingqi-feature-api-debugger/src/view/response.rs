use super::ApiDebuggerView;
use crate::code_gen::CodeLanguage;
use crate::service::ResponseTab;
use gpui::App;

impl ApiDebuggerView {
    pub(crate) fn response_text(&self) -> String {
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
            ResponseTab::History => String::new(),
            ResponseTab::Code => self.code_snippet(),
        }
    }

    fn code_snippet(&self) -> String {
        crate::service::code_snippet(
            self.selected_environment(),
            self.selected_request(),
            self.response_code_lang,
        )
    }

    pub(crate) fn set_response_tab(&mut self, tab: ResponseTab) {
        self.response_tab = tab;
        if tab == ResponseTab::History {
            self.refresh_history();
        }
    }

    pub(crate) fn set_response_code_lang(&mut self, lang: CodeLanguage) {
        self.response_code_lang = lang;
    }

    pub(crate) fn refresh_history(&mut self) {
        let node_id = self.current_node_id().to_string();
        match self.service.list_history(&node_id, 50) {
            Ok(rows) => self.history_entries = rows,
            Err(error) => {
                self.history_entries.clear();
                tracing::warn!("加载历史记录失败: {error}");
            }
        }
    }

    pub(crate) fn clear_current_history(&mut self) {
        let node_id = self.current_node_id().to_string();
        match self.service.clear_history(&node_id) {
            Ok(count) => {
                self.history_entries.clear();
                self.notice = format!("已清空 {count} 条历史记录");
            }
            Err(error) => self.notice = format!("清空历史失败: {error}"),
        }
    }

    pub(crate) fn view_history_entry(&mut self, index: usize) {
        let Some(entry) = self.history_entries.get(index) else {
            return;
        };
        let created_at = entry.created_at.clone();
        self.response.status_line = format!("{} {} · {}", entry.method, entry.status, entry.url);
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

    pub(crate) fn copy_response_body(&mut self, cx: &mut App) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(self.response.body.clone()));
        self.notice = String::from("响应已复制到剪贴板");
    }

    pub(crate) fn format_response_body(&mut self) {
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

    pub(crate) fn save_response_body(&mut self) {
        use super::types::content_type_extension;
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
}
