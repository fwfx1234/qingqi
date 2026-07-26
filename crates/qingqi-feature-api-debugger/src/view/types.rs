use gpui::{App, AppContext, Entity, Window};
use qingqi_ui::components::input::InputState;

use crate::service::{
    self, ApiGroup, ApiRequest, ApiResponse, AuthType, BodyMode, EditorTab, KeyValueRow,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KvEditorTarget {
    Tab(EditorTab),
    Body(BodyMode),
}

#[derive(Clone)]
pub struct KvRow {
    pub enabled: bool,
    pub key: Entity<InputState>,
    pub value: Entity<InputState>,
    pub value_type: Entity<InputState>,
    pub description: Entity<InputState>,
}

pub struct KvEditor {
    pub rows: Vec<KvRow>,
}

impl KvEditor {
    pub fn new(window: &mut Window, cx: &mut App, rows: &[KeyValueRow]) -> Self {
        let mut editor = Self { rows: Vec::new() };
        editor.set_rows(window, cx, rows);
        editor
    }

    pub fn from_text(window: &mut Window, cx: &mut App, text: &str) -> Self {
        Self::new(window, cx, &parse_rows(text))
    }

    pub fn set_rows(&mut self, window: &mut Window, cx: &mut App, rows: &[KeyValueRow]) {
        self.rows = rows
            .iter()
            .map(|row| KvRow {
                enabled: row.enabled,
                key: kv_input(window, cx, &row.key, "键"),
                value: kv_input(window, cx, &row.value, "值"),
                value_type: kv_input(window, cx, &row.value_type, "string"),
                description: kv_input(window, cx, &row.description, "说明"),
            })
            .collect();
    }

    pub fn set_from_text(&mut self, window: &mut Window, cx: &mut App, text: &str) {
        self.set_rows(window, cx, &parse_rows(text));
    }

    pub fn to_rows(&self, cx: &App) -> Vec<KeyValueRow> {
        self.rows
            .iter()
            .map(|row| KeyValueRow {
                enabled: row.enabled,
                key: row.key.read(cx).value().trim().to_string(),
                value: row.value.read(cx).value().trim().to_string(),
                value_type: row.value_type.read(cx).value().trim().to_string(),
                description: row.description.read(cx).value().trim().to_string(),
            })
            .collect()
    }

    pub fn to_text(&self, cx: &App) -> String {
        format_rows(&self.to_rows(cx))
    }

    pub fn add_row(&mut self, window: &mut Window, cx: &mut App) {
        self.rows.push(KvRow {
            enabled: true,
            key: kv_input(window, cx, "", "键"),
            value: kv_input(window, cx, "", "值"),
            value_type: kv_input(window, cx, "", "string"),
            description: kv_input(window, cx, "", "说明"),
        });
    }

    pub fn remove_row(&mut self, index: usize) {
        if index < self.rows.len() {
            self.rows.remove(index);
        }
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(row) = self.rows.get_mut(index) {
            row.enabled = !row.enabled;
        }
    }

    pub fn update_row_values(&mut self, rows: &[KeyValueRow], cx: &mut App) {
        let len = self.rows.len().min(rows.len());
        for i in 0..len {
            let row = &mut self.rows[i];
            let new = &rows[i];
            if row.key.read(cx).value() != new.key {
                row.key.update(cx, |input, input_cx| {
                    input.reset_value(new.key.clone(), input_cx)
                });
            }
            if row.value.read(cx).value() != new.value {
                row.value.update(cx, |input, input_cx| {
                    input.reset_value(new.value.clone(), input_cx)
                });
            }
            if row.value_type.read(cx).value() != new.value_type {
                row.value_type.update(cx, |input, input_cx| {
                    input.reset_value(new.value_type.clone(), input_cx)
                });
            }
            if row.description.read(cx).value() != new.description {
                row.description.update(cx, |input, input_cx| {
                    input.reset_value(new.description.clone(), input_cx)
                });
            }
        }
    }

    pub fn adjust_row_count(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        rows: &[KeyValueRow],
    ) {
        while self.rows.len() > rows.len() {
            self.rows.pop();
        }
        while self.rows.len() < rows.len() {
            let row = &rows[self.rows.len()];
            self.rows.push(KvRow {
                enabled: row.enabled,
                key: kv_input(window, cx, &row.key, "键"),
                value: kv_input(window, cx, &row.value, "值"),
                value_type: kv_input(window, cx, &row.value_type, "string"),
                description: kv_input(window, cx, &row.description, "说明"),
            });
        }
    }
}

#[derive(Clone)]
pub struct AuthFormInputs {
    pub bearer: Entity<InputState>,
    pub basic_user: Entity<InputState>,
    pub basic_pass: Entity<InputState>,
    pub apikey_name: Entity<InputState>,
    pub apikey_value: Entity<InputState>,
    pub in_query: bool,
}

#[derive(Default)]
pub struct AuthFormValues {
    pub auth_type: Option<AuthType>,
    pub bearer: String,
    pub basic_user: String,
    pub basic_pass: String,
    pub apikey_name: String,
    pub apikey_value: String,
    pub in_query: bool,
}

pub fn derive_auth_form(rows: &[KeyValueRow]) -> AuthFormValues {
    let Some(row) = rows.iter().find(|r| !r.key.trim().is_empty()) else {
        return AuthFormValues {
            auth_type: Some(AuthType::None),
            ..Default::default()
        };
    };
    let key = row.key.trim();
    let value = row.value.trim();
    if key.eq_ignore_ascii_case("authorization") {
        if let Some(token) = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
        {
            return AuthFormValues {
                auth_type: Some(AuthType::BearerToken),
                bearer: token.trim().to_string(),
                ..Default::default()
            };
        }
        if let Some(encoded) = value
            .strip_prefix("Basic ")
            .or_else(|| value.strip_prefix("basic "))
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
    AuthFormValues {
        auth_type: Some(AuthType::ApiKey),
        apikey_name: key.to_string(),
        apikey_value: value.to_string(),
        in_query: row.description.trim().eq_ignore_ascii_case("query"),
        ..Default::default()
    }
}

pub fn kv_input(
    window: &mut Window,
    cx: &mut App,
    value: &str,
    placeholder: &str,
) -> Entity<InputState> {
    input_state(window, cx, value, placeholder, false, false)
}

pub fn single_input(
    window: &mut Window,
    cx: &mut App,
    value: &str,
    placeholder: &str,
) -> Entity<InputState> {
    input_state(window, cx, value, placeholder, false, false)
}

pub fn masked_single_input(
    window: &mut Window,
    cx: &mut App,
    value: &str,
    placeholder: &str,
) -> Entity<InputState> {
    input_state(window, cx, value, placeholder, false, true)
}

pub fn multiline_input(
    window: &mut Window,
    cx: &mut App,
    value: &str,
    placeholder: &str,
) -> Entity<InputState> {
    input_state(window, cx, value, placeholder, true, false)
}

fn input_state(
    window: &mut Window,
    cx: &mut App,
    value: &str,
    placeholder: &str,
    multiline: bool,
    masked: bool,
) -> Entity<InputState> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        let mut input = if multiline {
            InputState::new(window, cx)
                .multi_line(true)
                .searchable(true)
                .soft_wrap(true)
        } else {
            InputState::new(window, cx)
        };
        input.set_placeholder(placeholder.clone(), window, cx);
        input.reset_value(value.clone(), cx);
        if masked {
            input.set_masked(true, window, cx);
        }
        input
    })
}

pub fn request_at(groups: &[ApiGroup], index: usize) -> Option<&ApiRequest> {
    fn find<'a>(
        groups: &'a [ApiGroup],
        target: usize,
        offset: &mut usize,
    ) -> Option<&'a ApiRequest> {
        for group in groups {
            if target < *offset + group.requests.len() {
                return group.requests.get(target - *offset);
            }
            *offset += group.requests.len();
            if let Some(r) = find(&group.folders, target, offset) {
                return Some(r);
            }
        }
        None
    }
    find(groups, index, &mut 0)
}

pub fn request_at_mut(groups: &mut [ApiGroup], index: usize) -> Option<&mut ApiRequest> {
    fn find<'a>(
        groups: &'a mut [ApiGroup],
        target: usize,
        offset: &mut usize,
    ) -> Option<&'a mut ApiRequest> {
        for group in groups.iter_mut() {
            if target < *offset + group.requests.len() {
                return group.requests.get_mut(target - *offset);
            }
            *offset += group.requests.len();
            if let Some(r) = find(&mut group.folders, target, offset) {
                return Some(r);
            }
        }
        None
    }
    find(groups, index, &mut 0)
}

pub fn parse_rows(text: &str) -> Vec<KeyValueRow> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (enabled, content) = match line.strip_prefix('#') {
                Some(rest) => (false, rest.trim()),
                None => (true, line),
            };
            let mut parts = content.splitn(3, '\t');
            let pair = parts.next().unwrap_or_default().trim();
            let value_type = parts.next().unwrap_or_default().trim();
            let description = parts.next().unwrap_or_default().trim();
            let (key, value) = pair
                .split_once('=')
                .map(|(key, value)| (key.trim(), value.trim()))
                .unwrap_or((pair, ""));
            KeyValueRow {
                enabled,
                key: key.to_string(),
                value: value.to_string(),
                value_type: value_type.to_string(),
                description: description.to_string(),
            }
        })
        .collect()
}

pub fn format_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .map(|row| {
            let mut body = format!("{}={}", row.key, row.value);
            let value_type = sanitize_row_metadata(&row.value_type);
            let description = sanitize_row_metadata(&row.description);
            if !value_type.is_empty() || !description.is_empty() {
                body.push('\t');
                body.push_str(&value_type);
                body.push('\t');
                body.push_str(&description);
            }
            if row.enabled {
                body
            } else {
                format!("# {body}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize_row_metadata(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ").trim().to_string()
}

pub fn detect_body_mode(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "none".to_string();
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return "json".to_string();
    }
    "text".to_string()
}

pub fn content_type_extension(content_type: &str) -> &'static str {
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

pub fn is_binary_content_type(content_type: &str) -> bool {
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

pub fn sample_response() -> ApiResponse {
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
        body_bytes: None,
    }
}

#[cfg(test)]
mod tests {
    use gpui::{App, TestAppContext};

    use super::*;

    fn build_auth_probe(
        bearer_val: &str,
        user_val: &str,
        pass_val: &str,
        name_val: &str,
        value_val: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> AuthInputProbe {
        AuthInputProbe {
            bearer: masked_single_input(window, cx, bearer_val, "Token"),
            basic_user: single_input(window, cx, user_val, "用户名"),
            basic_pass: masked_single_input(window, cx, pass_val, "密码"),
            apikey_name: single_input(window, cx, name_val, "Key"),
            apikey_value: masked_single_input(window, cx, value_val, "Value"),
        }
    }

    struct AuthInputProbe {
        bearer: Entity<InputState>,
        basic_user: Entity<InputState>,
        basic_pass: Entity<InputState>,
        apikey_name: Entity<InputState>,
        apikey_value: Entity<InputState>,
    }

    #[gpui::test]
    fn auth_secret_fields_are_masked(cx: &mut TestAppContext) {
        let window_cx = cx.add_empty_window();
        let probe = window_cx.update(|window, cx| {
            build_auth_probe(
                "tok-secret",
                "alice",
                "pw-secret",
                "X-API-Key",
                "val-secret",
                window,
                cx,
            )
        });

        window_cx.read(|cx| {
            assert!(
                probe.bearer.read(cx).is_masked(),
                "bearer token must be masked"
            );
            assert!(
                !probe.basic_user.read(cx).is_masked(),
                "basic user must remain plaintext"
            );
            assert!(
                probe.basic_pass.read(cx).is_masked(),
                "basic password must be masked"
            );
            assert!(
                !probe.apikey_name.read(cx).is_masked(),
                "apikey name must remain plaintext"
            );
            assert!(
                probe.apikey_value.read(cx).is_masked(),
                "apikey value must be masked"
            );
        });
    }

    #[gpui::test]
    fn masked_fields_preserve_value(cx: &mut TestAppContext) {
        let window_cx = cx.add_empty_window();
        let probe = window_cx.update(|window, cx| {
            build_auth_probe("tok-123", "bob", "pw-456", "X-Key", "val-789", window, cx)
        });

        window_cx.read(|cx| {
            assert_eq!(probe.bearer.read(cx).value().as_ref(), "tok-123");
            assert_eq!(probe.basic_pass.read(cx).value().as_ref(), "pw-456");
            assert_eq!(probe.apikey_value.read(cx).value().as_ref(), "val-789");
        });
    }

    #[gpui::test]
    fn masked_state_survives_reset_value(cx: &mut TestAppContext) {
        let window_cx = cx.add_empty_window();
        let probe = window_cx
            .update(|window, cx| build_auth_probe("tok-a", "u", "p", "k", "v", window, cx));

        let bearer = probe.bearer.clone();
        let pass = probe.basic_pass.clone();
        let value = probe.apikey_value.clone();

        window_cx.update(|_window, cx| {
            bearer.update(cx, |input, cx| input.reset_value("tok-new", cx));
            pass.update(cx, |input, cx| input.reset_value("pw-new", cx));
            value.update(cx, |input, cx| input.reset_value("val-new", cx));
        });

        window_cx.read(|cx| {
            assert!(
                bearer.read(cx).is_masked(),
                "bearer stays masked after reset"
            );
            assert_eq!(bearer.read(cx).value().as_ref(), "tok-new");
            assert!(
                pass.read(cx).is_masked(),
                "basic pass stays masked after reset"
            );
            assert_eq!(pass.read(cx).value().as_ref(), "pw-new");
            assert!(
                value.read(cx).is_masked(),
                "apikey value stays masked after reset"
            );
            assert_eq!(value.read(cx).value().as_ref(), "val-new");
        });
    }
}
