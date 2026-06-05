use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteProtocol {
    Ssh,
    Sftp,
    Ftp,
    FtpsExplicit,
    FtpsImplicit,
}

impl RemoteProtocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
            Self::Sftp => "SFTP",
            Self::Ftp => "FTP",
            Self::FtpsExplicit => "FTPS (Explicit)",
            Self::FtpsImplicit => "FTPS (Implicit)",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Sftp => "sftp",
            Self::Ftp => "ftp",
            Self::FtpsExplicit => "ftps_explicit",
            Self::FtpsImplicit => "ftps_implicit",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "ssh" => Self::Ssh,
            "ftp" => Self::Ftp,
            "ftps_explicit" => Self::FtpsExplicit,
            "ftps_implicit" => Self::FtpsImplicit,
            "sftp" => Self::Sftp,
            _ => Self::Sftp,
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Self::Ssh | Self::Sftp => 22,
            Self::Ftp => 21,
            Self::FtpsExplicit => 21,
            Self::FtpsImplicit => 990,
        }
    }

    pub fn default_remote_root(self) -> &'static str {
        match self {
            Self::Ssh | Self::Sftp => "~",
            Self::Ftp | Self::FtpsExplicit | Self::FtpsImplicit => "/",
        }
    }

    pub fn supports_terminal(self) -> bool {
        matches!(self, Self::Ssh)
    }

    pub fn supports_file_browser(self) -> bool {
        true
    }

    pub fn is_secure(self) -> bool {
        !matches!(self, Self::Ftp)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Password,
    PrivateKey,
    Agent,
}

impl AuthMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Password => "密码",
            Self::PrivateKey => "私钥",
            Self::Agent => "SSH Agent",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::PrivateKey => "private_key",
            Self::Agent => "agent",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshHostKeyPolicy {
    TrustOnFirstUse,
    StrictPinned,
    InsecureAcceptAny,
}

impl SshHostKeyPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::TrustOnFirstUse => "首次信任",
            Self::StrictPinned => "严格校验",
            Self::InsecureAcceptAny => "不安全接受",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrustOnFirstUse => "tofu",
            Self::StrictPinned => "strict",
            Self::InsecureAcceptAny => "insecure",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "strict" => Self::StrictPinned,
            "insecure" => Self::InsecureAcceptAny,
            _ => Self::TrustOnFirstUse,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsVerifyPolicy {
    SystemRoots,
    PinnedSha256,
    InsecureAcceptAny,
}

impl TlsVerifyPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::SystemRoots => "系统证书",
            Self::PinnedSha256 => "证书指纹",
            Self::InsecureAcceptAny => "不安全接受",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemRoots => "system",
            Self::PinnedSha256 => "pinned",
            Self::InsecureAcceptAny => "insecure",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "pinned" => Self::PinnedSha256,
            "insecure" => Self::InsecureAcceptAny,
            _ => Self::SystemRoots,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TerminalId(pub Uuid);

impl TerminalId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TerminalId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransferId(pub Uuid);

impl TransferId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TransferId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthConfig {
    pub method: AuthMethod,
    pub username: String,
    pub password: String,
    pub private_key_path: String,
    pub private_key_passphrase: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            method: AuthMethod::Password,
            username: String::new(),
            password: String::new(),
            private_key_path: String::new(),
            private_key_passphrase: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub ssh_host_key: SshHostKeyPolicy,
    pub pinned_host_key: String,
    pub tls_verify: TlsVerifyPolicy,
    pub pinned_tls_sha256: String,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            ssh_host_key: SshHostKeyPolicy::TrustOnFirstUse,
            pinned_host_key: String::new(),
            tls_verify: TlsVerifyPolicy::SystemRoots,
            pinned_tls_sha256: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilePaths {
    pub remote_root: String,
    pub local_root: String,
}

impl Default for ProfilePaths {
    fn default() -> Self {
        Self {
            remote_root: RemoteProtocol::Ssh.default_remote_root().to_string(),
            local_root: String::from("~/Downloads"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionLimits {
    pub connect_timeout_secs: u16,
    pub transfer_concurrency: u16,
    pub passive_mode: bool,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            connect_timeout_secs: 15,
            transfer_concurrency: 3,
            passive_mode: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub protocol: RemoteProtocol,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub paths: ProfilePaths,
    pub security: SecurityPolicy,
    pub limits: ConnectionLimits,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: String,
}

impl Profile {
    pub fn endpoint(&self) -> String {
        let user = self.auth.username.trim();
        if user.is_empty() {
            format!("{}:{}", self.host, self.port)
        } else {
            format!("{user}@{}:{}", self.host, self.port)
        }
    }

    pub fn protocol_label(&self) -> &'static str {
        self.protocol.label()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDraft {
    pub name: String,
    pub protocol: RemoteProtocol,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub paths: ProfilePaths,
    pub security: SecurityPolicy,
    pub limits: ConnectionLimits,
    pub notes: String,
}

impl Default for ProfileDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            protocol: RemoteProtocol::Ssh,
            host: String::new(),
            port: RemoteProtocol::Ssh.default_port(),
            auth: AuthConfig::default(),
            paths: ProfilePaths::default(),
            security: SecurityPolicy::default(),
            limits: ConnectionLimits::default(),
            notes: String::new(),
        }
    }
}

impl ProfileDraft {
    pub fn normalize(mut self) -> Self {
        let baseline_port = ProfileDraft::default().port;
        let protocol_default_port = self.protocol.default_port();
        self.name = self.name.trim().to_string();
        self.host = self.host.trim().to_string();
        self.auth.username = self.auth.username.trim().to_string();
        self.auth.private_key_path = self.auth.private_key_path.trim().to_string();
        self.auth.private_key_passphrase = self.auth.private_key_passphrase.trim().to_string();
        self.paths.remote_root =
            normalize_remote_root_for_protocol(&self.paths.remote_root, self.protocol);
        self.paths.local_root = normalize_local_root(&self.paths.local_root);
        if self.port == 0 || (self.port == baseline_port && protocol_default_port != baseline_port)
        {
            self.port = protocol_default_port;
        }
        if self.limits.connect_timeout_secs == 0 {
            self.limits.connect_timeout_secs = 15;
        }
        if self.limits.transfer_concurrency == 0 {
            self.limits.transfer_concurrency = 3;
        }
        if self.name.is_empty() {
            self.name = if self.host.is_empty() {
                String::from("未命名连接")
            } else if self.auth.username.is_empty() {
                format!("{} {}", self.protocol.label(), self.host)
            } else {
                format!("{}@{}", self.auth.username, self.host)
            };
        }
        self
    }

    pub fn from_profile(profile: &Profile) -> Self {
        Self {
            name: profile.name.clone(),
            protocol: profile.protocol,
            host: profile.host.clone(),
            port: profile.port,
            auth: profile.auth.clone(),
            paths: profile.paths.clone(),
            security: profile.security.clone(),
            limits: profile.limits.clone(),
            notes: profile.notes.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Degraded,
    Failed,
    Closed,
}

impl SessionStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "连接中",
            Self::Connected => "已连接",
            Self::Degraded => "部分可用",
            Self::Failed => "失败",
            Self::Closed => "已关闭",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub profile_id: i64,
    pub title: String,
    pub protocol: RemoteProtocol,
    pub endpoint: String,
    pub status: SessionStatus,
    pub has_terminal: bool,
    pub transfer_count: usize,
    pub message: String,
}

impl SessionSummary {
    pub fn supports_terminal(&self) -> bool {
        self.protocol.supports_terminal()
    }
}

fn normalize_remote_root_for_protocol(value: &str, protocol: RemoteProtocol) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return protocol.default_remote_root().to_string();
    }
    if trimmed == "~" || trimmed.starts_with("~/") {
        return trimmed.to_string();
    }
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn normalize_local_root(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        String::from("~/Downloads")
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProfileDraft, RemoteProtocol, SessionId, SshHostKeyPolicy, TerminalId, TlsVerifyPolicy,
        TransferId,
    };

    #[test]
    fn protocol_capabilities_match_expectation() {
        assert!(RemoteProtocol::Ssh.supports_terminal());
        assert!(RemoteProtocol::Sftp.supports_file_browser());
        assert!(!RemoteProtocol::Ftp.supports_terminal());
        assert!(RemoteProtocol::FtpsExplicit.is_secure());
        assert!(!RemoteProtocol::Ftp.is_secure());
    }

    #[test]
    fn draft_normalize_fills_defaults() {
        let draft = ProfileDraft {
            host: "example.com".into(),
            protocol: RemoteProtocol::FtpsImplicit,
            ..ProfileDraft::default()
        }
        .normalize();
        assert_eq!(draft.port, 990);
        assert_eq!(draft.paths.remote_root, "~");
        assert_eq!(draft.limits.transfer_concurrency, 3);
        assert!(!draft.name.is_empty());
    }

    #[test]
    fn policy_roundtrip_strings_are_stable() {
        assert_eq!(
            SshHostKeyPolicy::from_db("strict"),
            SshHostKeyPolicy::StrictPinned
        );
        assert_eq!(
            TlsVerifyPolicy::from_db("pinned"),
            TlsVerifyPolicy::PinnedSha256
        );
    }

    #[test]
    fn ids_are_unique() {
        assert_ne!(SessionId::new(), SessionId::new());
        assert_ne!(TerminalId::new(), TerminalId::new());
        assert_ne!(TransferId::new(), TransferId::new());
    }
}
