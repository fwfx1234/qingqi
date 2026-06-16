use gpui::{App, AppContext, Entity};
use uuid::Uuid;

use crate::service::{self, ApiGroup, ApiRequest, ApiResponse, AuthType, KeyValueRow};

use qingqi_ui::text_input::{TextInput, TextInputStyle};

#[derive(Clone, Debug)]
pub enum OpenTab {
    Request {
        index: usize,
        tab_id: String,
        node_id: String,
    },
    Scenario {
        request_index: usize,
        scenario_index: usize,
        tab_id: String,
        node_id: String,
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
    pub fn tab_id(&self) -> &str {
        match self {
            Self::Request { tab_id, .. } | Self::Scenario { tab_id, .. } => tab_id,
        }
    }

    pub fn fallback_node_id(&self) -> &str {
        match self {
            Self::Request { node_id, .. } => node_id,
            Self::Scenario { node_id, .. } => node_id,
        }
    }

    pub fn new_request(index: usize) -> Self {
        Self::Request {
            index,
            tab_id: format!("tab-{}", Uuid::new_v4().simple()),
            node_id: String::new(),
        }
    }

    pub fn new_scenario_with_node(
        request_index: usize,
        scenario_index: usize,
        node_id: String,
    ) -> Self {
        Self::Scenario {
            request_index,
            scenario_index,
            tab_id: format!("tab-{}", Uuid::new_v4().simple()),
            node_id,
        }
    }

    pub fn with_id(index: usize, tab_id: String, node_id: String) -> Self {
        Self::Request {
            index,
            tab_id,
            node_id,
        }
    }

    pub fn matches_request_index(&self, request_index: usize) -> bool {
        matches!(self, Self::Request { index, .. } if *index == request_index)
    }

    pub fn matches_scenario_index(&self, request_index: usize, scenario_index: usize) -> bool {
        matches!(
            self,
            Self::Scenario {
                request_index: tab_request_index,
                scenario_index: tab_scenario_index,
                ..
            } if *tab_request_index == request_index && *tab_scenario_index == scenario_index
        )
    }
}

#[derive(Clone)]
pub struct KvRow {
    pub enabled: bool,
    pub key: Entity<TextInput>,
    pub value: Entity<TextInput>,
    pub value_type: Entity<TextInput>,
    pub description: Entity<TextInput>,
}

pub struct KvEditor {
    pub rows: Vec<KvRow>,
}

impl KvEditor {
    pub fn new(cx: &mut App, rows: &[KeyValueRow]) -> Self {
        let mut editor = Self { rows: Vec::new() };
        editor.set_rows(cx, rows);
        editor
    }

    pub fn from_text(cx: &mut App, text: &str) -> Self {
        Self::new(cx, &parse_rows(text))
    }

    pub fn set_rows(&mut self, cx: &mut App, rows: &[KeyValueRow]) {
        self.rows = rows
            .iter()
            .map(|row| KvRow {
                enabled: row.enabled,
                key: kv_input(cx, &row.key, "键"),
                value: kv_input(cx, &row.value, "值"),
                value_type: kv_input(cx, &row.value_type, "string"),
                description: kv_input(cx, &row.description, "说明"),
            })
            .collect();
    }

    pub fn set_from_text(&mut self, cx: &mut App, text: &str) {
        self.set_rows(cx, &parse_rows(text));
    }

    pub fn to_rows(&self, cx: &App) -> Vec<KeyValueRow> {
        self.rows
            .iter()
            .map(|row| KeyValueRow {
                enabled: row.enabled,
                key: row.key.read(cx).text().trim().to_string(),
                value: row.value.read(cx).text().trim().to_string(),
                value_type: row.value_type.read(cx).text().trim().to_string(),
                description: row.description.read(cx).text().trim().to_string(),
            })
            .collect()
    }

    pub fn to_text(&self, cx: &App) -> String {
        format_rows(&self.to_rows(cx))
    }

    pub fn add_row(&mut self, cx: &mut App) {
        self.rows.push(KvRow {
            enabled: true,
            key: kv_input(cx, "", "键"),
            value: kv_input(cx, "", "值"),
            value_type: kv_input(cx, "", "string"),
            description: kv_input(cx, "", "说明"),
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
}

#[derive(Clone)]
pub struct AuthFormInputs {
    pub bearer: Entity<TextInput>,
    pub basic_user: Entity<TextInput>,
    pub basic_pass: Entity<TextInput>,
    pub apikey_name: Entity<TextInput>,
    pub apikey_value: Entity<TextInput>,
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

pub fn kv_input(cx: &mut App, value: &str, placeholder: &str) -> Entity<TextInput> {
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

pub fn single_input(cx: &mut App, value: &str, placeholder: &str) -> Entity<TextInput> {
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

pub fn multiline_input(cx: &mut App, value: &str, placeholder: &str) -> Entity<TextInput> {
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

pub fn request_at(groups: &[ApiGroup], index: usize) -> Option<&ApiRequest> {
    fn find<'a>(groups: &'a [ApiGroup], target: usize, offset: &mut usize) -> Option<&'a ApiRequest> {
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
    fn find<'a>(groups: &'a mut [ApiGroup], target: usize, offset: &mut usize) -> Option<&'a mut ApiRequest> {
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

pub fn find_request_index_by_method_url(
    groups: &[ApiGroup],
    method: &str,
    url: &str,
) -> Option<usize> {
    fn search(groups: &[ApiGroup], method_upper: &str, url: &str, offset: &mut usize) -> Option<usize> {
        for group in groups {
            for (i, req) in group.requests.iter().enumerate() {
                if req.method.label() == method_upper && req.path == url {
                    return Some(*offset + i);
                }
            }
            *offset += group.requests.len();
            if let Some(r) = search(&group.folders, method_upper, url, offset) {
                return Some(r);
            }
        }
        None
    }
    search(groups, &method.to_uppercase(), url, &mut 0)
}

pub fn persisted_tab_to_open_tab(
    groups: &[ApiGroup],
    tab: &crate::model::HttpTab,
) -> Option<OpenTab> {
    fn search(groups: &[ApiGroup], tab: &crate::model::HttpTab, offset: &mut usize) -> Option<OpenTab> {
        for group in groups {
            for (request_offset, request) in group.requests.iter().enumerate() {
                let request_index = *offset + request_offset;
                if request.node_id == tab.node_id {
                    return Some(OpenTab::with_id(
                        request_index,
                        tab.id.clone(),
                        tab.node_id.clone(),
                    ));
                }
                if let Some((scenario_index, scenario)) = request
                    .scenarios
                    .iter()
                    .enumerate()
                    .find(|(_, scenario)| scenario.node_id == tab.node_id)
                {
                    return Some(OpenTab::Scenario {
                        request_index,
                        scenario_index,
                        tab_id: tab.id.clone(),
                        node_id: scenario.node_id.clone(),
                    });
                }
            }
            *offset += group.requests.len();
            if let Some(r) = search(&group.folders, tab, offset) {
                return Some(r);
            }
        }
        None
    }

    if !tab.node_id.is_empty() {
        if let Some(found) = search(groups, tab, &mut 0) {
            return Some(found);
        }
    }

    find_request_index_by_method_url(groups, &tab.method, &tab.url)
        .map(|index| OpenTab::with_id(index, tab.id.clone(), tab.node_id.clone()))
}

pub fn first_non_empty<'a>(primary: &'a str, fallback: &'a str) -> &'a str {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
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
    }
}
