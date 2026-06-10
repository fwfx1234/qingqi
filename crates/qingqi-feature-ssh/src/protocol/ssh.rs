//! SSH 协议实现（russh 0.54）

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use async_trait::async_trait;
use russh::{ChannelId, client, keys};
use russh_sftp::client::SftpSession;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::mpsc;
use tracing::debug;

use super::{ProtocolCapability, RemoteProtocol, TerminalOutput, TransferProgress};
use crate::model::{AuthConfig, Profile, RemoteEntry, SshAuthMethod};

#[derive(Clone)]
struct Handler {
    state: Arc<Mutex<KeyState>>,
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
        if let Ok(mut s) = self.state.lock() {
            s.fingerprint = Some(fpr);
        }
        Ok(true)
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

pub struct SshProtocol {
    host: String,
    port: u16,
    auth: AuthConfig,
    handle: TokioMutex<Option<client::Handle<Handler>>>,
    sftp: TokioMutex<Option<SftpSession>>,
    terminal_tx: Mutex<Option<mpsc::UnboundedSender<TerminalOutput>>>,
}

impl SshProtocol {
    pub fn new(profile: &Profile) -> Result<Self> {
        Ok(Self {
            host: profile.host.clone(),
            port: profile.port,
            auth: profile.auth.clone(),
            handle: TokioMutex::new(None),
            sftp: TokioMutex::new(None),
            terminal_tx: Mutex::new(None),
        })
    }

    async fn ensure_sftp(&self) -> Result<()> {
        {
            let g = self.sftp.lock().await;
            if g.is_some() {
                return Ok(());
            }
        }

        let mut h = self.handle.lock().await;
        let handle = h.as_mut().ok_or_else(|| anyhow::anyhow!("SSH 未连接"))?;

        let channel = handle
            .channel_open_session()
            .await
            .context("打开 SFTP channel 失败")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("请求 SFTP 子系统失败")?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .context("初始化 SFTP 客户端失败")?;

        let mut g = self.sftp.lock().await;
        *g = Some(sftp);
        Ok(())
    }
}

#[async_trait]
impl RemoteProtocol for SshProtocol {
    async fn connect(&self) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        debug!(target: "ssh", endpoint = %addr, "SSH 连接");

        let handler = Handler {
            state: Arc::new(Mutex::new(KeyState::default())),
        };

        let config = Arc::new(client::Config::default());
        let mut handle = russh::client::connect(config, &addr, handler)
            .await
            .with_context(|| format!("SSH 连接 {} 失败", addr))?;

        let username = "root"; // 简化，生产环境从 Profile 读取

        match &self.auth {
            AuthConfig::Ssh { method } => match method {
                SshAuthMethod::Password { password } => {
                    handle
                        .authenticate_password(username, password)
                        .await
                        .context("SSH 密码认证失败")?;
                }
                SshAuthMethod::PrivateKey { .. } => {
                    return Err(anyhow::anyhow!("SSH 私钥认证: russh 0.54 API 适配中，请使用密码"));
                }
                SshAuthMethod::Agent => {
                    return Err(anyhow::anyhow!("SSH Agent 认证尚未实现"));
                }
            },
            _ => return Err(anyhow::anyhow!("SSH 协议需要 SSH 认证")),
        }

        debug!(target: "ssh", endpoint = %addr, "SSH 认证成功");

        let mut g = self.handle.lock().await;
        *g = Some(handle);
        Ok(())
    }

    async fn disconnect(&self) {
        if let Ok(mut g) = self.terminal_tx.lock() {
            *g = None;
        }
        {
            let mut g = self.sftp.lock().await;
            *g = None;
        }
        let mut g = self.handle.lock().await;
        if let Some(h) = g.take() {
            drop(h); // 直接释放 Handle
        }
    }

    fn is_connected(&self) -> bool {
        self.handle.try_lock().map(|g| g.is_some()).unwrap_or(false)
    }

    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::InteractiveTerminal]
    }

    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>> {
        let mut h = self.handle.lock().await;
        let handle = h.as_mut().ok_or_else(|| anyhow::anyhow!("SSH 未连接"))?;

        let channel = handle
            .channel_open_session()
            .await
            .context("打开 SSH channel 失败")?;

        channel
            .request_pty(true, "xterm-256color", 120, 40, 0, 0, &[])
            .await
            .context("请求 PTY 失败")?;

        channel
            .request_shell(true)
            .await
            .context("请求 shell 失败")?;

        let (tx, rx) = mpsc::unbounded_channel();
        let tx2 = tx.clone();
        let mut stream = channel.into_stream();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                match tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx2.send(TerminalOutput::PtyOutput(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mut g = self.terminal_tx.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(tx);
        Ok(rx)
    }

    async fn send_terminal_input(&self, _data: &[u8]) -> Result<()> {
        Err(anyhow::anyhow!("终端输入通道待实现（需保存 PTY writer）"))
    }

    // ===== SFTP =====

    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;

        let entries = sftp.read_dir(path).await
            .with_context(|| format!("列出目录 {path} 失败"))?;

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
        Ok(result)
    }

    async fn create_directory(&self, path: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.create_dir(path).await
            .with_context(|| format!("创建目录 {path} 失败"))?;
        Ok(())
    }

    async fn rename_entry(&self, old: &str, new: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.rename(old, new).await
            .with_context(|| "重命名失败")?;
        Ok(())
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.remove_file(path).await
            .with_context(|| format!("删除文件 {path} 失败"))?;
        Ok(())
    }

    async fn remove_directory(&self, path: &str) -> Result<()> {
        self.ensure_sftp().await?;
        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;
        sftp.remove_dir(path).await
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
        let file_data = tokio::fs::read(local).await
            .with_context(|| format!("读取文件 {} 失败", local.display()))?;

        let g = self.sftp.lock().await;
        let sftp = g.as_ref().ok_or_else(|| anyhow::anyhow!("SFTP 未初始化"))?;

        let mut remote_file = sftp.create(remote).await
            .with_context(|| format!("创建远程文件 {remote} 失败"))?;

        use tokio::io::AsyncWriteExt;
        let chunk_size = 65536;
        let mut written: u64 = 0;
        let start = std::time::Instant::now();

        for chunk in file_data.chunks(chunk_size) {
            remote_file.write_all(chunk).await?;
            written += chunk.len() as u64;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { written as f64 / elapsed } else { 0.0 };
            let _ = progress_tx.send(TransferProgress { transferred_bytes: written, total_bytes: file_size, speed_bytes_per_sec: speed });
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

        let mut remote_file = sftp.open(remote).await
            .with_context(|| format!("打开远程文件 {remote} 失败"))?;

        use tokio::io::AsyncReadExt;
        let mut data = Vec::new();
        let mut buf = vec![0u8; 65536];
        let mut downloaded: u64 = 0;
        let start = std::time::Instant::now();

        loop {
            let n = remote_file.read(&mut buf).await?;
            if n == 0 { break; }
            data.extend_from_slice(&buf[..n]);
            downloaded += n as u64;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { downloaded as f64 / elapsed } else { 0.0 };
            let _ = progress_tx.send(TransferProgress { transferred_bytes: downloaded, total_bytes: downloaded.max(1), speed_bytes_per_sec: speed });
        }

        tokio::fs::write(local, &data).await
            .with_context(|| format!("写入文件 {} 失败", local.display()))?;
        Ok(())
    }
}
