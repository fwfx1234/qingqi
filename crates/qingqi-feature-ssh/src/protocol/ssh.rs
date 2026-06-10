//! SSH 协议实现（russh 0.54）

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use russh::client::{self, AuthResult};
use russh::{ChannelId, keys};
use russh_sftp::client::SftpSession;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::mpsc;
use tracing::debug;

use super::{ProtocolCapability, RemoteProtocol, TerminalOutput, TransferProgress};
use crate::log_util::bytes_preview;
use crate::model::{AuthConfig, Profile, RemoteEntry, SshAuthMethod, SshRole};

#[derive(Clone)]
struct Handler {
    state: Arc<Mutex<KeyState>>,
    pty_output: Arc<Mutex<Option<mpsc::UnboundedSender<TerminalOutput>>>>,
    shell_channel: Arc<TokioMutex<Option<ChannelId>>>,
    role: SshRole,
}

#[derive(Default)]
struct KeyState {
    fingerprint: Option<String>,
}

impl client::Handler for Handler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fpr = format!("{}", server_public_key.fingerprint(keys::HashAlg::Sha256));
        debug!(
            target: "qingqi_ssh",
            fingerprint = %fpr,
            algo = ?server_public_key.algorithm(),
            "russh: 服务端 host key"
        );
        if let Ok(mut s) = self.state.lock() {
            s.fingerprint = Some(fpr);
        }
        Ok(true)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        debug!(
            target: "qingqi_ssh",
            channel = ?channel,
            bytes = data.len(),
            preview = %bytes_preview(data, 120),
            "russh: channel data"
        );
        if self.role != SshRole::Terminal {
            return Ok(());
        }
        let is_shell = self
            .shell_channel
            .try_lock()
            .ok()
            .and_then(|g| *g)
            .is_some_and(|id| id == channel);
        if !is_shell {
            return Ok(());
        }
        if let Ok(guard) = self.pty_output.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(TerminalOutput::PtyOutput(data.to_vec()));
            }
        }
        Ok(())
    }

    async fn channel_open_confirmation(
        &mut self,
        channel: ChannelId,
        max_packet_size: u32,
        window_size: u32,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        debug!(
            target: "qingqi_ssh",
            channel = ?channel,
            max_packet_size,
            window_size,
            "russh: channel open confirmation"
        );
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        debug!(target: "qingqi_ssh", channel = ?channel, "russh: channel EOF");
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        debug!(target: "qingqi_ssh", channel = ?channel, "russh: channel close");
        Ok(())
    }
}

pub struct SshProtocol {
    role: SshRole,
    host: String,
    port: u16,
    auth: AuthConfig,
    handle: TokioMutex<Option<client::Handle<Handler>>>,
    sftp: TokioMutex<Option<SftpSession>>,
    home_dir: TokioMutex<Option<String>>,
    last_list_path: TokioMutex<Option<String>>,
    pty_output: Arc<Mutex<Option<mpsc::UnboundedSender<TerminalOutput>>>>,
    pty_writer: TokioMutex<Option<russh::Channel<client::Msg>>>,
    shell_channel: Arc<TokioMutex<Option<ChannelId>>>,
}

impl SshProtocol {
    pub fn new(profile: &Profile, role: SshRole) -> Result<Self> {
        Ok(Self {
            role,
            host: profile.host.clone(),
            port: profile.port,
            auth: profile.auth.clone(),
            handle: TokioMutex::new(None),
            sftp: TokioMutex::new(None),
            home_dir: TokioMutex::new(None),
            last_list_path: TokioMutex::new(None),
            pty_output: Arc::new(Mutex::new(None)),
            pty_writer: TokioMutex::new(None),
            shell_channel: Arc::new(TokioMutex::new(None)),
        })
    }

    fn ssh_username(&self) -> Result<String> {
        match &self.auth {
            AuthConfig::Ssh { username, .. } => Ok(username.clone()),
            _ => bail!("SSH 协议需要 SSH 认证"),
        }
    }

    async fn home_directory(&self) -> Result<String> {
        if let Some(home) = self.home_dir.lock().await.clone() {
            return Ok(home);
        }
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        let home = match sftp.canonicalize(".").await {
            Ok(path) => path,
            Err(e) => {
                let username = self.ssh_username()?;
                let fallback = format!("/home/{username}");
                debug!(
                    target: "qingqi_ssh",
                    error = %e,
                    fallback = %fallback,
                    "ssh: canonicalize(\".\") 失败，使用用户名回退路径"
                );
                fallback
            }
        };
        drop(g);
        *self.home_dir.lock().await = Some(home.clone());
        Ok(home)
    }

    async fn resolve_remote_path(&self, path: &str) -> Result<String> {
        let path = path.trim();
        if path.is_empty() || path == "." {
            return self.home_directory().await;
        }
        if path == "~" {
            return self.home_directory().await;
        }
        if let Some(rest) = path.strip_prefix("~/") {
            let home = self.home_directory().await?;
            return Ok(if rest.is_empty() {
                home
            } else {
                format!("{home}/{rest}")
            });
        }
        Ok(path.to_string())
    }

    async fn ensure_sftp(&self) -> Result<()> {
        {
            let g = self.sftp.lock().await;
            if g.is_some() {
                return Ok(());
            }
        }

        debug!(target: "qingqi_ssh", "ssh: 初始化 SFTP 子系统");
        let mut h = self.handle.lock().await;
        let handle = h.as_mut().ok_or_else(|| anyhow::anyhow!("SSH 未连接"))?;

        let channel = handle
            .channel_open_session()
            .await
            .context("打开 SFTP channel 失败")?;
        debug!(target: "qingqi_ssh", channel = ?channel.id(), "ssh: SFTP channel 已打开");
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("请求 SFTP 子系统失败")?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .context("初始化 SFTP 客户端失败")?;

        let mut g = self.sftp.lock().await;
        *g = Some(sftp);
        debug!(target: "qingqi_ssh", "ssh: SFTP 子系统就绪");
        Ok(())
    }

    fn auth_method_label(method: &SshAuthMethod) -> &'static str {
        match method {
            SshAuthMethod::Password { .. } => "password",
            SshAuthMethod::PrivateKey { .. } => "private_key",
            SshAuthMethod::Agent => "agent",
        }
    }

    async fn ensure_auth_success(result: AuthResult, step: &str) -> Result<()> {
        match result {
            AuthResult::Success => {
                debug!(target: "qingqi_ssh", step, "russh: 认证成功");
                Ok(())
            }
            AuthResult::Failure {
                remaining_methods,
                partial_success,
            } => {
                debug!(
                    target: "qingqi_ssh",
                    step,
                    ?remaining_methods,
                    partial_success,
                    "russh: 认证失败"
                );
                bail!("{step} 被拒绝 (partial_success={partial_success})")
            }
        }
    }
}

#[async_trait]
impl RemoteProtocol for SshProtocol {
    async fn connect(&self) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        debug!(
            target: "qingqi_ssh",
            endpoint = %addr,
            role = ?self.role,
            "ssh: TCP 连接开始"
        );

        let handler = Handler {
            state: Arc::new(Mutex::new(KeyState::default())),
            pty_output: Arc::clone(&self.pty_output),
            shell_channel: Arc::clone(&self.shell_channel),
            role: self.role,
        };

        let config = Arc::new(client::Config::default());
        let mut handle = client::connect(config, &addr, handler)
            .await
            .with_context(|| format!("SSH 连接 {addr} 失败"))?;
        debug!(target: "qingqi_ssh", endpoint = %addr, "ssh: TCP 握手完成");

        let (username, method) = match &self.auth {
            AuthConfig::Ssh { username, method } => (username.as_str(), method),
            _ => return Err(anyhow::anyhow!("SSH 协议需要 SSH 认证")),
        };
        let method_label = Self::auth_method_label(method);
        debug!(
            target: "qingqi_ssh",
            endpoint = %addr,
            username,
            method = method_label,
            "ssh: 开始认证"
        );

        match method {
            SshAuthMethod::Password { password } => {
                let result = handle
                    .authenticate_password(username, password)
                    .await
                    .context("SSH 密码认证请求失败")?;
                Self::ensure_auth_success(result, "password").await?;
            }
            SshAuthMethod::PrivateKey { path, passphrase } => {
                debug!(target: "qingqi_ssh", key_path = %path, "ssh: 私钥认证");
                let key_data = std::fs::read_to_string(path)
                    .with_context(|| format!("读取私钥文件 {path}"))?;
                let pass = if passphrase.is_empty() {
                    None
                } else {
                    Some(passphrase.as_str())
                };
                let key =
                    keys::decode_secret_key(&key_data, pass).with_context(|| "解析私钥失败")?;
                let key_with_hash =
                    keys::PrivateKeyWithHashAlg::new(std::sync::Arc::new(key), None);
                let result = handle
                    .authenticate_publickey(username, key_with_hash)
                    .await
                    .context("SSH 公钥认证请求失败")?;
                Self::ensure_auth_success(result, "publickey").await?;
            }
            SshAuthMethod::Agent => {
                return Err(anyhow::anyhow!("SSH Agent 认证尚未实现"));
            }
        }

        debug!(target: "qingqi_ssh", endpoint = %addr, username, "ssh: 会话就绪");

        let mut g = self.handle.lock().await;
        *g = Some(handle);
        Ok(())
    }

    async fn disconnect(&self) {
        if let Ok(mut g) = self.pty_output.lock() {
            *g = None;
        }
        {
            let mut w = self.pty_writer.lock().await;
            *w = None;
        }
        {
            let mut id = self.shell_channel.lock().await;
            *id = None;
        }
        {
            let mut g = self.sftp.lock().await;
            *g = None;
        }
        {
            let mut g = self.home_dir.lock().await;
            *g = None;
        }
        {
            let mut g = self.last_list_path.lock().await;
            *g = None;
        }
        let mut g = self.handle.lock().await;
        if let Some(h) = g.take() {
            drop(h);
        }
        debug!(target: "qingqi_ssh", "ssh: 已断开");
    }

    fn is_connected(&self) -> bool {
        self.handle.try_lock().map(|g| g.is_some()).unwrap_or(false)
    }

    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::InteractiveTerminal]
    }

    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>> {
        if self.role != SshRole::Terminal {
            bail!("此连接仅用于 SFTP，不支持终端");
        }
        let (tx, rx) = mpsc::unbounded_channel();

        {
            let mut out = self.pty_output.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
            *out = Some(tx);
        }

        let mut h = self.handle.lock().await;
        let handle = h.as_mut().ok_or_else(|| anyhow::anyhow!("SSH 未连接"))?;

        let channel = handle
            .channel_open_session()
            .await
            .context("打开 SSH shell channel 失败")?;
        let channel_id = channel.id();
        debug!(target: "qingqi_ssh", channel = ?channel_id, "ssh: shell channel 已打开");

        channel
            .request_pty(true, "xterm-256color", 120, 40, 0, 0, &[])
            .await
            .context("请求 PTY 失败")?;
        debug!(target: "qingqi_ssh", channel = ?channel_id, "ssh: PTY 已请求");

        channel
            .request_shell(true)
            .await
            .context("请求 shell 失败")?;
        debug!(target: "qingqi_ssh", channel = ?channel_id, "ssh: shell 已启动");

        {
            let mut w = self.pty_writer.lock().await;
            *w = Some(channel);
        }
        {
            let mut id = self.shell_channel.lock().await;
            *id = Some(channel_id);
        }

        Ok(rx)
    }

    async fn send_terminal_input(&self, data: &[u8]) -> Result<()> {
        debug!(
            target: "qingqi_ssh",
            bytes = data.len(),
            preview = %bytes_preview(data, 64),
            "ssh: 发送终端输入"
        );
        let mut w = self.pty_writer.lock().await;
        let channel = w.as_mut().ok_or_else(|| anyhow::anyhow!("终端未打开"))?;
        let mut writer = channel.make_writer();
        tokio::io::AsyncWriteExt::write_all(&mut writer, data).await?;
        tokio::io::AsyncWriteExt::flush(&mut writer).await?;
        Ok(())
    }

    fn last_list_path(&self) -> Option<String> {
        self.last_list_path
            .try_lock()
            .ok()
            .and_then(|g| g.clone())
    }

    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        if self.role != SshRole::Sftp {
            bail!("此连接仅用于终端，不支持 SFTP");
        }
        let resolved = self.resolve_remote_path(path).await?;
        debug!(
            target: "qingqi_ssh",
            input = path,
            resolved = %resolved,
            "ssh: SFTP list_directory"
        );
        *self.last_list_path.lock().await = Some(resolved.clone());
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;

        let entries = sftp
            .read_dir(&resolved)
            .await
            .with_context(|| format!("列出目录 {resolved} 失败"))?;

        let mut result = Vec::new();
        for entry in entries {
            let ft = entry.file_type();
            let meta = entry.metadata();
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default();
            result.push(RemoteEntry {
                path: entry.path(),
                name: entry.file_name(),
                is_dir: ft.is_dir(),
                size: meta.len(),
                modified_at: modified,
            });
        }
        debug!(
            target: "qingqi_ssh",
            resolved = %resolved,
            count = result.len(),
            "ssh: SFTP list_directory 完成"
        );
        Ok(result)
    }

    async fn create_directory(&self, path: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.create_dir(path)
            .await
            .with_context(|| format!("创建目录 {path} 失败"))?;
        Ok(())
    }

    async fn rename_entry(&self, old: &str, new: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.rename(old, new).await.with_context(|| "重命名失败")?;
        Ok(())
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.remove_file(path)
            .await
            .with_context(|| format!("删除文件 {path} 失败"))?;
        Ok(())
    }

    async fn remove_directory(&self, path: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.remove_dir(path)
            .await
            .with_context(|| format!("删除目录 {path} 失败"))?;
        Ok(())
    }

    async fn upload_file(
        &self,
        local: &Path,
        remote: &str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        self.ensure_sftp().await?;
        let file_size = std::fs::metadata(local)
            .with_context(|| format!("读取文件 {} 元数据失败", local.display()))?
            .len();
        let file_data = tokio::fs::read(local)
            .await
            .with_context(|| format!("读取文件 {} 失败", local.display()))?;

        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;

        let mut remote_file = sftp
            .create(remote)
            .await
            .with_context(|| format!("创建远程文件 {remote} 失败"))?;

        use tokio::io::AsyncWriteExt;
        let chunk_size = 65536;
        let mut written: u64 = 0;
        let start = std::time::Instant::now();

        for chunk in file_data.chunks(chunk_size) {
            remote_file.write_all(chunk).await?;
            written += chunk.len() as u64;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                written as f64 / elapsed
            } else {
                0.0
            };
            let _ = progress_tx.send(TransferProgress {
                transferred_bytes: written,
                total_bytes: file_size,
                speed_bytes_per_sec: speed,
            });
        }
        remote_file.flush().await?;
        Ok(())
    }

    async fn download_file(
        &self,
        remote: &str,
        local: &Path,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;

        let mut remote_file = sftp
            .open(remote)
            .await
            .with_context(|| format!("打开远程文件 {remote} 失败"))?;

        use tokio::io::AsyncReadExt;
        let mut data = Vec::new();
        let mut buf = vec![0u8; 65536];
        let mut downloaded: u64 = 0;
        let start = std::time::Instant::now();

        loop {
            let n = remote_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            downloaded += n as u64;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                downloaded as f64 / elapsed
            } else {
                0.0
            };
            let _ = progress_tx.send(TransferProgress {
                transferred_bytes: downloaded,
                total_bytes: downloaded.max(1),
                speed_bytes_per_sec: speed,
            });
        }

        tokio::fs::write(local, &data)
            .await
            .with_context(|| format!("写入文件 {} 失败", local.display()))?;
        Ok(())
    }
}
