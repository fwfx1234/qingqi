use gpui::{SharedString, Styled, ParentElement, IntoElement};
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

impl gpui_component::select::SelectItem for HttpMethod {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioStatus {
    Passed,
    Pending,
    Failed,
}

impl ScenarioStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Passed => "已通过",
            Self::Pending => "待执行",
            Self::Failed => "失败",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Passed => "✓",
            Self::Pending => "⏳",
            Self::Failed => "✗",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyValueRow {
    pub enabled: bool,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub value_type: String,
    #[serde(default)]
    pub description: String,
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
    pub status: ScenarioStatus,
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
    pub headers: Vec<KeyValueRow>,
    pub cookies: Vec<KeyValueRow>,
    pub auth: Vec<KeyValueRow>,
    pub pre_ops: String,
    pub post_ops: String,
    pub scenarios: Vec<ApiScenario>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiGroup {
    pub id: Option<String>,
    pub name: String,
    pub requests: Vec<ApiRequest>,
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
    pub request_json: String,
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

// ── Tabs ──

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpTab {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub request_mode: String,
    pub body_mode: String,
    pub auth_type: String,
    pub auth_value: String,
    pub headers_text: String,
    pub cookies_text: String,
    pub body_text: String,
    pub params_text: String,
    pub path_params_text: String,
    pub pre_ops_text: String,
    pub post_ops_text: String,
    pub node_id: String,
    pub active_request_tab: i64,
    pub updated_at: String,
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

// ── Request snapshot (for collection node request_json) ──

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RequestSnapshot {
    pub method: String,
    pub url: String,
    pub params_text: String,
    pub path_params_text: String,
    pub headers_text: String,
    pub cookies_text: String,
    pub body_text: String,
    pub body_mode: String,
    pub auth_type: String,
    pub auth_value: String,
    pub pre_ops_text: String,
    pub post_ops_text: String,
}

impl RequestSnapshot {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| String::from("{}"))
    }

    pub fn from_json(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
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
}
