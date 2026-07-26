use gpui::{IntoElement, ParentElement, SharedString, Styled};
use serde::{Deserialize, Serialize};

// ── Shared service/request types ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            Self::Get => 0x338855,
            Self::Post => 0x336699,
            Self::Put => 0x7b5fff,
            Self::Patch => 0x997733,
            Self::Delete => 0x994444,
            Self::Head => 0x557788,
            Self::Options => 0x6b5b95,
        }
    }

    /// Whether a request body should be sent for this method.
    pub fn allows_body(&self) -> bool {
        !matches!(self, Self::Get | Self::Head)
    }

    pub fn all() -> [Self; 7] {
        [
            Self::Get,
            Self::Post,
            Self::Put,
            Self::Patch,
            Self::Delete,
            Self::Head,
            Self::Options,
        ]
    }
}

impl qingqi_ui::components::widgets::SelectItem for HttpMethod {
    type Value = HttpMethod;

    fn title(&self) -> SharedString {
        SharedString::from(self.label())
    }

    fn display_title(&self) -> Option<gpui::AnyElement> {
        Some(
            gpui::div()
                .font_family("SF Mono")
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(gpui::rgb(self.color()))
                .child(self.label())
                .into_any_element(),
        )
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BodyMode {
    #[default]
    None,
    Json,
    Text,
    Xml,
    FormUrlEncoded,
    FormData,
    Binary,
}

impl BodyMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Json => "JSON",
            Self::Text => "Text",
            Self::Xml => "XML",
            Self::FormUrlEncoded => "x-www-form",
            Self::FormData => "Form Data",
            Self::Binary => "Binary",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Json => "json",
            Self::Text => "text",
            Self::Xml => "xml",
            Self::FormUrlEncoded => "urlencoded",
            Self::FormData => "formdata",
            Self::Binary => "binary",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "json" => Self::Json,
            "text" => Self::Text,
            "xml" => Self::Xml,
            "urlencoded" | "form-url-encoded" => Self::FormUrlEncoded,
            "formdata" | "form-data" => Self::FormData,
            "binary" => Self::Binary,
            _ => Self::None,
        }
    }

    pub fn all() -> [BodyMode; 7] {
        [
            Self::None,
            Self::Json,
            Self::Text,
            Self::Xml,
            Self::FormUrlEncoded,
            Self::FormData,
            Self::Binary,
        ]
    }
}

impl std::fmt::Display for BodyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthType {
    None,
    BearerToken,
    BasicAuth,
    ApiKey,
}

impl AuthType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::BearerToken => "Bearer",
            Self::BasicAuth => "Basic",
            Self::ApiKey => "API Key",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BearerToken => "bearer",
            Self::BasicAuth => "basic",
            Self::ApiKey => "apikey",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bearer" => Self::BearerToken,
            "basic" => Self::BasicAuth,
            "apikey" => Self::ApiKey,
            _ => Self::None,
        }
    }

    pub fn all() -> [AuthType; 4] {
        [Self::None, Self::BearerToken, Self::BasicAuth, Self::ApiKey]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyValueRow {
    pub enabled: bool,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub value_type: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RequestBody {
    #[serde(default)]
    pub json: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub xml: String,
    #[serde(default)]
    pub urlencoded: Vec<KeyValueRow>,
    #[serde(default)]
    pub form_data: Vec<KeyValueRow>,
    #[serde(default)]
    pub binary_path: String,
}

impl RequestBody {
    pub fn is_empty(&self) -> bool {
        self.json.is_empty()
            && self.text.is_empty()
            && self.xml.is_empty()
            && self.urlencoded.is_empty()
            && self.form_data.is_empty()
            && self.binary_path.is_empty()
    }

    pub fn raw(&self, mode: BodyMode) -> &str {
        match mode {
            BodyMode::Json => &self.json,
            BodyMode::Text => &self.text,
            BodyMode::Xml => &self.xml,
            BodyMode::Binary => &self.binary_path,
            BodyMode::None | BodyMode::FormUrlEncoded | BodyMode::FormData => "",
        }
    }

    pub fn raw_mut(&mut self, mode: BodyMode) -> Option<&mut String> {
        match mode {
            BodyMode::Json => Some(&mut self.json),
            BodyMode::Text => Some(&mut self.text),
            BodyMode::Xml => Some(&mut self.xml),
            BodyMode::Binary => Some(&mut self.binary_path),
            BodyMode::None | BodyMode::FormUrlEncoded | BodyMode::FormData => None,
        }
    }

    pub fn rows(&self, mode: BodyMode) -> &[KeyValueRow] {
        match mode {
            BodyMode::FormUrlEncoded => &self.urlencoded,
            BodyMode::FormData => &self.form_data,
            _ => &[],
        }
    }

    pub fn rows_mut(&mut self, mode: BodyMode) -> Option<&mut Vec<KeyValueRow>> {
        match mode {
            BodyMode::FormUrlEncoded => Some(&mut self.urlencoded),
            BodyMode::FormData => Some(&mut self.form_data),
            _ => None,
        }
    }

    pub fn migrate_legacy(mode: BodyMode, body: &str) -> Self {
        let mut payloads = Self::default();
        match mode {
            BodyMode::Json => payloads.json = body.to_string(),
            BodyMode::Text => payloads.text = body.to_string(),
            BodyMode::Xml => payloads.xml = body.to_string(),
            BodyMode::Binary => payloads.binary_path = body.to_string(),
            BodyMode::FormUrlEncoded => payloads.urlencoded = parse_legacy_body_rows(body, false),
            BodyMode::FormData => payloads.form_data = parse_legacy_body_rows(body, true),
            BodyMode::None => {}
        }
        payloads
    }
}

fn parse_legacy_body_rows(text: &str, detect_files: bool) -> Vec<KeyValueRow> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (enabled, pair) = line
                .strip_prefix('#')
                .map(|rest| (false, rest.trim()))
                .unwrap_or((true, line));
            let (key, value) = pair
                .split_once('=')
                .map(|(key, value)| (key.trim(), value.trim()))
                .unwrap_or((pair, ""));
            let mut row = KeyValueRow::new(key, value.trim_start_matches('@'));
            row.enabled = enabled;
            if detect_files && value.starts_with('@') {
                row.value_type = String::from("file");
            } else if detect_files {
                row.value_type = String::from("text");
            }
            row
        })
        .collect()
}

impl KeyValueRow {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            enabled: true,
            key: key.into(),
            value: value.into(),
            value_type: String::new(),
            description: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiScenario {
    #[serde(default)]
    pub node_id: String,
    pub name: String,
    #[serde(default)]
    pub request: Option<Box<ApiRequest>>,
}

// ── Script management ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptCategory {
    PreRequest,
    PostRequest,
    Common,
}

impl ScriptCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreRequest => "pre",
            Self::PostRequest => "post",
            Self::Common => "common",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "pre" => Self::PreRequest,
            "post" => Self::PostRequest,
            _ => Self::Common,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Script {
    pub id: String,
    pub name: String,
    pub category: ScriptCategory,
    pub content: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiRequest {
    pub node_id: String,
    pub title: String,
    pub method: HttpMethod,
    pub path: String,
    pub params: Vec<KeyValueRow>,
    pub path_rows: Vec<KeyValueRow>,
    pub body: String,
    #[serde(default)]
    pub body_mode: BodyMode,
    #[serde(default)]
    pub body_payloads: RequestBody,
    #[serde(default = "default_editor_tab")]
    pub editor_tab: String,
    pub headers: Vec<KeyValueRow>,
    pub cookies: Vec<KeyValueRow>,
    pub auth: Vec<KeyValueRow>,
    pub pre_ops: String,
    pub post_ops: String,
    pub scenarios: Vec<ApiScenario>,
}

impl ApiRequest {
    pub fn ensure_body_payloads(&mut self) {
        if self.body_payloads.is_empty() && !self.body.is_empty() {
            self.body_payloads = RequestBody::migrate_legacy(self.body_mode, &self.body);
        }
        self.body = self.active_body_text();
    }

    pub fn active_body_text(&self) -> String {
        if self.body_payloads.is_empty() {
            return self.body.clone();
        }
        match self.body_mode {
            BodyMode::FormUrlEncoded => format_body_rows(&self.body_payloads.urlencoded, false),
            BodyMode::FormData => format_body_rows(&self.body_payloads.form_data, true),
            BodyMode::None => String::new(),
            mode => self.body_payloads.raw(mode).to_string(),
        }
    }
}

fn format_body_rows(rows: &[KeyValueRow], file_marker: bool) -> String {
    rows.iter()
        .filter(|row| !row.key.trim().is_empty())
        .map(|row| {
            let value = if file_marker && row.value_type.eq_ignore_ascii_case("file") {
                format!("@{}", row.value)
            } else {
                row.value.clone()
            };
            let pair = format!("{}={value}", row.key);
            if row.enabled { pair } else { format!("# {pair}") }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn default_editor_tab() -> String {
    String::from("params")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiGroup {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub folders: Vec<ApiGroup>,
    pub requests: Vec<ApiRequest>,
}

impl ApiGroup {
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty() && self.folders.iter().all(|f| f.is_empty())
    }

    pub fn total_request_count(&self) -> usize {
        self.requests.len()
            + self
                .folders
                .iter()
                .map(|f| f.total_request_count())
                .sum::<usize>()
    }

    pub fn any_scenario_exists(&self, predicate: impl Fn(&ApiScenario) -> bool + Copy) -> bool {
        if self
            .requests
            .iter()
            .any(|r| r.scenarios.iter().any(predicate))
        {
            return true;
        }
        self.folders
            .iter()
            .any(|f| f.any_scenario_exists(predicate))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiEnvironment {
    pub name: String,
    pub badge: String,
    pub color: u32,
    pub base_url: String,
    pub variables: Vec<KeyValueRow>,
    pub headers: Vec<KeyValueRow>,
}

// ── Collection tree ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Folder,
    Endpoint,
    Case,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Endpoint => "endpoint",
            Self::Case => "case",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "endpoint" => Self::Endpoint,
            "case" => Self::Case,
            _ => Self::Folder,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: NodeKind,
    pub name: String,
    pub method: String,
    pub url: String,
    pub request: RequestSnapshot,
    pub sort_order: i64,
    pub expanded: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ── Environment ──

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvVariable {
    pub id: i64,
    pub environment_id: String,
    pub enabled: bool,
    pub var_key: String,
    pub var_value: String,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvHeader {
    pub id: i64,
    pub environment_id: String,
    pub enabled: bool,
    pub header_key: String,
    pub header_value: String,
    pub sort_order: i64,
}

// ── History ──

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpHistory {
    pub id: i64,
    pub tab_id: String,
    pub method: String,
    pub url: String,
    pub status: i64,
    pub title: String,
    pub response: String,
    pub created_at: String,
}

// ── Scoped variables ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableScope {
    Global,
    Environment,
    Module,
}

impl VariableScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Environment => "environment",
            Self::Module => "module",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "environment" => Self::Environment,
            "module" => Self::Module,
            _ => Self::Global,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiVariable {
    pub scope: VariableScope,
    pub env_name: String,
    pub var_key: String,
    pub var_value: String,
    pub updated_at: String,
}

// ── Full environment with children ──

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentFull {
    pub env: Environment,
    pub variables: Vec<EnvVariable>,
    pub headers: Vec<EnvHeader>,
}

// ── Request snapshot (persisted as collection node request_json) ──

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct RequestSnapshot {
    pub method: String,
    pub url: String,
    pub params_text: String,
    pub path_params_text: String,
    pub headers_text: String,
    pub cookies_text: String,
    pub body_text: String,
    pub body_mode: String,
    #[serde(default)]
    pub body_payloads: RequestBody,
    #[serde(default = "default_editor_tab")]
    pub editor_tab: String,
    pub auth_type: String,
    pub auth_value: String,
    pub pre_ops_text: String,
    pub post_ops_text: String,
}

impl RequestSnapshot {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| String::from("{}"))
    }

    pub fn from_json(json: &str) -> Self {
        let mut snapshot: Self = serde_json::from_str(json).unwrap_or_default();
        if snapshot.body_payloads.is_empty() && !snapshot.body_text.is_empty() {
            snapshot.body_payloads = RequestBody::migrate_legacy(
                BodyMode::from_db(&snapshot.body_mode),
                &snapshot.body_text,
            );
        }
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_roundtrip() {
        for kind in [NodeKind::Folder, NodeKind::Endpoint, NodeKind::Case] {
            let s = kind.as_str();
            assert_eq!(NodeKind::from_db(s), kind);
        }
    }

    #[test]
    fn variable_scope_roundtrip() {
        for scope in [
            VariableScope::Global,
            VariableScope::Environment,
            VariableScope::Module,
        ] {
            let s = scope.as_str();
            assert_eq!(VariableScope::from_db(s), scope);
        }
    }

    #[test]
    fn request_snapshot_json_roundtrip() {
        let snap = RequestSnapshot {
            method: "POST".into(),
            url: "/api/test".into(),
            body_text: r#"{"key": "value"}"#.into(),
            ..Default::default()
        };
        let json = snap.to_json();
        let restored = RequestSnapshot::from_json(&json);
        assert_eq!(restored.method, "POST");
        assert_eq!(restored.url, "/api/test");
        assert_eq!(restored.body_text, r#"{"key": "value"}"#);
    }

    #[test]
    fn legacy_snapshot_migrates_body_to_active_payload() {
        let restored = RequestSnapshot::from_json(
            r#"{"body_text":"upload=@/tmp/a.png\ntitle=demo","body_mode":"form-data"}"#,
        );
        assert_eq!(restored.body_payloads.form_data.len(), 2);
        assert_eq!(restored.body_payloads.form_data[0].value, "/tmp/a.png");
        assert_eq!(restored.body_payloads.form_data[0].value_type, "file");
        assert_eq!(restored.editor_tab, "params");
    }

    #[test]
    fn request_body_preserves_every_mode() {
        let payloads = RequestBody {
            json: String::from("{\"ok\":true}"),
            text: String::from("plain"),
            xml: String::from("<ok/>") ,
            urlencoded: vec![KeyValueRow::new("a", "1")],
            form_data: vec![KeyValueRow::new("file", "/tmp/a")],
            binary_path: String::from("/tmp/raw.bin"),
        };
        let json = serde_json::to_string(&payloads).unwrap();
        let restored: RequestBody = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, payloads);
    }

    #[test]
    fn request_snapshot_empty_detection() {
        assert!(RequestSnapshot::default().is_empty());
        assert!(
            !RequestSnapshot {
                method: "GET".into(),
                ..Default::default()
            }
            .is_empty()
        );
    }

    #[test]
    fn body_mode_roundtrip() {
        for mode in BodyMode::all() {
            let s = mode.as_str();
            assert_eq!(BodyMode::from_db(s), mode);
        }
    }

    #[test]
    fn auth_type_roundtrip() {
        for auth in AuthType::all() {
            let s = auth.as_str();
            assert_eq!(AuthType::from_db(s), auth);
        }
    }

    #[test]
    fn body_mode_switch_preserves_other_modes() {
        let mut payloads = RequestBody::default();
        payloads.json = String::from(r#"{"key":"value"}"#);
        payloads.text = String::from("plain text");
        payloads.xml = String::from("<root/>");
        payloads.binary_path = String::from("/tmp/file.bin");
        payloads.urlencoded = vec![KeyValueRow::new("a", "1")];
        payloads.form_data = vec![KeyValueRow::new("file", "/tmp/a.png")];

        // Simulate switching modes: each mode should only access its own content
        assert_eq!(payloads.raw(BodyMode::Json), r#"{"key":"value"}"#);
        assert_eq!(payloads.raw(BodyMode::Text), "plain text");
        assert_eq!(payloads.raw(BodyMode::Xml), "<root/>");
        assert_eq!(payloads.raw(BodyMode::Binary), "/tmp/file.bin");
        assert!(payloads.raw(BodyMode::FormUrlEncoded).is_empty());
        assert!(payloads.raw(BodyMode::FormData).is_empty());
        assert!(payloads.raw(BodyMode::None).is_empty());

        // Verify rows are independent
        assert_eq!(payloads.rows(BodyMode::FormUrlEncoded).len(), 1);
        assert_eq!(payloads.rows(BodyMode::FormData).len(), 1);

        // Mutating one mode does not affect others
        payloads.json = String::from(r#"{"updated":true}"#);
        assert_eq!(payloads.raw(BodyMode::Text), "plain text");
        assert_eq!(payloads.raw(BodyMode::Xml), "<root/>");
    }

    #[test]
    fn form_data_roundtrip_with_type_and_description() {
        let rows = vec![
            KeyValueRow {
                enabled: true,
                key: "file".into(),
                value: "/tmp/upload.png".into(),
                value_type: "file".into(),
                description: "头像图片".into(),
            },
            KeyValueRow {
                enabled: true,
                key: "title".into(),
                value: "demo".into(),
                value_type: "text".into(),
                description: "标题".into(),
            },
            KeyValueRow {
                enabled: false,
                key: "skip".into(),
                value: "ignored".into(),
                value_type: "text".into(),
                description: "disabled row".into(),
            },
        ];

        let payloads = RequestBody {
            form_data: rows.clone(),
            ..Default::default()
        };

        let json = serde_json::to_string(&payloads).unwrap();
        let restored: RequestBody = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.form_data.len(), 3);
        assert_eq!(restored.form_data[0].value_type, "file");
        assert_eq!(restored.form_data[0].description, "头像图片");
        assert_eq!(restored.form_data[1].value_type, "text");
        assert_eq!(restored.form_data[1].description, "标题");
        assert!(!restored.form_data[2].enabled);
        assert_eq!(restored.form_data[2].description, "disabled row");
    }

    #[test]
    fn urlencoded_roundtrip_with_description() {
        let rows = vec![
            KeyValueRow {
                enabled: true,
                key: "username".into(),
                value: "admin".into(),
                value_type: String::new(),
                description: "登录名".into(),
            },
            KeyValueRow {
                enabled: false,
                key: "debug".into(),
                value: "1".into(),
                value_type: String::new(),
                description: "debug mode".into(),
            },
        ];

        let payloads = RequestBody {
            urlencoded: rows,
            ..Default::default()
        };

        let json = serde_json::to_string(&payloads).unwrap();
        let restored: RequestBody = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.urlencoded.len(), 2);
        assert_eq!(restored.urlencoded[0].description, "登录名");
        assert!(!restored.urlencoded[1].enabled);
    }

    #[test]
    fn active_body_text_returns_only_active_mode() {
        let mut request = ApiRequest {
            node_id: String::new(),
            title: String::from("test"),
            method: HttpMethod::Post,
            path: String::from("/api/test"),
            params: Vec::new(),
            path_rows: Vec::new(),
            body: String::new(),
            body_mode: BodyMode::Json,
            body_payloads: RequestBody {
                json: r#"{"active":true}"#.into(),
                text: "inactive text".into(),
                xml: "<inactive/>".into(),
                ..Default::default()
            },
            editor_tab: String::from("body"),
            headers: Vec::new(),
            cookies: Vec::new(),
            auth: Vec::new(),
            pre_ops: String::new(),
            post_ops: String::new(),
            scenarios: Vec::new(),
        };

        // JSON mode should return JSON content
        request.body_mode = BodyMode::Json;
        assert_eq!(request.active_body_text(), r#"{"active":true}"#);

        // Text mode should return text content
        request.body_mode = BodyMode::Text;
        assert_eq!(request.active_body_text(), "inactive text");

        // XML mode should return XML content
        request.body_mode = BodyMode::Xml;
        assert_eq!(request.active_body_text(), "<inactive/>");

        // None mode should return empty
        request.body_mode = BodyMode::None;
        assert!(request.active_body_text().is_empty());
    }

    #[test]
    fn legacy_snapshot_with_form_data_migrates_to_structured() {
        let restored = RequestSnapshot::from_json(
            r#"{"body_text":"file=@/tmp/a.png\ntitle=demo","body_mode":"form-data"}"#,
        );
        assert_eq!(restored.body_payloads.form_data.len(), 2);
        assert_eq!(restored.body_payloads.form_data[0].key, "file");
        assert_eq!(restored.body_payloads.form_data[0].value, "/tmp/a.png");
        assert_eq!(restored.body_payloads.form_data[0].value_type, "file");
        assert_eq!(restored.body_payloads.form_data[1].key, "title");
        assert_eq!(restored.body_payloads.form_data[1].value, "demo");
        assert_eq!(restored.body_payloads.form_data[1].value_type, "text");
    }

    #[test]
    fn legacy_snapshot_with_urlencoded_migrates_to_structured() {
        let restored = RequestSnapshot::from_json(
            r#"{"body_text":"a=1\nb=2\n# c=3","body_mode":"urlencoded"}"#,
        );
        assert_eq!(restored.body_payloads.urlencoded.len(), 3);
        assert_eq!(restored.body_payloads.urlencoded[0].key, "a");
        assert_eq!(restored.body_payloads.urlencoded[0].value, "1");
        assert!(!restored.body_payloads.urlencoded[2].enabled);
    }
}
