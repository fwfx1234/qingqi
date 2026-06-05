use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use russh::{
    Channel, ChannelId, ChannelMsg, Pty,
    client::{self, Handle},
    keys::{self, PrivateKeyWithHashAlg},
};
use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use rustls::{
    ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use suppaftp::{
    list::File as FtpListFile,
    tokio::{AsyncFtpStream, AsyncRustlsConnector, AsyncRustlsFtpStream},
    types::{FileType, Mode},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{TlsConnector, rustls};
use webpki_roots::TLS_SERVER_ROOTS;

use crate::model::{AuthMethod, Profile, RemoteProtocol, SshHostKeyPolicy, TlsVerifyPolicy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionHealth {
    pub protocol: RemoteProtocol,
    pub can_terminal: bool,
    pub can_files: bool,
    pub message: String,
}

impl ConnectionHealth {
    pub fn placeholder(protocol: RemoteProtocol) -> Self {
        Self {
            can_terminal: protocol.supports_terminal(),
            can_files: protocol.supports_file_browser(),
            message: format!("{} runtime 尚未接入真实协议客户端", protocol.label()),
            protocol,
        }
    }
}

pub trait RemoteFileClient: Send + Sync {
    fn protocol(&self) -> RemoteProtocol;
    fn connect(&mut self) -> Result<ConnectionHealth>;
    fn list(&self, path: &str) -> Result<Vec<RemoteEntry>>;
    fn stat(&self, path: &str) -> Result<RemoteEntry>;
    fn mkdir(&self, path: &str) -> Result<()>;
    fn rename(&self, from: &str, to: &str) -> Result<()>;
    fn remove(&self, path: &str, is_dir: bool) -> Result<()>;
    fn upload(&self, local_path: &str, remote_path: &str) -> Result<()>;
    fn download(&self, remote_path: &str, local_path: &str) -> Result<()>;
}

pub fn create_file_client(profile: &Profile) -> Box<dyn RemoteFileClient> {
    match profile.protocol {
        RemoteProtocol::Ssh | RemoteProtocol::Sftp => {
            Box::new(SftpFileClient::new(profile.clone()))
        }
        RemoteProtocol::Ftp | RemoteProtocol::FtpsExplicit | RemoteProtocol::FtpsImplicit => {
            Box::new(FtpFileClient::new(profile.clone()))
        }
    }
}

pub struct SftpFileClient {
    profile: Profile,
    last_health_message: Mutex<String>,
}

impl SftpFileClient {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            last_health_message: Mutex::new(String::from("尚未连接")),
        }
    }

    fn run_async<T>(
        &self,
        future: impl std::future::Future<Output = Result<T>> + Send + 'static,
    ) -> Result<T> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("创建异步运行时失败")?;
        runtime.block_on(future)
    }

    fn set_last_health_message(&self, message: String) {
        if let Ok(mut slot) = self.last_health_message.lock() {
            *slot = message;
        }
    }
}

pub struct FtpFileClient {
    profile: Profile,
    last_health_message: Mutex<String>,
}

impl FtpFileClient {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            last_health_message: Mutex::new(String::from("尚未连接")),
        }
    }

    fn run_async<T>(
        &self,
        future: impl std::future::Future<Output = Result<T>> + Send + 'static,
    ) -> Result<T> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("创建异步运行时失败")?;
        runtime.block_on(future)
    }

    fn set_last_health_message(&self, message: String) {
        if let Ok(mut slot) = self.last_health_message.lock() {
            *slot = message;
        }
    }
}

impl RemoteFileClient for FtpFileClient {
    fn protocol(&self) -> RemoteProtocol {
        self.profile.protocol
    }

    fn connect(&mut self) -> Result<ConnectionHealth> {
        let profile = self.profile.clone();
        let message = self.run_async(async move {
            let mut connection = connect_ftp(profile.clone()).await?;
            let cwd = ftp_current_dir(&mut connection)
                .await
                .unwrap_or_else(|_| String::from("/"));
            let base = match profile.protocol {
                RemoteProtocol::Ftp => String::from("已连接 FTP"),
                RemoteProtocol::FtpsExplicit => String::from("已连接 FTPS (Explicit)"),
                RemoteProtocol::FtpsImplicit => String::from("已连接 FTPS (Implicit)"),
                _ => String::from("已连接"),
            };
            Ok(format!("{base} · cwd {cwd}{}", ftp_tls_suffix(&profile)))
        })?;
        self.set_last_health_message(message.clone());
        Ok(ConnectionHealth {
            protocol: self.profile.protocol,
            can_terminal: false,
            can_files: true,
            message,
        })
    }

    fn list(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let path = normalize_ftp_path(path);
        let profile = self.profile.clone();
        self.run_async(async move {
            let mut connection = connect_ftp(profile).await?;
            ftp_list_entries(&mut connection, Some(path.as_str())).await
        })
    }

    fn stat(&self, path: &str) -> Result<RemoteEntry> {
        let path = normalize_ftp_path(path);
        let profile = self.profile.clone();
        self.run_async(async move {
            let mut connection = connect_ftp(profile).await?;
            ftp_stat_entry(&mut connection, &path).await
        })
    }

    fn mkdir(&self, path: &str) -> Result<()> {
        let path = normalize_ftp_path(path);
        let profile = self.profile.clone();
        self.run_async(async move {
            let mut connection = connect_ftp(profile).await?;
            connection
                .mkdir(path.as_str())
                .await
                .with_context(|| format!("创建远端目录失败: {path}"))?;
            ftp_quit(connection).await;
            Ok(())
        })
    }

    fn rename(&self, from: &str, to: &str) -> Result<()> {
        let from = normalize_ftp_path(from);
        let to = normalize_ftp_path(to);
        let profile = self.profile.clone();
        self.run_async(async move {
            let mut connection = connect_ftp(profile).await?;
            connection
                .rename(from.as_str(), to.as_str())
                .await
                .with_context(|| format!("重命名远端文件失败: {from} -> {to}"))?;
            ftp_quit(connection).await;
            Ok(())
        })
    }

    fn remove(&self, path: &str, is_dir: bool) -> Result<()> {
        let path = normalize_ftp_path(path);
        let profile = self.profile.clone();
        self.run_async(async move {
            let mut connection = connect_ftp(profile).await?;
            if is_dir {
                connection
                    .rmdir(path.as_str())
                    .await
                    .with_context(|| format!("删除远端目录失败: {path}"))?;
            } else {
                connection
                    .rm(path.as_str())
                    .await
                    .with_context(|| format!("删除远端文件失败: {path}"))?;
            }
            ftp_quit(connection).await;
            Ok(())
        })
    }

    fn upload(&self, local_path: &str, remote_path: &str) -> Result<()> {
        let local_path = PathBuf::from(local_path);
        let remote_path = normalize_ftp_path(remote_path);
        let profile = self.profile.clone();
        self.run_async(async move {
            let mut connection = connect_ftp(profile).await?;
            let bytes = fs::read(&local_path)
                .with_context(|| format!("读取本地文件失败: {}", local_path.display()))?;
            let mut reader = Cursor::new(bytes);
            connection
                .put_file(remote_path.as_str(), &mut reader)
                .await
                .with_context(|| format!("上传文件失败: {remote_path}"))?;
            ftp_quit(connection).await;
            Ok(())
        })
    }

    fn download(&self, remote_path: &str, local_path: &str) -> Result<()> {
        let remote_path = normalize_ftp_path(remote_path);
        let local_path = PathBuf::from(local_path);
        let profile = self.profile.clone();
        self.run_async(async move {
            let mut connection = connect_ftp(profile).await?;
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("创建本地目录失败: {}", parent.display()))?;
            }
            let bytes = connection
                .retr_bytes(remote_path.as_str())
                .await
                .with_context(|| format!("下载远端文件失败: {remote_path}"))?;
            fs::write(&local_path, bytes)
                .with_context(|| format!("写入本地文件失败: {}", local_path.display()))?;
            ftp_quit(connection).await;
            Ok(())
        })
    }
}

struct SftpConnection {
    _session: Handle<HostKeyHandler>,
    sftp: SftpSession,
    host_key_state: HostKeyCheckState,
}

pub(crate) struct SshConnection {
    session: Handle<HostKeyHandler>,
    host_key_state: HostKeyCheckState,
}

impl SshConnection {
    pub(crate) async fn open_terminal_channel(
        self,
        columns: u32,
        rows: u32,
    ) -> Result<SshPtyChannel> {
        let channel = self
            .session
            .channel_open_session()
            .await
            .context("打开 SSH 终端 channel 失败")?;
        channel
            .request_pty(
                true,
                "xterm-256color",
                columns.max(2),
                rows.max(1),
                0,
                0,
                &default_terminal_modes(),
            )
            .await
            .context("请求远端 PTY 失败")?;
        channel
            .request_shell(true)
            .await
            .context("请求远端 shell 失败")?;
        Ok(SshPtyChannel {
            _session: self.session,
            channel,
        })
    }

    async fn open_sftp(self) -> Result<SftpConnection> {
        let channel = self
            .session
            .channel_open_session()
            .await
            .context("打开 SFTP session channel 失败")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("请求 SFTP 子系统失败")?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .context("初始化 SFTP 客户端失败")?;
        Ok(SftpConnection {
            _session: self.session,
            sftp,
            host_key_state: self.host_key_state,
        })
    }
}

pub(crate) struct SshPtyChannel {
    _session: Handle<HostKeyHandler>,
    channel: Channel<client::Msg>,
}

impl SshPtyChannel {
    pub(crate) async fn send_input(&self, bytes: &[u8]) -> Result<()> {
        let mut writer = self.channel.make_writer();
        writer.write_all(bytes).await.context("写入远端终端失败")?;
        writer.flush().await.context("刷新远端终端输出失败")?;
        Ok(())
    }

    pub(crate) async fn resize(&self, columns: u32, rows: u32) -> Result<()> {
        self.channel
            .window_change(columns.max(2), rows.max(1), 0, 0)
            .await
            .context("调整远端终端尺寸失败")
    }

    pub(crate) async fn recv(&mut self) -> Option<ChannelMsg> {
        self.channel.wait().await
    }

    pub(crate) async fn close(&self) -> Result<()> {
        self.channel.close().await.context("关闭远端终端失败")
    }
}

impl RemoteFileClient for SftpFileClient {
    fn protocol(&self) -> RemoteProtocol {
        self.profile.protocol
    }

    fn connect(&mut self) -> Result<ConnectionHealth> {
        let profile = self.profile.clone();
        let (message, can_terminal, can_files) = self.run_async(async move {
            let connection = connect_sftp(profile.clone()).await?;
            Ok((
                connection_message(&profile, &connection.host_key_state),
                profile.protocol.supports_terminal(),
                true,
            ))
        })?;
        self.set_last_health_message(message.clone());
        Ok(ConnectionHealth {
            protocol: self.profile.protocol,
            can_terminal,
            can_files,
            message,
        })
    }

    fn list(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let requested_path = path.to_string();
        let profile = self.profile.clone();
        self.run_async(async move {
            let connection = connect_sftp(profile).await?;
            let path = resolve_sftp_path(&connection.sftp, &requested_path).await?;
            let mut items: Vec<RemoteEntry> = connection
                .sftp
                .read_dir(path.clone())
                .await
                .with_context(|| format!("读取远端目录失败: {path}"))?
                .map(|entry| RemoteEntry {
                    name: entry.file_name(),
                    path: entry.path(),
                    is_dir: entry.metadata().is_dir(),
                    size: entry.metadata().size.unwrap_or_default(),
                })
                .collect();
            items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });
            Ok(items)
        })
    }

    fn stat(&self, path: &str) -> Result<RemoteEntry> {
        let requested_path = path.to_string();
        let profile = self.profile.clone();
        self.run_async(async move {
            let connection = connect_sftp(profile).await?;
            let path = resolve_sftp_path(&connection.sftp, &requested_path).await?;
            let metadata = connection
                .sftp
                .metadata(path.clone())
                .await
                .with_context(|| format!("读取远端文件信息失败: {path}"))?;
            Ok(RemoteEntry {
                name: remote_basename(&path),
                path,
                is_dir: metadata.is_dir(),
                size: metadata.size.unwrap_or_default(),
            })
        })
    }

    fn mkdir(&self, path: &str) -> Result<()> {
        let requested_path = path.to_string();
        let profile = self.profile.clone();
        self.run_async(async move {
            let connection = connect_sftp(profile).await?;
            let path = resolve_sftp_parented_path(&connection.sftp, &requested_path).await?;
            connection
                .sftp
                .create_dir(path.clone())
                .await
                .with_context(|| format!("创建远端目录失败: {path}"))?;
            Ok(())
        })
    }

    fn rename(&self, from: &str, to: &str) -> Result<()> {
        let from_requested = from.to_string();
        let to_requested = to.to_string();
        let profile = self.profile.clone();
        self.run_async(async move {
            let connection = connect_sftp(profile).await?;
            let from = resolve_sftp_path(&connection.sftp, &from_requested).await?;
            let to = resolve_sftp_parented_path(&connection.sftp, &to_requested).await?;
            connection
                .sftp
                .rename(from.clone(), to.clone())
                .await
                .with_context(|| format!("重命名远端文件失败: {from} -> {to}"))?;
            Ok(())
        })
    }

    fn remove(&self, path: &str, is_dir: bool) -> Result<()> {
        let requested_path = path.to_string();
        let profile = self.profile.clone();
        self.run_async(async move {
            let connection = connect_sftp(profile).await?;
            let path = resolve_sftp_path(&connection.sftp, &requested_path).await?;
            if is_dir {
                connection
                    .sftp
                    .remove_dir(path.clone())
                    .await
                    .with_context(|| format!("删除远端目录失败: {path}"))?;
            } else {
                connection
                    .sftp
                    .remove_file(path.clone())
                    .await
                    .with_context(|| format!("删除远端文件失败: {path}"))?;
            }
            Ok(())
        })
    }

    fn upload(&self, local_path: &str, remote_path: &str) -> Result<()> {
        let local_path = PathBuf::from(local_path);
        let requested_remote_path = remote_path.to_string();
        let profile = self.profile.clone();
        self.run_async(async move {
            let connection = connect_sftp(profile).await?;
            let remote_path =
                resolve_sftp_parented_path(&connection.sftp, &requested_remote_path).await?;
            let bytes = fs::read(&local_path)
                .with_context(|| format!("读取本地文件失败: {}", local_path.display()))?;
            let mut remote = connection
                .sftp
                .open_with_flags(
                    remote_path.clone(),
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                )
                .await
                .with_context(|| format!("打开远端文件失败: {remote_path}"))?;
            remote
                .write_all(&bytes)
                .await
                .with_context(|| format!("上传文件失败: {remote_path}"))?;
            remote
                .shutdown()
                .await
                .with_context(|| format!("关闭远端文件失败: {remote_path}"))?;
            Ok(())
        })
    }

    fn download(&self, remote_path: &str, local_path: &str) -> Result<()> {
        let requested_remote_path = remote_path.to_string();
        let local_path = PathBuf::from(local_path);
        let profile = self.profile.clone();
        self.run_async(async move {
            let connection = connect_sftp(profile).await?;
            let remote_path = resolve_sftp_path(&connection.sftp, &requested_remote_path).await?;
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("创建本地目录失败: {}", parent.display()))?;
            }
            let bytes = connection
                .sftp
                .read(remote_path.clone())
                .await
                .with_context(|| format!("下载远端文件失败: {remote_path}"))?;
            fs::write(&local_path, bytes)
                .with_context(|| format!("写入本地文件失败: {}", local_path.display()))?;
            Ok(())
        })
    }
}

enum FtpConnection {
    Plain(AsyncFtpStream),
    Secure(AsyncRustlsFtpStream),
}

impl FtpConnection {
    async fn pwd(&mut self) -> std::result::Result<String, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.pwd().await,
            Self::Secure(stream) => stream.pwd().await,
        }
    }

    async fn transfer_type(
        &mut self,
        file_type: FileType,
    ) -> std::result::Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.transfer_type(file_type).await,
            Self::Secure(stream) => stream.transfer_type(file_type).await,
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        match self {
            Self::Plain(stream) => stream.set_mode(mode),
            Self::Secure(stream) => stream.set_mode(mode),
        }
    }

    async fn list(
        &mut self,
        path: Option<&str>,
    ) -> std::result::Result<Vec<String>, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.list(path).await,
            Self::Secure(stream) => stream.list(path).await,
        }
    }

    async fn mlsd(
        &mut self,
        path: Option<&str>,
    ) -> std::result::Result<Vec<String>, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.mlsd(path).await,
            Self::Secure(stream) => stream.mlsd(path).await,
        }
    }

    async fn mlst(
        &mut self,
        path: Option<&str>,
    ) -> std::result::Result<String, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.mlst(path).await,
            Self::Secure(stream) => stream.mlst(path).await,
        }
    }

    async fn size(&mut self, path: &str) -> std::result::Result<usize, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.size(path).await,
            Self::Secure(stream) => stream.size(path).await,
        }
    }

    async fn mkdir(&mut self, path: &str) -> std::result::Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.mkdir(path).await,
            Self::Secure(stream) => stream.mkdir(path).await,
        }
    }

    async fn rename(
        &mut self,
        from: &str,
        to: &str,
    ) -> std::result::Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.rename(from, to).await,
            Self::Secure(stream) => stream.rename(from, to).await,
        }
    }

    async fn rmdir(&mut self, path: &str) -> std::result::Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.rmdir(path).await,
            Self::Secure(stream) => stream.rmdir(path).await,
        }
    }

    async fn rm(&mut self, path: &str) -> std::result::Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.rm(path).await,
            Self::Secure(stream) => stream.rm(path).await,
        }
    }

    async fn put_file(
        &mut self,
        remote_path: &str,
        reader: &mut Cursor<Vec<u8>>,
    ) -> std::result::Result<u64, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.put_file(remote_path, reader).await,
            Self::Secure(stream) => stream.put_file(remote_path, reader).await,
        }
    }

    async fn retr_bytes(
        &mut self,
        remote_path: &str,
    ) -> std::result::Result<Vec<u8>, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => {
                stream
                    .retr(remote_path, |mut data_stream| {
                        Box::pin(async move {
                            let mut bytes = Vec::new();
                            data_stream
                                .read_to_end(&mut bytes)
                                .await
                                .map_err(suppaftp::FtpError::ConnectionError)?;
                            Ok((bytes, data_stream))
                        })
                    })
                    .await
            }
            Self::Secure(stream) => {
                stream
                    .retr(remote_path, |mut data_stream| {
                        Box::pin(async move {
                            let mut bytes = Vec::new();
                            data_stream
                                .read_to_end(&mut bytes)
                                .await
                                .map_err(suppaftp::FtpError::ConnectionError)?;
                            Ok((bytes, data_stream))
                        })
                    })
                    .await
            }
        }
    }

    async fn login(
        &mut self,
        username: &str,
        password: &str,
    ) -> std::result::Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.login(username, password).await,
            Self::Secure(stream) => stream.login(username, password).await,
        }
    }

    async fn quit(self) -> std::result::Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(mut stream) => stream.quit().await,
            Self::Secure(mut stream) => stream.quit().await,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct HostKeyCheckState {
    fingerprint_sha256: Option<String>,
    accepted_with_warning: bool,
    warning_message: Option<String>,
}

#[derive(Clone)]
struct HostKeyHandler {
    profile: Profile,
    state: Arc<Mutex<HostKeyCheckState>>,
}

impl HostKeyHandler {
    fn new(profile: Profile, state: Arc<Mutex<HostKeyCheckState>>) -> Self {
        Self { profile, state }
    }

    fn record(&self, update: impl FnOnce(&mut HostKeyCheckState)) {
        if let Ok(mut state) = self.state.lock() {
            update(&mut state);
        }
    }
}

impl client::Handler for HostKeyHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = format!("{}", server_public_key.fingerprint(keys::HashAlg::Sha256));
        self.record(|state| state.fingerprint_sha256 = Some(fingerprint.clone()));

        match self.profile.security.ssh_host_key {
            SshHostKeyPolicy::InsecureAcceptAny => {
                self.record(|state| {
                    state.accepted_with_warning = true;
                    state.warning_message =
                        Some(String::from("当前会话使用了不安全的主机密钥策略"));
                });
                Ok(true)
            }
            SshHostKeyPolicy::TrustOnFirstUse => Ok(true),
            SshHostKeyPolicy::StrictPinned => {
                let expected = self.profile.security.pinned_host_key.trim();
                if expected.is_empty() {
                    self.record(|state| {
                        state.warning_message = Some(String::from("未配置主机密钥指纹"));
                    });
                    return Ok(false);
                }
                let matched =
                    normalize_fingerprint(expected) == normalize_fingerprint(&fingerprint);
                if !matched {
                    self.record(|state| {
                        state.warning_message = Some(format!(
                            "主机密钥不匹配，期望 {expected}，实际 {fingerprint}"
                        ));
                    });
                }
                Ok(matched)
            }
        }
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        _data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) async fn connect_ssh(profile: Profile) -> Result<SshConnection> {
    let state = Arc::new(Mutex::new(HostKeyCheckState::default()));
    let handler = HostKeyHandler::new(profile.clone(), Arc::clone(&state));
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(
            profile.limits.connect_timeout_secs as u64,
        )),
        ..Default::default()
    });
    let mut session: Handle<HostKeyHandler> =
        client::connect(config, (profile.host.as_str(), profile.port), handler)
            .await
            .with_context(|| format!("连接远端主机失败: {}", profile.endpoint()))?;

    authenticate_session(&profile, &mut session).await?;

    let host_key_state = state.lock().map(|value| value.clone()).unwrap_or_default();
    Ok(SshConnection {
        session,
        host_key_state,
    })
}

async fn connect_sftp(profile: Profile) -> Result<SftpConnection> {
    connect_ssh(profile).await?.open_sftp().await
}

async fn connect_ftp(profile: Profile) -> Result<FtpConnection> {
    let address = format!("{}:{}", profile.host, profile.port);
    let timeout = Duration::from_secs(profile.limits.connect_timeout_secs as u64);

    let mut connection = match profile.protocol {
        RemoteProtocol::Ftp => {
            let socket_addr = tokio::net::lookup_host(address.as_str())
                .await
                .with_context(|| format!("解析远端主机失败: {}", profile.endpoint()))?
                .next()
                .with_context(|| format!("未找到远端地址: {}", profile.endpoint()))?;
            let stream = AsyncFtpStream::connect_timeout(socket_addr, timeout)
                .await
                .with_context(|| format!("连接 FTP 主机失败: {}", profile.endpoint()))?;
            FtpConnection::Plain(stream)
        }
        RemoteProtocol::FtpsExplicit => {
            let socket_addr = tokio::net::lookup_host(address.as_str())
                .await
                .with_context(|| format!("解析远端主机失败: {}", profile.endpoint()))?
                .next()
                .with_context(|| format!("未找到远端地址: {}", profile.endpoint()))?;
            let stream = AsyncRustlsFtpStream::connect_timeout(socket_addr, timeout)
                .await
                .with_context(|| format!("连接 FTPS 主机失败: {}", profile.endpoint()))?;
            let connector = ftp_tls_connector(&profile)?;
            let stream = stream
                .into_secure(connector, profile.host.as_str())
                .await
                .with_context(|| format!("升级 FTPS 显式 TLS 失败: {}", profile.endpoint()))?;
            FtpConnection::Secure(stream)
        }
        RemoteProtocol::FtpsImplicit => {
            let connector = ftp_tls_connector(&profile)?;
            let stream = AsyncRustlsFtpStream::connect_secure_implicit(
                address.as_str(),
                connector,
                profile.host.as_str(),
            )
            .await
            .with_context(|| format!("连接 FTPS 隐式 TLS 失败: {}", profile.endpoint()))?;
            FtpConnection::Secure(stream)
        }
        RemoteProtocol::Ssh | RemoteProtocol::Sftp => {
            bail!("当前 profile 不是 FTP/FTPS 协议")
        }
    };

    let username = profile.auth.username.trim();
    if username.is_empty() {
        bail!("用户名不能为空");
    }
    if profile.auth.method != AuthMethod::Password {
        bail!("FTP/FTPS 当前仅支持密码认证");
    }

    connection
        .login(username, profile.auth.password.as_str())
        .await
        .context("FTP/FTPS 登录失败")?;
    connection
        .transfer_type(FileType::Binary)
        .await
        .context("切换 FTP 二进制传输模式失败")?;
    connection.set_mode(if profile.limits.passive_mode {
        Mode::Passive
    } else {
        Mode::Active
    });

    Ok(connection)
}

async fn authenticate_session(
    profile: &Profile,
    session: &mut Handle<HostKeyHandler>,
) -> Result<()> {
    let username = profile.auth.username.trim();
    if username.is_empty() {
        bail!("用户名不能为空");
    }

    let success = match profile.auth.method {
        AuthMethod::Password => session
            .authenticate_password(username, profile.auth.password.clone())
            .await
            .context("SSH 密码认证失败")?
            .success(),
        AuthMethod::PrivateKey => {
            let path = profile.auth.private_key_path.trim();
            if path.is_empty() {
                bail!("未配置私钥路径");
            }
            let password = (!profile.auth.private_key_passphrase.trim().is_empty())
                .then_some(profile.auth.private_key_passphrase.as_str());
            let private_key = keys::load_secret_key(path, password)
                .with_context(|| format!("加载私钥失败: {path}"))?;
            let hash_alg = session.best_supported_rsa_hash().await?.flatten();
            session
                .authenticate_publickey(
                    username,
                    PrivateKeyWithHashAlg::new(Arc::new(private_key), hash_alg),
                )
                .await
                .context("SSH 私钥认证失败")?
                .success()
        }
        AuthMethod::Agent => bail!("当前版本暂未接入 SSH Agent 认证"),
    };

    if !success {
        bail!("远端认证未通过");
    }

    Ok(())
}

fn connection_message(profile: &Profile, state: &HostKeyCheckState) -> String {
    let fingerprint = state
        .fingerprint_sha256
        .clone()
        .unwrap_or_else(|| String::from("unknown"));
    match profile.security.ssh_host_key {
        SshHostKeyPolicy::InsecureAcceptAny => {
            format!("已连接，主机密钥未校验 ({fingerprint})")
        }
        SshHostKeyPolicy::TrustOnFirstUse => format!("已连接，主机指纹 {fingerprint}"),
        SshHostKeyPolicy::StrictPinned => format!("已校验主机指纹 {fingerprint}"),
    }
}

fn ftp_tls_connector(profile: &Profile) -> Result<AsyncRustlsConnector> {
    let config = Arc::new(build_tls_config(profile)?);
    Ok(AsyncRustlsConnector::from(TlsConnector::from(config)))
}

fn build_tls_config(profile: &Profile) -> Result<ClientConfig> {
    match profile.security.tls_verify {
        TlsVerifyPolicy::SystemRoots => {
            let roots = load_root_store();
            Ok(ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth())
        }
        TlsVerifyPolicy::PinnedSha256 => {
            let verifier = Arc::new(PinnedCertificateVerifier::new(
                profile.security.pinned_tls_sha256.clone(),
            )?);
            Ok(ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth())
        }
        TlsVerifyPolicy::InsecureAcceptAny => Ok(ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
            .with_no_client_auth()),
    }
}

fn load_root_store() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    let mut loaded_any = false;
    if !native.certs.is_empty() {
        let _ = roots.add_parsable_certificates(native.certs);
        loaded_any = true;
    }
    if !loaded_any {
        roots.extend(TLS_SERVER_ROOTS.iter().cloned());
    }
    roots
}

#[derive(Debug)]
struct InsecureVerifier;

impl ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        default_supported_verify_schemes()
    }
}

#[derive(Debug)]
struct PinnedCertificateVerifier {
    normalized_pin: String,
}

impl PinnedCertificateVerifier {
    fn new(pin: String) -> Result<Self> {
        let normalized_pin = normalize_fingerprint(&pin);
        if normalized_pin.is_empty() {
            bail!("未配置 TLS 证书指纹");
        }
        Ok(Self { normalized_pin })
    }
}

impl ServerCertVerifier for PinnedCertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let actual = format!(
            "sha256:{}",
            hex_lower(Sha256::digest(end_entity.as_ref()).as_slice())
        );
        if normalize_fingerprint(&actual) == self.normalized_pin {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(format!(
                "TLS 证书指纹不匹配，期望 {}，实际 {}",
                self.normalized_pin, actual
            )))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        default_supported_verify_schemes()
    }
}

async fn ftp_current_dir(connection: &mut FtpConnection) -> Result<String> {
    connection.pwd().await.context("读取 FTP 当前目录失败")
}

async fn ftp_list_entries(
    connection: &mut FtpConnection,
    path: Option<&str>,
) -> Result<Vec<RemoteEntry>> {
    let path_value = path.unwrap_or("/");
    let machine_entries = connection.mlsd(path).await.ok();
    let mut items = if let Some(lines) = machine_entries {
        lines
            .into_iter()
            .filter_map(|line| FtpListFile::try_from(line).ok())
            .filter(|item| item.name() != "." && item.name() != "..")
            .map(|item| ftp_remote_entry_from_item(path_value, item))
            .collect::<Vec<_>>()
    } else {
        connection
            .list(path)
            .await
            .with_context(|| format!("读取远端目录失败: {path_value}"))?
            .into_iter()
            .filter_map(|line| FtpListFile::try_from(line).ok())
            .filter(|item| item.name() != "." && item.name() != "..")
            .map(|item| ftp_remote_entry_from_item(path_value, item))
            .collect::<Vec<_>>()
    };
    items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(items)
}

async fn ftp_stat_entry(connection: &mut FtpConnection, path: &str) -> Result<RemoteEntry> {
    if let Ok(line) = connection.mlst(Some(path)).await
        && let Ok(item) = FtpListFile::try_from(line)
    {
        return Ok(ftp_remote_entry_exact(path, item));
    }

    if let Ok(size) = connection.size(path).await {
        return Ok(RemoteEntry {
            name: remote_basename(path),
            path: path.to_string(),
            is_dir: false,
            size: size as u64,
        });
    }

    let parent = remote_parent_dir(path);
    let name = remote_basename(path);
    let entries = ftp_list_entries(connection, Some(parent.as_str())).await?;
    entries
        .into_iter()
        .find(|entry| entry.name == name || entry.path == path)
        .with_context(|| format!("读取远端文件信息失败: {path}"))
}

fn ftp_remote_entry_from_item(base: &str, item: FtpListFile) -> RemoteEntry {
    let name = item.name().to_string();
    RemoteEntry {
        path: join_ftp_path(base, &name),
        name,
        is_dir: item.is_directory(),
        size: item.size() as u64,
    }
}

fn ftp_remote_entry_exact(path: &str, item: FtpListFile) -> RemoteEntry {
    RemoteEntry {
        name: remote_basename(path),
        path: path.to_string(),
        is_dir: item.is_directory(),
        size: item.size() as u64,
    }
}

async fn ftp_quit(connection: FtpConnection) {
    let _ = connection.quit().await;
}

fn ftp_tls_suffix(profile: &Profile) -> String {
    match profile.protocol {
        RemoteProtocol::FtpsExplicit | RemoteProtocol::FtpsImplicit => {
            match profile.security.tls_verify {
                TlsVerifyPolicy::SystemRoots => String::from(" · TLS 系统证书"),
                TlsVerifyPolicy::PinnedSha256 => String::from(" · TLS 指纹固定"),
                TlsVerifyPolicy::InsecureAcceptAny => String::from(" · TLS 未校验"),
            }
        }
        _ => String::new(),
    }
}

fn normalize_ftp_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "." {
        String::from("/")
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn join_ftp_path(parent: &str, child: &str) -> String {
    if parent == "/" {
        format!("/{child}")
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), child)
    }
}

fn remote_parent_dir(path: &str) -> String {
    let normalized = normalize_ftp_path(path);
    if normalized == "/" {
        return normalized;
    }
    let mut parts = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let _ = parts.pop();
    if parts.is_empty() {
        String::from("/")
    } else {
        format!("/{}", parts.join("/"))
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn default_supported_verify_schemes() -> Vec<SignatureScheme> {
    vec![
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::ED25519,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PKCS1_SHA512,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PKCS1_SHA256,
    ]
}

fn default_terminal_modes() -> Vec<(Pty, u32)> {
    vec![
        (Pty::ECHO, 1),
        (Pty::ICANON, 1),
        (Pty::ISIG, 1),
        (Pty::IEXTEN, 1),
        (Pty::IXON, 1),
        (Pty::TTY_OP_ISPEED, 14400),
        (Pty::TTY_OP_OSPEED, 14400),
    ]
}

fn normalize_remote_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "." {
        String::from("/")
    } else if trimmed == "~" || trimmed.starts_with("~/") {
        trimmed.to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

async fn resolve_sftp_path(session: &SftpSession, path: &str) -> Result<String> {
    let normalized = normalize_remote_path(path);
    if normalized == "~" {
        return session
            .canonicalize(".")
            .await
            .context("解析远端 home 目录失败");
    }
    if let Some(rest) = normalized.strip_prefix("~/") {
        let home = session
            .canonicalize(".")
            .await
            .context("解析远端 home 目录失败")?;
        return Ok(join_remote_segments(&home, rest));
    }
    Ok(normalized)
}

async fn resolve_sftp_parented_path(session: &SftpSession, path: &str) -> Result<String> {
    let normalized = normalize_remote_path(path);
    if normalized == "~" {
        return resolve_sftp_path(session, "~").await;
    }
    if let Some(rest) = normalized.strip_prefix("~/") {
        let home = session
            .canonicalize(".")
            .await
            .context("解析远端 home 目录失败")?;
        return Ok(join_remote_segments(&home, rest));
    }
    Ok(normalized)
}

fn join_remote_segments(parent: &str, child: &str) -> String {
    let trimmed_child = child.trim_matches('/');
    if trimmed_child.is_empty() {
        parent.trim_end_matches('/').to_string()
    } else if parent == "/" {
        format!("/{trimmed_child}")
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), trimmed_child)
    }
}

fn remote_basename(path: &str) -> String {
    path.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn normalize_fingerprint(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{normalize_fingerprint, normalize_remote_path, remote_basename};

    #[test]
    fn normalizes_remote_paths() {
        assert_eq!(normalize_remote_path(""), "/");
        assert_eq!(normalize_remote_path("var/log"), "/var/log");
        assert_eq!(normalize_remote_path("/tmp"), "/tmp");
        assert_eq!(normalize_remote_path("~"), "~");
        assert_eq!(normalize_remote_path("~/demo"), "~/demo");
    }

    #[test]
    fn extracts_remote_basename() {
        assert_eq!(remote_basename("/tmp/demo.txt"), "demo.txt");
        assert_eq!(remote_basename("/"), "/");
    }

    #[test]
    fn normalizes_fingerprint_case() {
        assert_eq!(normalize_fingerprint(" SHA256:ABC123 "), "sha256:abc123");
    }
}
