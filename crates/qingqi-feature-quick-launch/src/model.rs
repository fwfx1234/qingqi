use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::parameters::{ParameterSpec, extract_parameters};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    Script,
    OpenPath,
    OpenUrl,
}

impl ActionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Script => "script",
            Self::OpenPath => "open_path",
            Self::OpenUrl => "open_url",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "open_path" => Self::OpenPath,
            "open_url" => Self::OpenUrl,
            _ => Self::Script,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Script => "脚本",
            Self::OpenPath => "路径",
            Self::OpenUrl => "URL",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackMode {
    Silent,
    Popup,
    Notification,
}

impl FeedbackMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Silent => "silent",
            Self::Popup => "popup",
            Self::Notification => "notification",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "silent" => Self::Silent,
            "popup" => Self::Popup,
            _ => Self::Notification,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptType {
    Shell,
    Node,
    Python,
    Other,
}

impl ScriptType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Node => "node",
            Self::Python => "python",
            Self::Other => "other",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "node" => Self::Node,
            "python" => Self::Python,
            "other" => Self::Other,
            _ => Self::Shell,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::Node => "Node",
            Self::Python => "Python",
            Self::Other => "其他",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptSource {
    Path,
    Inline,
}

impl ScriptSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Inline => "inline",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "inline" => Self::Inline,
            _ => Self::Path,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Path => "文件",
            Self::Inline => "内联",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Success,
    Failed,
    Timeout,
    Stopped,
    Error,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "failed" => Self::Failed,
            "timeout" => Self::Timeout,
            "stopped" => Self::Stopped,
            "error" => Self::Error,
            _ => Self::Success,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAction {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub kind: ActionKind,
    pub script_type: ScriptType,
    pub script_source: ScriptSource,
    pub script_body: String,
    pub interpreter: String,
    pub path: String,
    pub url: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub keywords: Vec<String>,
    pub prefixes: Vec<String>,
    pub icon: String,
    pub feedback_mode: FeedbackMode,
    pub timeout_sec: i64,
    pub enabled: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl QuickAction {
    pub fn command_keywords(&self) -> Vec<String> {
        let mut keywords = self.keywords.clone();
        keywords.push(self.name.clone());
        keywords.push(self.description.clone());
        keywords
    }

    pub fn parameter_specs(&self) -> Vec<ParameterSpec> {
        extract_parameters(
            [
                self.script_body.as_str(),
                self.interpreter.as_str(),
                self.path.as_str(),
                self.url.as_str(),
                self.cwd.as_str(),
            ]
            .into_iter()
            .chain(self.args.iter().map(String::as_str))
            .chain(self.env.values().map(String::as_str)),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickActionDraft {
    pub name: String,
    pub description: String,
    pub kind: ActionKind,
    pub script_type: ScriptType,
    pub script_source: ScriptSource,
    pub script_body: String,
    pub interpreter: String,
    pub path: String,
    pub url: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub keywords: Vec<String>,
    pub prefixes: Vec<String>,
    pub icon: String,
    pub feedback_mode: FeedbackMode,
    pub timeout_sec: i64,
    pub enabled: bool,
    pub sort_order: Option<i64>,
}

impl QuickActionDraft {
    pub fn script(
        name: impl Into<String>,
        description: impl Into<String>,
        script_body: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            kind: ActionKind::Script,
            script_type: ScriptType::Shell,
            script_source: ScriptSource::Inline,
            script_body: script_body.into(),
            interpreter: String::new(),
            path: String::new(),
            url: String::new(),
            args: Vec::new(),
            cwd: String::new(),
            env: HashMap::new(),
            keywords: Vec::new(),
            prefixes: Vec::new(),
            icon: String::new(),
            feedback_mode: FeedbackMode::Notification,
            timeout_sec: 300,
            enabled: true,
            sort_order: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickRun {
    pub id: i64,
    pub action_id: i64,
    pub status: RunStatus,
    pub exit_code: Option<i64>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: i64,
    pub started_at: String,
    pub finished_at: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickRunDraft {
    pub action_id: i64,
    pub status: RunStatus,
    pub exit_code: Option<i64>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: i64,
    pub started_at: String,
    pub finished_at: String,
    pub message: String,
}
