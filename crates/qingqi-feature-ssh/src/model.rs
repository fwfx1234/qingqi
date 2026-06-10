//! SSH 远程管理插件 — 领域类型
//!
//! 纯数据，无 GPUI 依赖。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============ 协议类型 ============

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolType {
    #[default]
    #[serde(rename = "ssh")]
    Ssh,
    #[serde(rename = "ftp")]
    Ftp,
    #[serde(rename = "ftps")]
    Ftps,
}

impl ProtocolType {
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Ssh => 22,
            Self::Ftp => 21,
            Self::Ftps => 990,
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
            Self::Ftp => "FTP",
            Self::Ftps => "FTPS",
        }
    }

    pub fn supports_terminal(&self) -> TerminalKind {
        match self {
            Self::Ssh => TerminalKind::Shell,
            Self::Ftp | Self::Ftps => TerminalKind::Log,
        }
    }
}

// ============ 认证配置 ============

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthConfig {
    Ssh {
        method: SshAuthMethod,
    },
    Ftp {
        username: String,
        password: String,
    },
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::Ssh {
            method: SshAuthMethod::Password {
                password: String::new(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshAuthMethod {
    Password { password: String },
    PrivateKey { path: String, passphrase: String },
    Agent,
}

// ============ 路径配置 ============

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathConfig {
    pub remote_root: String,
    pub local_root: String,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            remote_root: "~".into(),
            local_root: "~/Downloads".into(),
        }
    }
}

// ============ Profile ============

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub paths: PathConfig,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default)]
pub struct ProfileDraft {
    pub name: String,
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub paths: PathConfig,
    pub note: String,
}

// ============ Session ============

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalKind {
    /// SSH: 交互式 PTY
    Shell,
    /// FTP: 命令响应日志
    Log,
}

#[derive(Clone, Debug)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub profile_id: i64,
    pub title: String,
    pub endpoint: String,
    pub protocol: ProtocolType,
    pub status: SessionStatus,
    pub terminal_kind: TerminalKind,
    pub has_terminal: bool,
    pub message: String,
}

// ============ 文件系统 ============

#[derive(Clone, Debug)]
pub struct RemoteEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_at: String,
}

// ============ 传输 ============

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransferId(pub Uuid);

impl TransferId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct TransferTask {
    pub id: TransferId,
    pub session_id: SessionId,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub local_path: String,
    pub remote_path: String,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub message: String,
    pub logs: Vec<String>,
}

// ============ 快照 ============

#[derive(Clone, Debug)]
pub struct SshSnapshot {
    pub profiles: Vec<Profile>,
    pub sessions: Vec<SessionSummary>,
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    pub summary: SessionSummary,
    pub entries: Vec<RemoteEntry>,
    pub remote_cwd: String,
}
