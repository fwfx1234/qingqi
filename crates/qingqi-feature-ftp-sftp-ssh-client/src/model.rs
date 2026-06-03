use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteProtocol {
    Sftp,
    Ftp,
    Ftps,
    Ssh,
}

impl RemoteProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sftp => "sftp",
            Self::Ftp => "ftp",
            Self::Ftps => "ftps",
            Self::Ssh => "ssh",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sftp => "SFTP",
            Self::Ftp => "FTP",
            Self::Ftps => "FTPS",
            Self::Ssh => "SSH",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "ftp" => Self::Ftp,
            "ftps" => Self::Ftps,
            "ssh" => Self::Ssh,
            _ => Self::Sftp,
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Self::Ftp | Self::Ftps => 21,
            Self::Sftp | Self::Ssh => 22,
        }
    }

    pub fn supports_file_browser(self) -> bool {
        matches!(self, Self::Sftp | Self::Ftp | Self::Ftps)
    }

    pub fn supports_terminal(self) -> bool {
        matches!(self, Self::Sftp | Self::Ssh)
    }

    pub fn right_panel_mode(self) -> RightPanelMode {
        match self {
            Self::Sftp | Self::Ssh => RightPanelMode::Terminal,
            Self::Ftp | Self::Ftps => RightPanelMode::FtpLog,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FtpsMode {
    Explicit,
    Implicit,
}

impl FtpsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Implicit => "implicit",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Explicit => "显式 TLS (AUTH TLS)",
            Self::Implicit => "隐式 TLS",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "implicit" => Self::Implicit,
            _ => Self::Explicit,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Password,
    PrivateKey,
    Agent,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::PrivateKey => "private_key",
            Self::Agent => "agent",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Password => "密码",
            Self::PrivateKey => "私钥",
            Self::Agent => "SSH Agent",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "private_key" => Self::PrivateKey,
            "agent" => Self::Agent,
            _ => Self::Password,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProfile {
    pub id: i64,
    pub name: String,
    pub protocol: RemoteProtocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password: String,
    pub private_key_path: String,
    pub private_key_passphrase: String,
    pub remote_dir: String,
    pub local_dir: String,
    pub encoding: String,
    pub passive_mode: bool,
    pub connect_timeout_secs: u16,
    pub jump_enabled: bool,
    pub jump_host: String,
    pub jump_port: u16,
    pub jump_username: String,
    pub jump_password: String,
    pub jump_private_key_path: String,
    pub jump_private_key_passphrase: String,
    pub pinned: bool,
    pub notes: String,
    pub group_id: Option<i64>,
    pub ftps_mode: FtpsMode,
    pub last_used_at: String,
    pub created_at: String,
    pub updated_at: String,
}

impl RemoteProfile {
    pub fn endpoint(&self) -> String {
        format!("{}@{}:{}", self.username, self.host, self.port)
    }

    pub fn protocol_label(&self) -> &'static str {
        self.protocol.label()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteProfileDraft {
    pub name: String,
    pub protocol: RemoteProtocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password: String,
    pub private_key_path: String,
    pub private_key_passphrase: String,
    pub remote_dir: String,
    pub local_dir: String,
    pub encoding: String,
    pub passive_mode: bool,
    pub connect_timeout_secs: u16,
    pub jump_enabled: bool,
    pub jump_host: String,
    pub jump_port: u16,
    pub jump_username: String,
    pub jump_password: String,
    pub jump_private_key_path: String,
    pub jump_private_key_passphrase: String,
    pub pinned: bool,
    pub notes: String,
    pub group_id: Option<i64>,
    pub ftps_mode: FtpsMode,
}

impl RemoteProfileDraft {
    pub fn blank() -> Self {
        Self {
            name: String::new(),
            protocol: RemoteProtocol::Sftp,
            host: String::new(),
            port: 22,
            username: String::new(),
            auth_method: AuthMethod::Password,
            password: String::new(),
            private_key_path: String::new(),
            private_key_passphrase: String::new(),
            remote_dir: String::from("/"),
            local_dir: String::from("~/Downloads"),
            encoding: String::from("utf-8"),
            passive_mode: true,
            connect_timeout_secs: 15,
            jump_enabled: false,
            jump_host: String::new(),
            jump_port: 22,
            jump_username: String::new(),
            jump_password: String::new(),
            jump_private_key_path: String::new(),
            jump_private_key_passphrase: String::new(),
            pinned: false,
            notes: String::new(),
            group_id: None,
            ftps_mode: FtpsMode::Explicit,
        }
    }

    pub fn from_profile(profile: &RemoteProfile) -> Self {
        Self {
            name: profile.name.clone(),
            protocol: profile.protocol,
            host: profile.host.clone(),
            port: profile.port,
            username: profile.username.clone(),
            auth_method: profile.auth_method,
            password: profile.password.clone(),
            private_key_path: profile.private_key_path.clone(),
            private_key_passphrase: profile.private_key_passphrase.clone(),
            remote_dir: profile.remote_dir.clone(),
            local_dir: profile.local_dir.clone(),
            encoding: profile.encoding.clone(),
            passive_mode: profile.passive_mode,
            connect_timeout_secs: profile.connect_timeout_secs,
            jump_enabled: profile.jump_enabled,
            jump_host: profile.jump_host.clone(),
            jump_port: profile.jump_port,
            jump_username: profile.jump_username.clone(),
            jump_password: profile.jump_password.clone(),
            jump_private_key_path: profile.jump_private_key_path.clone(),
            jump_private_key_passphrase: profile.jump_private_key_passphrase.clone(),
            pinned: profile.pinned,
            notes: profile.notes.clone(),
            group_id: profile.group_id,
            ftps_mode: profile.ftps_mode,
        }
    }

    pub fn normalize(mut self) -> Self {
        self.name = self.name.trim().to_string();
        self.host = self.host.trim().to_string();
        self.username = self.username.trim().to_string();
        self.private_key_path = self.private_key_path.trim().to_string();
        if self.name.is_empty() {
            self.name = if self.host.is_empty() {
                String::from("未命名连接")
            } else if self.username.is_empty() {
                self.host.clone()
            } else {
                format!("{}@{}", self.username, self.host)
            };
        }
        self.remote_dir = normalize_remote_path(&self.remote_dir);
        self.local_dir = normalize_local_path(&self.local_dir);
        self.encoding = if self.encoding.trim().is_empty() {
            String::from("utf-8")
        } else {
            self.encoding.trim().to_string()
        };
        if self.port == 0 {
            self.port = self.protocol.default_port();
        }
        if self.connect_timeout_secs == 0 {
            self.connect_timeout_secs = 15;
        }
        if self.jump_port == 0 {
            self.jump_port = 22;
        }
        self
    }

    pub fn demo(index: usize) -> Self {
        match index % 4 {
            1 => Self {
                name: String::from("静态资源仓"),
                protocol: RemoteProtocol::Sftp,
                host: String::from("cdn.internal"),
                port: 2222,
                username: String::from("deploy"),
                auth_method: AuthMethod::PrivateKey,
                password: String::new(),
                private_key_path: String::from("~/.ssh/id_ed25519"),
                private_key_passphrase: String::new(),
                remote_dir: String::from("/srv/assets"),
                local_dir: String::from("~/Downloads"),
                encoding: String::from("utf-8"),
                passive_mode: true,
                connect_timeout_secs: 15,
                jump_enabled: false,
                jump_host: String::new(),
                jump_port: 22,
                jump_username: String::new(),
                jump_password: String::new(),
                jump_private_key_path: String::new(),
                jump_private_key_passphrase: String::new(),
                pinned: false,
                notes: String::from("用于静态资源发布"),
                group_id: None,
                ftps_mode: FtpsMode::Explicit,
            },
            2 => Self {
                name: String::from("旧版迁移机"),
                protocol: RemoteProtocol::Ftp,
                host: String::from("legacy.example.com"),
                port: 21,
                username: String::from("ops"),
                auth_method: AuthMethod::Password,
                password: String::new(),
                private_key_path: String::new(),
                private_key_passphrase: String::new(),
                remote_dir: String::from("/home/ops"),
                local_dir: String::from("~/Downloads"),
                encoding: String::from("utf-8"),
                passive_mode: true,
                connect_timeout_secs: 15,
                jump_enabled: false,
                jump_host: String::new(),
                jump_port: 22,
                jump_username: String::new(),
                jump_password: String::new(),
                jump_private_key_path: String::new(),
                jump_private_key_passphrase: String::new(),
                pinned: false,
                notes: String::from("FTP 后端待接入"),
                group_id: None,
                ftps_mode: FtpsMode::Explicit,
            },
            3 => Self {
                name: String::from("测试环境"),
                protocol: RemoteProtocol::Ssh,
                host: String::from("staging.example.com"),
                port: 22,
                username: String::from("qa"),
                auth_method: AuthMethod::Agent,
                password: String::new(),
                private_key_path: String::new(),
                private_key_passphrase: String::new(),
                remote_dir: String::from("/var/log"),
                local_dir: String::from("~/Downloads"),
                encoding: String::from("utf-8"),
                passive_mode: true,
                connect_timeout_secs: 15,
                jump_enabled: false,
                jump_host: String::new(),
                jump_port: 22,
                jump_username: String::new(),
                jump_password: String::new(),
                jump_private_key_path: String::new(),
                jump_private_key_passphrase: String::new(),
                pinned: false,
                notes: String::from("终端桥接待迁移"),
                group_id: None,
                ftps_mode: FtpsMode::Explicit,
            },
            _ => Self {
                name: String::from("生产服务器"),
                protocol: RemoteProtocol::Sftp,
                host: String::from("prod.example.com"),
                port: 22,
                username: String::from("root"),
                auth_method: AuthMethod::PrivateKey,
                password: String::new(),
                private_key_path: String::from("~/.ssh/id_rsa"),
                private_key_passphrase: String::new(),
                remote_dir: String::from("/etc/nginx"),
                local_dir: String::from("~/Downloads"),
                encoding: String::from("utf-8"),
                passive_mode: true,
                connect_timeout_secs: 15,
                jump_enabled: false,
                jump_host: String::new(),
                jump_port: 22,
                jump_username: String::new(),
                jump_password: String::new(),
                jump_private_key_path: String::new(),
                jump_private_key_passphrase: String::new(),
                pinned: true,
                notes: String::from("示例配置"),
                group_id: None,
                ftps_mode: FtpsMode::Explicit,
            },
        }
    }
}

fn normalize_remote_path(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::from("/");
    }
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn normalize_local_path(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        String::from("~/Downloads")
    } else {
        trimmed.to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Idle,
    Connected,
    Failed,
}

impl ConnectionStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "空闲",
            Self::Connected => "已连接",
            Self::Failed => "连接失败",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanelMode {
    Empty,
    Terminal,
    FtpLog,
}

impl RightPanelMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "工作区",
            Self::Terminal => "SSH 终端",
            Self::FtpLog => "FTP 命令日志",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalStatus {
    Idle,
    Connecting,
    Connected,
    Error,
}

impl TerminalStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "未启动",
            Self::Connecting => "连接中",
            Self::Connected => "已连接",
            Self::Error => "终端异常",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolLogKind {
    Command,
    Response,
    Info,
    Error,
}

impl ProtocolLogKind {
    pub fn marker(self) -> &'static str {
        match self {
            Self::Command => "*cmd*",
            Self::Response => "*resp*",
            Self::Info => "*info*",
            Self::Error => "*error*",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolLogEntry {
    pub kind: ProtocolLogKind,
    pub text: String,
}

impl ProtocolLogEntry {
    pub fn new(kind: ProtocolLogKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }

    pub fn display_text(&self) -> String {
        format!("{} {}", self.kind.marker(), self.text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteEditState {
    Synced,
    ModifiedLocal,
    UploadingBack,
    ConflictRisk,
    UploadFailed,
}

impl RemoteEditState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Synced => "已同步",
            Self::ModifiedLocal => "待回传",
            Self::UploadingBack => "回传中",
            Self::ConflictRisk => "远程有变更风险",
            Self::UploadFailed => "回传失败",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEditDraft {
    pub id: String,
    pub profile_id: i64,
    pub file_name: String,
    pub remote_path: String,
    pub local_cache_path: String,
    pub remote_version_hint: String,
    pub last_local_modified_at: i64,
    pub state: RemoteEditState,
    pub message: String,
}

impl RemoteEditDraft {
    pub fn is_dirty(&self) -> bool {
        matches!(
            self.state,
            RemoteEditState::ModifiedLocal
                | RemoteEditState::UploadingBack
                | RemoteEditState::ConflictRisk
                | RemoteEditState::UploadFailed
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSnapshot {
    pub status: TerminalStatus,
    pub cwd_hint: String,
    pub lines: Vec<String>,
}

impl TerminalSnapshot {
    pub fn empty() -> Self {
        Self {
            status: TerminalStatus::Idle,
            cwd_hint: String::new(),
            lines: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSummary {
    pub profile_id: i64,
    pub name: String,
    pub protocol: RemoteProtocol,
    pub status: ConnectionStatus,
    pub remote_path: String,
    pub right_panel_mode: RightPanelMode,
    pub active_transfer_count: usize,
    pub dirty_edit_count: usize,
    pub ftp_log_count: usize,
    pub has_session: bool,
    pub last_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionTransferItem {
    pub profile_id: i64,
    pub session_name: String,
    pub item: TransferItem,
}

#[derive(Clone, Debug)]
pub struct RemoteFileItem {
    pub name: String,
    pub path: String,
    pub kind: &'static str,
    pub is_dir: bool,
    pub size: i64,
    pub modified_at: i64,
    pub permissions: String,
    pub meta: String,
    pub selected: bool,
}

impl RemoteFileItem {
    pub fn file(
        name: String,
        path: String,
        size: i64,
        modified_at: i64,
        permissions: String,
    ) -> Self {
        let meta = format!("{} · {}", format_size(size), format_timestamp(modified_at));
        Self {
            name,
            path,
            kind: "文件",
            is_dir: false,
            size,
            modified_at,
            permissions,
            meta,
            selected: false,
        }
    }

    pub fn dir(name: String, path: String, permissions: String) -> Self {
        Self {
            name,
            path,
            kind: "目录",
            is_dir: true,
            size: 0,
            modified_at: 0,
            permissions,
            meta: String::from("目录"),
            selected: false,
        }
    }

    pub fn status(name: String, meta: String) -> Self {
        Self {
            name,
            path: String::new(),
            kind: "状态",
            is_dir: false,
            size: 0,
            modified_at: 0,
            permissions: String::new(),
            meta,
            selected: true,
        }
    }
}

// --- Transfer types ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

impl TransferDirection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Upload => "上传",
            Self::Download => "下载",
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            Self::Upload => "↑",
            Self::Download => "↓",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TransferStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "排队中",
            Self::Running => "传输中",
            Self::Completed => "已完成",
            Self::Failed => "失败",
            Self::Cancelled => "已取消",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferItem {
    pub id: String,
    pub direction: TransferDirection,
    pub name: String,
    pub local_path: String,
    pub remote_path: String,
    pub size: i64,
    pub transferred: i64,
    pub status: TransferStatus,
    pub speed: String,
    pub message: String,
}

impl TransferItem {
    pub fn new(
        id: String,
        direction: TransferDirection,
        local_path: String,
        remote_path: String,
        size: i64,
    ) -> Self {
        let name = Path::new(if direction == TransferDirection::Upload {
            &local_path
        } else {
            &remote_path
        })
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

        Self {
            id,
            direction,
            name,
            local_path,
            remote_path,
            size,
            transferred: 0,
            status: TransferStatus::Queued,
            speed: String::new(),
            message: String::new(),
        }
    }

    pub fn progress_percent(&self) -> u8 {
        if self.size <= 0 {
            return 0;
        }
        ((self.transferred as f64 / self.size as f64) * 100.0).clamp(0.0, 100.0) as u8
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            TransferStatus::Queued | TransferStatus::Running
        )
    }

    /// Rich status line combining progress, size, speed, or message.
    pub fn status_line(&self) -> String {
        match self.status {
            TransferStatus::Queued => format!("排队 · {}", format_size(self.size)),
            TransferStatus::Running => {
                let progress = format_size(self.transferred);
                let total = format_size(self.size);
                let speed = if self.speed.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", self.speed)
                };
                format!("{progress} / {total}{speed}")
            }
            TransferStatus::Completed => format!("已完成 · {}", format_size(self.size)),
            TransferStatus::Failed => {
                if self.message.is_empty() || self.message == "失败" {
                    String::from("失败")
                } else {
                    format!("失败: {}", self.message)
                }
            }
            TransferStatus::Cancelled => String::from("已取消"),
        }
    }
}

// --- Path helpers ---

pub fn join_remote_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

pub fn parent_remote_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => String::from("/"),
        Some(pos) => trimmed[..pos].to_string(),
        None => String::from("/"),
    }
}

// --- Formatting helpers ---

fn format_size(bytes: i64) -> String {
    if bytes < 0 {
        return String::from("-");
    }
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_timestamp(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    // Simple formatting: YYYY-MM-DD HH:MM
    let secs = ts as u64;
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;
    // Days since 1970-01-01 to year/month/day
    // Simplified: just show relative time or a basic format
    let total_days = days;
    let year = 1970 + total_days / 365;
    let remaining_days = total_days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    let h = hours % 24;
    let m = mins % 60;
    format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}")
}
