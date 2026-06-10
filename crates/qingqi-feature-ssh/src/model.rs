//! SSH 远程管理插件 — 领域类型
//!
//! 纯数据，无 GPUI 依赖。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============ 协议类型 ============

/// SSH 连接角色：终端与 SFTP 使用独立 TCP 会话，避免 channel 数据串扰
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SshRole {
    Terminal,
    Sftp,
}

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
        #[serde(default = "default_ssh_username")]
        username: String,
        method: SshAuthMethod,
    },
    Ftp { username: String, password: String },
}

fn default_ssh_username() -> String {
    "root".into()
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::Ssh {
            username: default_ssh_username(),
            method: SshAuthMethod::Password {
                password: String::new(),
            },
        }
    }
}

// ============ 高级配置 ============

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileAdvanced {
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u32,
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval_secs: u32,
}

fn default_connection_timeout() -> u32 {
    30
}

fn default_keepalive_interval() -> u32 {
    60
}

impl Default for ProfileAdvanced {
    fn default() -> Self {
        Self {
            connection_timeout_secs: default_connection_timeout(),
            keepalive_interval_secs: default_keepalive_interval(),
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
    pub advanced: ProfileAdvanced,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProfileDraft {
    pub name: String,
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub paths: PathConfig,
    pub advanced: ProfileAdvanced,
    pub note: String,
}

impl Default for ProfileDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            protocol: ProtocolType::default(),
            host: String::new(),
            port: ProtocolType::default().default_port(),
            auth: AuthConfig::default(),
            paths: PathConfig::default(),
            advanced: ProfileAdvanced::default(),
            note: String::new(),
        }
    }
}

// ============ Session ============

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

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

impl Default for TransferId {
    fn default() -> Self {
        Self::new()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_unique() {
        assert_ne!(SessionId::new(), SessionId::new());
    }

    #[test]
    fn test_transfer_id_unique() {
        assert_ne!(TransferId::new(), TransferId::new());
    }

    #[test]
    fn test_protocol_default_port() {
        assert_eq!(ProtocolType::Ssh.default_port(), 22);
        assert_eq!(ProtocolType::Ftp.default_port(), 21);
        assert_eq!(ProtocolType::Ftps.default_port(), 990);
    }

    #[test]
    fn test_protocol_supports_terminal() {
        assert_eq!(ProtocolType::Ssh.supports_terminal(), TerminalKind::Shell);
        assert_eq!(ProtocolType::Ftp.supports_terminal(), TerminalKind::Log);
    }

    #[test]
    fn test_auth_config_json_roundtrip() {
        let config = AuthConfig::Ssh {
            username: "root".into(),
            method: SshAuthMethod::PrivateKey {
                path: "/tmp/key".into(),
                passphrase: "pw".into(),
            },
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn test_profile_draft_default() {
        let draft = ProfileDraft::default();
        assert_eq!(draft.port, 22); // SSH 默认端口
        assert!(matches!(draft.protocol, ProtocolType::Ssh));
    }
}
