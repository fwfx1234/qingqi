use gpui::{App, Entity};
use crate::service::{AuthType, EditorTab, KeyValueRow};
use super::ApiDebuggerView;
use super::types::AuthFormInputs;

impl ApiDebuggerView {
    pub fn text_editor_input(&self, tab: EditorTab) -> Option<Entity<qingqi_ui::text_input::TextInput>> {
        
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

    pub fn auth_form_inputs(&self) -> AuthFormInputs {
        AuthFormInputs {
            bearer: self.auth_bearer_input.clone(),
            basic_user: self.auth_basic_user_input.clone(),
            basic_pass: self.auth_basic_pass_input.clone(),
            apikey_name: self.auth_apikey_name_input.clone(),
            apikey_value: self.auth_apikey_value_input.clone(),
            in_query: self.auth_apikey_in_query,
        }
    }

    pub fn auth_rows(&self, cx: &App) -> Vec<KeyValueRow> {
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
                    let encoded = crate::service::base64_encode(format!("{user}:{pass}").as_bytes());
                    vec![KeyValueRow::new("Authorization", format!("Basic {encoded}"))]
                }
            }
            AuthType::ApiKey => {
                let name = self
                    .auth_apikey_name_input
                    .read(cx)
                    .text()
                    .trim()
                    .to_string();
                let value = self
                    .auth_apikey_value_input
                    .read(cx)
                    .text()
                    .trim()
                    .to_string();
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

    pub fn load_auth_form(&mut self, cx: &mut App, rows: &[KeyValueRow]) {
        use super::types::derive_auth_form;
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

    pub fn kv_editor(&self, tab: EditorTab) -> Option<&super::types::KvEditor> {
        match tab {
            EditorTab::Params => Some(&self.params_kv),
            EditorTab::Path => Some(&self.path_kv),
            EditorTab::Headers => Some(&self.headers_kv),
            EditorTab::Cookies => Some(&self.cookies_kv),
            _ => None,
        }
    }

    pub fn kv_editor_mut(&mut self, tab: EditorTab) -> Option<&mut super::types::KvEditor> {
        match tab {
            EditorTab::Params => Some(&mut self.params_kv),
            EditorTab::Path => Some(&mut self.path_kv),
            EditorTab::Headers => Some(&mut self.headers_kv),
            EditorTab::Cookies => Some(&mut self.cookies_kv),
            _ => None,
        }
    }

    pub fn format_json_body(&mut self, cx: &mut App) {
        let text = self.body_input.read(cx).text();
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => {
                let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.clone());
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

    pub fn pick_binary_file(&mut self, cx: &mut App) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("选择要上传的文件")
            .pick_file()
        else {
            self.notice = String::from("已取消选择文件");
            return;
        };
        let path_string = path.display().to_string();
        self.body_input.update(cx, |input, input_cx| {
            input.set_text(path_string.clone(), input_cx)
        });
        self.sync_models(cx);
        self.persist_current_tab_state(cx);
        self.notice = format!("已选择文件: {path_string}");
    }
}
