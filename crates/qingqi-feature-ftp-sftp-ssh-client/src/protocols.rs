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
use tokio::time::timeout as tokio_timeout;
use tokio_rustls::{TlsConnector, rustls};
use tracing::{debug, error};
use webpki_roots::TLS_SERVER_ROOTS;

use crate::model::{AuthMethod, Profile, RemoteProtocol, SshHostKeyPolicy, TlsVerifyPolicy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_at: Option<u64>,
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
    /// TOFU 首次连接时待持久化的主机指纹（仅 SSH/SFTP 客户端有）
    fn tou_pending_fingerprint(&self) -> Option<String> {
        None
    }
}

pub fn create_file_client(profile: &Profile, rt: tokio::runtime::Handle) -> Box<dyn RemoteFileClient> {
    let profile = profile.clone();
    match profile.protocol {
        RemoteProtocol::Ssh | RemoteProtocol::Sftp => {
            let mut client = SftpFileClient {
                profile,
                last_health_message: Mutex::new(String::from("尚未连接")),
                rt: rt.clone(),
                shared_session: Mutex::new(None),
                sftp_cache: Arc::new(Mutex::new(None)),
                tou_pending: Mutex::new(None),
            };
            let _ = client.connect();
            Box::new(client)
        }
        RemoteProtocol::Ftp | RemoteProtocol::FtpsExplicit | RemoteProtocol::FtpsImplicit => {
            let mut client = FtpFileClient::with_handle(profile, rt.clone());
            let _ = client.connect();
            Box::new(client)
        }
    }
}

pub struct SftpFileClient {
    profile: Profile,
    last_health_message: Mutex<String>,
    /// Session 级 Runtime Handle（Clone + Send + Sync），所有操作复用
    rt: tokio::runtime::Handle,
    /// 缓存的已认证 SSH 会话句柄
    shared_session: Mutex<Option<Arc<Handle<HostKeyHandler>>>>,
    /// 缓存的 SFTP 会话，避免每次操作都重新打开 channel
    sftp_cache: Arc<Mutex<Option<SftpSession>>>,
    /// TOFU 首次连接时待持久化的主机指纹
    tou_pending: Mutex<Option<String>>,
}

impl SftpFileClient {
    /// 使用已认证的 Handle 和 Session Runtime 创建客户端
    pub fn with_handle(
        profile: Profile,
        rt: tokio::runtime::Handle,
        handle: Arc<Handle<HostKeyHandler>>,
    ) -> Self {
        Self {
            profile,
            last_health_message: Mutex::new(String::from("已连接")),
            rt,
            shared_session: Mutex::new(Some(handle)),
            sftp_cache: Arc::new(Mutex::new(None)),
            tou_pending: Mutex::new(None),
        }
    }

    /// 获取缓存的 Handle，如果尚未连接则返回错误
    fn cached_handle(&self) -> Result<Arc<Handle<HostKeyHandler>>> {
        self.shared_session
            .lock()
            .map_err(|_| anyhow::anyhow!("SSH session lock poisoned"))?
            .clone()
            .context("SSH 会话尚未建立，请先连接")
    }

    fn run_async<T: Send + 'static>(
        &self,
        future: impl std::future::Future<Output = Result<T>> + Send + 'static,
    ) -> Result<T> {
        self.rt.block_on(future)
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
    /// Session 级 Runtime Handle
    rt: tokio::runtime::Handle,
    /// 缓存的已登录 FTP 连接，操作间复用
    cached_connection: Mutex<Option<FtpConnection>>,
}

impl FtpFileClient {
    /// 使用 Session Runtime Handle 创建客户端
    pub fn with_handle(profile: Profile, rt: tokio::runtime::Handle) -> Self {
        Self {
            profile,
            last_health_message: Mutex::new(String::from("尚未连接")),
            rt,
            cached_connection: Mutex::new(None),
        }
    }

    fn take_connection(&self) -> Result<FtpConnection> {
        self.cached_connection
            .lock()
            .map_err(|_| anyhow::anyhow!("FTP connection lock poisoned"))?
            .take()
            .context("FTP 尚未连接，请先调用 connect")
    }

    fn put_connection(&self, conn: FtpConnection) {
        if let Ok(mut guard) = self.cached_connection.lock() {
            *guard = Some(conn);
        }
    }

    fn run_async<T: Send + 'static>(
        &self,
        future: impl std::future::Future<Output = Result<T>> + Send + 'static,
    ) -> Result<T> {
        self.rt.block_on(future)
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
        let (message, connection) = self.run_async(async move {
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
            let msg = format!("{base} · cwd {cwd}{}", ftp_tls_suffix(&profile));
            Ok::<_, anyhow::Error>((msg, connection))
        })?;
        self.set_last_health_message(message.clone());
        // 缓存 FTP 连接
        self.put_connection(connection);
        debug!(target: "session", "FTP 连接已缓存");
        Ok(ConnectionHealth {
            protocol: self.profile.protocol,
            can_terminal: false,
            can_files: true,
            message,
        })
    }

    fn list(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let path = normalize_ftp_path(path);
        debug!(target: "session", path = %path, "列出 FTP 目录");
        let mut connection = self.take_connection()?;
        let (result, conn) = self.run_async(async move {
            let res = ftp_list_entries(&mut connection, Some(path.as_str())).await;
            Ok((res, connection))
        })?;
        self.put_connection(conn);
        let entries = result?;
        debug!(target: "session", count = entries.len(), "FTP 目录列表完成");
        Ok(entries)
    }

    fn stat(&self, path: &str) -> Result<RemoteEntry> {
        let path = normalize_ftp_path(path);
        debug!(target: "session", path = %path, "获取 FTP 文件信息");
        let mut connection = self.take_connection()?;
        let (result, conn) = self.run_async(async move {
            let res = ftp_stat_entry(&mut connection, &path).await;
            Ok((res, connection))
        })?;
        self.put_connection(conn);
        result
    }

    fn mkdir(&self, path: &str) -> Result<()> {
        let path = normalize_ftp_path(path);
        debug!(target: "session", path = %path, "创建 FTP 远端目录");
        let mut connection = self.take_connection()?;
        let (result, conn) = self.run_async(async move {
            let res = connection
                .mkdir(path.as_str())
                .await
                .with_context(|| format!("创建远端目录失败: {path}"));
            Ok((res, connection))
        })?;
        self.put_connection(conn);
        result
    }

    fn rename(&self, from: &str, to: &str) -> Result<()> {
        let from = normalize_ftp_path(from);
        let to = normalize_ftp_path(to);
        debug!(target: "session", from = %from, to = %to, "重命名 FTP 远端文件");
        let mut connection = self.take_connection()?;
        let (result, conn) = self.run_async(async move {
            let res = connection
                .rename(from.as_str(), to.as_str())
                .await
                .with_context(|| format!("重命名远端文件失败: {from} -> {to}"));
            Ok((res, connection))
        })?;
        self.put_connection(conn);
        result
    }

    fn remove(&self, path: &str, is_dir: bool) -> Result<()> {
        let path = normalize_ftp_path(path);
        debug!(target: "session", path = %path, is_dir, "删除 FTP 远端文件");
        let mut connection = self.take_connection()?;
        let (result, conn) = self.run_async(async move {
            let res = if is_dir {
                connection
                    .rmdir(path.as_str())
                    .await
                    .with_context(|| format!("删除远端目录失败: {path}"))
            } else {
                connection
                    .rm(path.as_str())
                    .await
                    .with_context(|| format!("删除远端文件失败: {path}"))
            };
            Ok((res, connection))
        })?;
        self.put_connection(conn);
        result
    }

    fn upload(&self, local_path: &str, remote_path: &str) -> Result<()> {
        let local_path = PathBuf::from(local_path);
        let remote_path = normalize_ftp_path(remote_path);
        let remote_display = remote_path.clone();
        let size = fs::metadata(&local_path).map(|m| m.len()).unwrap_or(0);
        debug!(target: "session", local = %local_path.display(), remote = %remote_display, size, "上传 FTP 文件");
        let bytes = fs::read(&local_path)
            .with_context(|| format!("读取本地文件失败: {}", local_path.display()))?;
        let mut connection = self.take_connection()?;
        let (written, conn) = self.run_async(async move {
            let mut reader = Cursor::new(bytes);
            let res = connection
                .put_file(remote_path.as_str(), &mut reader)
                .await
                .with_context(|| format!("上传文件失败: {remote_path}"));
            Ok((res, connection))
        })?;
        self.put_connection(conn);
        written?;
        debug!(target: "session", remote = %remote_display, "FTP 上传完成");
        Ok(())
    }

    fn download(&self, remote_path: &str, local_path: &str) -> Result<()> {
        let remote_path = normalize_ftp_path(remote_path);
        let local_path = PathBuf::from(local_path);
        let local_display = local_path.display().to_string();
        debug!(target: "session", remote = %remote_path, local = %local_display, "下载 FTP 文件");
        let mut connection = self.take_connection()?;
        let (result, conn) = self.run_async(async move {
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("创建本地目录失败: {}", parent.display()))?;
            }
            let res = connection
                .retr_bytes(remote_path.as_str())
                .await
                .with_context(|| format!("下载远端文件失败: {remote_path}"))
                .and_then(|bytes| {
                    fs::write(&local_path, bytes)
                        .with_context(|| format!("写入本地文件失败: {}", local_path.display()))
                });
            Ok((res, connection))
        })?;
        self.put_connection(conn);
        debug!(target: "session", local = %local_display, "FTP 下载完成");
        result
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
        let (handle, host_key_state) = self.run_async(async move {
            let connection = connect_ssh(profile.clone()).await?;
            Ok::<_, anyhow::Error>((connection.session, connection.host_key_state))
        })?;

        // 记录 TOFU 首次连接指纹，待调用方持久化
        if let Some(ref fingerprint) = host_key_state.tou_new_fingerprint {
            if let Ok(mut guard) = self.tou_pending.lock() {
                *guard = Some(fingerprint.clone());
            }
        }

        let message = connection_message(&self.profile, &host_key_state);
        self.set_last_health_message(message.clone());

        // 缓存 SSH 会话句柄
        if let Ok(mut guard) = self.shared_session.lock() {
            *guard = Some(Arc::new(handle));
        }

        Ok(ConnectionHealth {
            protocol: self.profile.protocol,
            can_terminal: self.profile.protocol.supports_terminal(),
            can_files: true,
            message,
        })
    }

    fn list(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let handle = self.cached_handle()?;
        let cache = Arc::clone(&self.sftp_cache);
        let requested_path = path.to_string();
        debug!(target: "session", path = %requested_path, "列出目录");
        self.run_async(async move {
            let sftp = if let Some(sftp) = cache.lock().unwrap().take() {
                sftp
            } else {
                let channel = handle
                    .channel_open_session()
                    .await
                    .context("打开 SFTP 会话通道失败")?;
                channel.request_subsystem(true, "sftp").await
                    .context("请求 SFTP 子系统失败")?;
                SftpSession::new(channel.into_stream())
                    .await
                    .context("初始化 SFTP 客户端失败")?
            };
            let path = resolve_sftp_path(&sftp, &requested_path).await?;
            let mut items: Vec<RemoteEntry> = sftp
                .read_dir(path.clone())
                .await
                .with_context(|| format!("读取远端目录失败: {path}"))?
                .map(|entry| RemoteEntry {
                    name: entry.file_name(),
                    path: entry.path(),
                    is_dir: entry.metadata().is_dir(),
                    size: entry.metadata().size.unwrap_or_default(),
                    modified_at: entry.metadata().mtime.map(|t| t as u64),
                })
                .collect();
            items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });
            debug!(target: "session", count = items.len(), "目录列表完成");
            *cache.lock().unwrap() = Some(sftp);
            Ok(items)
        })
    }

    fn stat(&self, path: &str) -> Result<RemoteEntry> {
        let handle = self.cached_handle()?;
        let cache = Arc::clone(&self.sftp_cache);
        let requested_path = path.to_string();
        debug!(target: "session", path = %requested_path, "获取文件信息");
        self.run_async(async move {
            let sftp = if let Some(sftp) = cache.lock().unwrap().take() {
                sftp
            } else {
                let channel = handle.channel_open_session().await
                    .context("打开 SFTP 会话通道失败")?;
                channel.request_subsystem(true, "sftp").await
                    .context("请求 SFTP 子系统失败")?;
                SftpSession::new(channel.into_stream()).await
                    .context("初始化 SFTP 客户端失败")?
            };
            let path = resolve_sftp_path(&sftp, &requested_path).await?;
            let metadata = sftp
                .metadata(path.clone())
                .await
                .with_context(|| format!("读取远端文件信息失败: {path}"))?;
            let entry = RemoteEntry {
                name: remote_basename(&path),
                path,
                is_dir: metadata.is_dir(),
                size: metadata.size.unwrap_or_default(),
                modified_at: metadata.mtime.map(|t| t as u64),
            };
            *cache.lock().unwrap() = Some(sftp);
            Ok(entry)
        })
    }

    fn mkdir(&self, path: &str) -> Result<()> {
        let handle = self.cached_handle()?;
        let cache = Arc::clone(&self.sftp_cache);
        let requested_path = path.to_string();
        debug!(target: "session", path = %requested_path, "创建远端目录");
        self.run_async(async move {
            let sftp = if let Some(sftp) = cache.lock().unwrap().take() {
                sftp
            } else {
                let channel = handle.channel_open_session().await
                    .context("打开 SFTP 会话通道失败")?;
                channel.request_subsystem(true, "sftp").await
                    .context("请求 SFTP 子系统失败")?;
                SftpSession::new(channel.into_stream()).await
                    .context("初始化 SFTP 客户端失败")?
            };
            let path = resolve_sftp_parented_path(&sftp, &requested_path).await?;
            sftp.create_dir(path.clone())
                .await
                .with_context(|| format!("创建远端目录失败: {path}"))?;
            debug!(target: "session", path = %path, "远端目录已创建");
            *cache.lock().unwrap() = Some(sftp);
            Ok(())
        })
    }

    fn rename(&self, from: &str, to: &str) -> Result<()> {
        let handle = self.cached_handle()?;
        let cache = Arc::clone(&self.sftp_cache);
        let from_requested = from.to_string();
        let to_requested = to.to_string();
        debug!(target: "session", from = %from_requested, to = %to_requested, "重命名远端文件");
        self.run_async(async move {
            let sftp = if let Some(sftp) = cache.lock().unwrap().take() {
                sftp
            } else {
                let channel = handle.channel_open_session().await
                    .context("打开 SFTP 会话通道失败")?;
                channel.request_subsystem(true, "sftp").await
                    .context("请求 SFTP 子系统失败")?;
                SftpSession::new(channel.into_stream()).await
                    .context("初始化 SFTP 客户端失败")?
            };
            let from = resolve_sftp_path(&sftp, &from_requested).await?;
            let to = resolve_sftp_parented_path(&sftp, &to_requested).await?;
            sftp.rename(from.clone(), to.clone())
                .await
                .with_context(|| format!("重命名远端文件失败: {from} -> {to}"))?;
            *cache.lock().unwrap() = Some(sftp);
            Ok(())
        })
    }

    fn remove(&self, path: &str, is_dir: bool) -> Result<()> {
        let handle = self.cached_handle()?;
        let cache = Arc::clone(&self.sftp_cache);
        let requested_path = path.to_string();
        debug!(target: "session", path = %requested_path, is_dir, "删除远端文件");
        self.run_async(async move {
            let sftp = if let Some(sftp) = cache.lock().unwrap().take() {
                sftp
            } else {
                let channel = handle.channel_open_session().await
                    .context("打开 SFTP 会话通道失败")?;
                channel.request_subsystem(true, "sftp").await
                    .context("请求 SFTP 子系统失败")?;
                SftpSession::new(channel.into_stream()).await
                    .context("初始化 SFTP 客户端失败")?
            };
            let path = resolve_sftp_path(&sftp, &requested_path).await?;
            if is_dir {
                sftp.remove_dir(path.clone())
                    .await
                    .with_context(|| format!("删除远端目录失败: {path}"))?;
            } else {
                sftp.remove_file(path.clone())
                    .await
                    .with_context(|| format!("删除远端文件失败: {path}"))?;
            }
            *cache.lock().unwrap() = Some(sftp);
            Ok(())
        })
    }

    fn upload(&self, local_path: &str, remote_path: &str) -> Result<()> {
        let handle = self.cached_handle()?;
        let cache = Arc::clone(&self.sftp_cache);
        let local_path = PathBuf::from(local_path);
        let requested_remote_path = remote_path.to_string();
        let size = fs::metadata(&local_path).map(|m| m.len()).unwrap_or(0);
        debug!(target: "session", local = %local_path.display(), remote = %requested_remote_path, size, "上传文件");
        self.run_async(async move {
            let sftp = if let Some(sftp) = cache.lock().unwrap().take() {
                sftp
            } else {
                let channel = handle.channel_open_session().await
                    .context("打开 SFTP 会话通道失败")?;
                channel.request_subsystem(true, "sftp").await
                    .context("请求 SFTP 子系统失败")?;
                SftpSession::new(channel.into_stream()).await
                    .context("初始化 SFTP 客户端失败")?
            };
            let remote_path =
                resolve_sftp_parented_path(&sftp, &requested_remote_path).await?;
            let bytes = fs::read(&local_path)
                .with_context(|| format!("读取本地文件失败: {}", local_path.display()))?;
            let mut remote = sftp
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
            debug!(target: "session", remote = %remote_path, "上传完成");
            *cache.lock().unwrap() = Some(sftp);
            Ok(())
        })
    }

    fn download(&self, remote_path: &str, local_path: &str) -> Result<()> {
        let handle = self.cached_handle()?;
        let cache = Arc::clone(&self.sftp_cache);
        let requested_remote_path = remote_path.to_string();
        let local_path = PathBuf::from(local_path);
        debug!(target: "session", remote = %requested_remote_path, local = %local_path.display(), "下载文件");
        self.run_async(async move {
            let sftp = if let Some(sftp) = cache.lock().unwrap().take() {
                sftp
            } else {
                let channel = handle.channel_open_session().await
                    .context("打开 SFTP 会话通道失败")?;
                channel.request_subsystem(true, "sftp").await
                    .context("请求 SFTP 子系统失败")?;
                SftpSession::new(channel.into_stream()).await
                    .context("初始化 SFTP 客户端失败")?
            };
            let remote_path = resolve_sftp_path(&sftp, &requested_remote_path).await?;
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("创建本地目录失败: {}", parent.display()))?;
            }
            let bytes = sftp
                .read(remote_path.clone())
                .await
                .with_context(|| format!("下载远端文件失败: {remote_path}"))?;
            fs::write(&local_path, bytes)
                .with_context(|| format!("写入本地文件失败: {}", local_path.display()))?;
            debug!(target: "session", local = %local_path.display(), "下载完成");
            *cache.lock().unwrap() = Some(sftp);
            Ok(())
        })
    }

    fn tou_pending_fingerprint(&self) -> Option<String> {
        self.tou_pending.lock().ok().and_then(|g| g.clone())
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
    /// TOFU 首次连接时记录的新指纹，需持久化到数据库
    tou_new_fingerprint: Option<String>,
}

#[derive(Clone)]
pub(crate) struct HostKeyHandler {
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
            SshHostKeyPolicy::TrustOnFirstUse => {
                let expected = self.profile.security.pinned_host_key.trim();
                if expected.is_empty() {
                    self.record(|state| {
                        state.tou_new_fingerprint = Some(fingerprint.clone());
                    });
                    Ok(true)
                } else {
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
    let endpoint = profile.endpoint();
    debug!(target: "ssh", endpoint = %endpoint, protocol = %profile.protocol.as_str(), "SSH 开始连接");

    let state = Arc::new(Mutex::new(HostKeyCheckState::default()));
    let handler = HostKeyHandler::new(profile.clone(), Arc::clone(&state));
    let config = Arc::new(client::Config {
        inactivity_timeout: None, // 不因空闲断开连接；使用 keepalive 保持连接
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });

    debug!(target: "ssh", endpoint = %endpoint, "建立 TCP 连接并完成密钥交换");
    let connect_timeout = Duration::from_secs(profile.limits.connect_timeout_secs.max(5) as u64);
    let mut session: Handle<HostKeyHandler> = tokio_timeout(
        connect_timeout,
        client::connect(config, (profile.host.as_str(), profile.port), handler),
    )
    .await
    .map_err(|_: tokio::time::error::Elapsed| {
        anyhow::anyhow!("连接 {} 超时（{} 秒）", endpoint, connect_timeout.as_secs())
    })?
    .map_err(|e| anyhow::anyhow!("无法连接 {}: {e}", endpoint))?;

    let fingerprint = state.lock().map(|s| s.fingerprint_sha256.clone()).unwrap_or_default();
    debug!(target: "ssh", endpoint = %endpoint, fingerprint = ?fingerprint, "SSH 密钥交换完成，开始认证");

    let auth_timeout = Duration::from_secs(profile.limits.connect_timeout_secs.max(10) as u64);
    tokio_timeout(auth_timeout, authenticate_session(&profile, &mut session))
        .await
        .map_err(|_: tokio::time::error::Elapsed| {
            anyhow::anyhow!("SSH 认证超时（{} 秒）", auth_timeout.as_secs())
        })??;

    let host_key_state = state.lock().map(|value| value.clone()).unwrap_or_default();
    debug!(target: "ssh", endpoint = %endpoint, "SSH 认证成功，连接已建立");
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
    let endpoint = profile.endpoint();
    let timeout = Duration::from_secs(profile.limits.connect_timeout_secs as u64);

    debug!(target: "ftp", endpoint = %endpoint, protocol = %profile.protocol.as_str(), "FTP 开始连接");

    let mut connection = match profile.protocol {
        RemoteProtocol::Ftp => {
            debug!(target: "ftp", endpoint = %endpoint, "解析 FTP 主机地址");
            let socket_addr = tokio::net::lookup_host(address.as_str())
                .await
                .map_err(|e| anyhow::anyhow!("无法解析 {}: {e}", endpoint))?
                .next()
                .ok_or_else(|| anyhow::anyhow!("未找到远端地址: {}", endpoint))?;
            debug!(target: "ftp", socket_addr = %socket_addr, "TCP 连接 FTP");
            let stream = AsyncFtpStream::connect_timeout(socket_addr, timeout)
                .await
                .map_err(|e| anyhow::anyhow!("无法连接 FTP {}: {e}", endpoint))?;
            FtpConnection::Plain(stream)
        }
        RemoteProtocol::FtpsExplicit => {
            debug!(target: "ftp", endpoint = %endpoint, "解析 FTPS 主机地址");
            let socket_addr = tokio::net::lookup_host(address.as_str())
                .await
                .map_err(|e| anyhow::anyhow!("无法解析 {}: {e}", endpoint))?
                .next()
                .ok_or_else(|| anyhow::anyhow!("未找到远端地址: {}", endpoint))?;
            debug!(target: "ftp", socket_addr = %socket_addr, "TCP 连接 FTPS (显式)");
            let stream = AsyncRustlsFtpStream::connect_timeout(socket_addr, timeout)
                .await
                .map_err(|e| anyhow::anyhow!("无法连接 FTPS {}: {e}", endpoint))?;
            let connector = ftp_tls_connector(&profile)?;
            debug!(target: "ftp", "升级到 TLS");
            let stream = stream
                .into_secure(connector, profile.host.as_str())
                .await
                .map_err(|e| anyhow::anyhow!("FTPS {} TLS 升级失败: {e}", endpoint))?;
            FtpConnection::Secure(stream)
        }
        RemoteProtocol::FtpsImplicit => {
            debug!(target: "ftp", endpoint = %endpoint, "连接 FTPS (隐式 TLS)");
            let connector = ftp_tls_connector(&profile)?;
            let stream = AsyncRustlsFtpStream::connect_secure_implicit(
                address.as_str(),
                connector,
                profile.host.as_str(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("无法连接 FTPS {} (隐式TLS): {e}", endpoint))?;
            FtpConnection::Secure(stream)
        }
        RemoteProtocol::Ssh | RemoteProtocol::Sftp => {
            error!(target: "ftp", "协议类型不匹配，期望 FTP/FTPS");
            bail!("当前 profile 不是 FTP/FTPS 协议")
        }
    };

    let username = profile.auth.username.trim();
    if username.is_empty() {
        error!(target: "ftp", "用户名为空");
        bail!("用户名不能为空");
    }
    if profile.auth.method != AuthMethod::Password {
        error!(target: "ftp", method = %profile.auth.method.as_str(), "FTP 仅支持密码认证");
        bail!("FTP/FTPS 当前仅支持密码认证");
    }

    debug!(target: "ftp", username = %username, "FTP 登录中");
    let login_timeout = Duration::from_secs(profile.limits.connect_timeout_secs.max(15) as u64);
    tokio_timeout(login_timeout, connection.login(username, profile.auth.password.as_str()))
        .await
        .map_err(|_: tokio::time::error::Elapsed| {
            anyhow::anyhow!("FTP {} 登录超时（{} 秒）", endpoint, login_timeout.as_secs())
        })?
        .map_err(|e| anyhow::anyhow!("FTP {} 登录失败 ({}): {e}", endpoint, username))?;
    debug!(target: "ftp", "FTP 登录成功，切换到二进制模式");
    connection
        .transfer_type(FileType::Binary)
        .await
        .context("切换 FTP 二进制传输模式失败")?;
    connection.set_mode(if profile.limits.passive_mode {
        Mode::Passive
    } else {
        Mode::Active
    });

    debug!(target: "ftp", endpoint = %endpoint, "FTP 连接建立完成");
    Ok(connection)
}

async fn authenticate_session(
    profile: &Profile,
    session: &mut Handle<HostKeyHandler>,
) -> Result<()> {
    let username = profile.auth.username.trim();
    if username.is_empty() {
        error!(target: "ssh", "用户名为空");
        bail!("用户名不能为空");
    }

    debug!(target: "ssh", username = %username, method = %profile.auth.method.as_str(), "SSH 开始认证");

    let success = match profile.auth.method {
        AuthMethod::Password => {
            debug!(target: "ssh", username = %username, host = %profile.host, port = profile.port, "尝试密码认证");
            let auth_result = session
                .authenticate_password(username, profile.auth.password.clone())
                .await;
            match auth_result {
                Ok(result) => result.success(),
                Err(e) => {
                    error!(target: "ssh", username = %username, host = %profile.host, error = %e, "密码认证协议错误");
                    bail!("{} 在 {}:{} 密码认证出错: {e}", username, profile.host, profile.port);
                }
            }
        }
        AuthMethod::PrivateKey => {
            let path = profile.auth.private_key_path.trim();
            if path.is_empty() {
                error!(target: "ssh", username = %username, "未配置私钥路径");
                bail!("未配置私钥路径");
            }
            debug!(target: "ssh", username = %username, key_path = %path, "尝试私钥认证");
            let password = (!profile.auth.private_key_passphrase.trim().is_empty())
                .then_some(profile.auth.private_key_passphrase.as_str());
            let private_key = keys::load_secret_key(path, password)
                .with_context(|| format!("加载私钥失败: {path}"))?;

            let hash_algs = session.best_supported_rsa_hash().await?;

            if let Some(hash_alg) = hash_algs.flatten() {
                debug!(target: "ssh", hash_alg = ?hash_alg, "使用推荐的 RSA 哈希算法");
                let auth_result = session
                    .authenticate_publickey(
                        username,
                        PrivateKeyWithHashAlg::new(Arc::new(private_key), Some(hash_alg)),
                    )
                    .await;
                match auth_result {
                    Ok(result) => result.success(),
                    Err(e) => {
                        error!(target: "ssh", username = %username, host = %profile.host, key = %path, error = %e, "私钥认证协议错误");
                        bail!("{} 在 {}:{} 私钥认证出错 ({e})", username, profile.host, profile.port);
                    }
                }
            } else {
                debug!(target: "ssh", username = %username, "服务器未推荐 RSA 哈希算法，尝试回退");
                let fallback_algs: Vec<Option<keys::HashAlg>> = vec![
                    Some(keys::HashAlg::Sha256),
                    Some(keys::HashAlg::Sha512),
                    None,
                ];
                let mut last_err: Option<String> = None;
                for alg in &fallback_algs {
                    debug!(target: "ssh", hash_alg = ?alg, "尝试回退哈希算法");
                    match session
                        .authenticate_publickey(
                            username,
                            PrivateKeyWithHashAlg::new(Arc::new(private_key.clone()), *alg),
                        )
                        .await
                    {
                        Ok(result) => {
                            if result.success() {
                                debug!(target: "ssh", username = %username, "私钥认证成功");
                                return Ok(());
                            }
                            last_err = Some("服务器拒绝认证".to_string());
                            debug!(target: "ssh", username = %username, "服务器拒绝此密钥");
                        }
                        Err(e) => {
                            last_err = Some(format!("{e}"));
                            debug!(target: "ssh", username = %username, error = %e, "私钥认证尝试失败");
                        }
                    }
                }
                let msg = last_err.unwrap_or_else(|| "所有 RSA 签名算法均失败".to_string());
                error!(target: "ssh", username = %username, error = %msg, "私钥认证最终失败");
                bail!("SSH 私钥认证失败: {msg}");
            }
        }
        AuthMethod::Agent => {
            error!(target: "ssh", "SSH Agent 认证尚未实现");
            bail!("当前版本暂未接入 SSH Agent 认证")
        }
    };

    if !success {
        let method_label = profile.auth.method.label();
        let msg = format!(
            "{} 在 {}:{} 被服务器拒绝（{}认证未通过）",
            username, profile.host, profile.port, method_label
        );
        error!(target: "ssh", username = %username, host = %profile.host, port = profile.port, method = %profile.auth.method.as_str(), "远端认证被拒绝");
        bail!("{msg}");
    }

    debug!(target: "ssh", username = %username, "SSH 认证成功");
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
            modified_at: None,
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
    let modified_at = item.modified()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs());
    RemoteEntry {
        path: join_ftp_path(base, &name),
        name,
        is_dir: item.is_directory(),
        size: item.size() as u64,
        modified_at,
    }
}

fn ftp_remote_entry_exact(path: &str, item: FtpListFile) -> RemoteEntry {
    let modified_at = item.modified()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs());
    RemoteEntry {
        name: remote_basename(path),
        path: path.to_string(),
        is_dir: item.is_directory(),
        size: item.size() as u64,
        modified_at,
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

pub(crate) fn remote_parent_dir(path: &str) -> String {
    let normalized = normalize_remote_path(path);
    if normalized == "/" || normalized == "~" {
        return normalized;
    }
    if let Some(rest) = normalized.strip_prefix("~/") {
        let mut segments: Vec<&str> = rest.split('/').filter(|part| !part.is_empty()).collect();
        let _ = segments.pop();
        if segments.is_empty() {
            return String::from("~");
        }
        return format!("~/{}", segments.join("/"));
    }
    let mut parts: Vec<&str> = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
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
