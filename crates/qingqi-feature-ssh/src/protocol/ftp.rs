//! FTP/FTPS 协议实现（suppaftp 8.0.3）
//!
//! 使用 tokio::sync::Mutex 允许跨 .await 持有流引用。

use std::path::Path;

use anyhow::{Context, Result};
use async_trait::async_trait;
use suppaftp::{tokio::AsyncFtpStream, types::FileType};
use tokio::sync::{Mutex, mpsc};
use tracing::debug;

use super::{LogLevel, ProtocolCapability, RemoteProtocol, TerminalOutput, TransferProgress};
use crate::model::{AuthConfig, Profile, ProtocolType, RemoteEntry};

pub struct FtpProtocol {
    host: String,
    port: u16,
    username: String,
    password: String,
    use_tls: bool,
    stream: Mutex<Option<AsyncFtpStream>>,
    log_tx: std::sync::Mutex<Option<mpsc::UnboundedSender<TerminalOutput>>>,
}

impl FtpProtocol {
    pub fn new(profile: &Profile) -> Result<Self> {
        let (username, password) = match &profile.auth {
            AuthConfig::Ftp { username, password } => (username.clone(), password.clone()),
            _ => return Err(anyhow::anyhow!("FTP 协议需要 FTP 认证")),
        };
        Ok(Self {
            host: profile.host.clone(),
            port: profile.port,
            username,
            password,
            use_tls: matches!(profile.protocol, ProtocolType::Ftps),
            stream: Mutex::new(None),
            log_tx: std::sync::Mutex::new(None),
        })
    }

    fn log(&self, level: LogLevel, text: &str) {
        if let Ok(g) = self.log_tx.lock()
            && let Some(tx) = g.as_ref()
        {
            let _ = tx.send(TerminalOutput::LogLine {
                level,
                text: text.into(),
            });
        }
    }
}

#[async_trait]
impl RemoteProtocol for FtpProtocol {
    async fn connect(&self) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        debug!(target: "qingqi_ssh", endpoint = %addr, tls = self.use_tls, "ftp: 连接开始");
        self.log(LogLevel::Info, &format!("CONNECT {}", addr));

        let mut stream = AsyncFtpStream::connect(&addr)
            .await
            .with_context(|| format!("FTP 连接 {} 失败", addr))?;

        let welcome = stream.get_welcome_msg().unwrap_or("").to_string();
        self.log(LogLevel::Received, &welcome);

        if self.use_tls {
            self.log(LogLevel::Info, "FTPS: TLS 待完善");
        }

        self.log(LogLevel::Sent, &format!("USER {}", self.username));
        stream
            .login(&self.username, &self.password)
            .await
            .map_err(|e| anyhow::anyhow!("FTP 登录: {e}"))?;
        self.log(LogLevel::Received, "230 Login OK");

        stream
            .transfer_type(FileType::Binary)
            .await
            .map_err(|e| anyhow::anyhow!("Binary: {e}"))?;

        let mut g = self.stream.lock().await;
        *g = Some(stream);
        Ok(())
    }

    async fn disconnect(&self) {
        let s = {
            let mut g = self.stream.lock().await;
            g.take()
        };
        if let Some(mut stream) = s {
            self.log(LogLevel::Sent, "QUIT");
            let _ = stream.quit().await;
        }
    }

    fn is_connected(&self) -> bool {
        self.stream.try_lock().map(|g| g.is_some()).unwrap_or(false)
    }

    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::LogTerminal]
    }

    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut g = self.log_tx.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(tx);
        Ok(rx)
    }

    async fn send_terminal_input(&self, data: &[u8]) -> Result<()> {
        let cmd = String::from_utf8_lossy(data).trim().to_string();
        if cmd.is_empty() {
            return Ok(());
        }
        self.log(LogLevel::Sent, &cmd);

        let response = match cmd.to_uppercase().as_str() {
            "PWD" => {
                let mut g = self.stream.lock().await;
                let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
                s.pwd()
                    .await
                    .map(|d| format!("257 \"{d}\""))
                    .unwrap_or_else(|e| format!("ERROR: {e}"))
            }
            "NOOP" => "200 OK".into(),
            "HELP" => "214 支持: PWD, NOOP, HELP".into(),
            _ => format!("不支持: {cmd}"),
        };
        self.log(LogLevel::Received, &response);
        Ok(())
    }

    // ===== 文件操作 =====

    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let mut g = self.stream.lock().await;
        let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        self.log(LogLevel::Sent, &format!("LIST {}", path));

        let lines = s
            .list(Some(path))
            .await
            .with_context(|| format!("LIST {} 失败", path))?;

        let mut result = Vec::new();
        for line in &lines {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }
            let is_dir = line.starts_with('d');
            let name = parts[8..].join(" ");
            if name == "." || name == ".." {
                continue;
            }
            let size: u64 = parts[4].parse().unwrap_or(0);
            result.push(RemoteEntry {
                path: format!("{}/{}", path.trim_end_matches('/'), name),
                name,
                is_dir,
                size,
                modified_at: parts[5..8].join(" "),
            });
        }
        self.log(LogLevel::Received, &format!("{} 个条目", result.len()));
        Ok(result)
    }

    async fn create_directory(&self, path: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("MKD {}", path));
        let mut g = self.stream.lock().await;
        let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        s.mkdir(path)
            .await
            .map_err(|e| anyhow::anyhow!("MKD: {e}"))?;
        self.log(LogLevel::Received, "257 Created");
        Ok(())
    }

    async fn rename_entry(&self, old: &str, new: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("RNFR {} -> {}", old, new));
        let mut g = self.stream.lock().await;
        let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        s.rename(old, new)
            .await
            .map_err(|e| anyhow::anyhow!("RNFR: {e}"))?;
        self.log(LogLevel::Received, "250 OK");
        Ok(())
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("DELE {}", path));
        let mut g = self.stream.lock().await;
        let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        s.rm(path).await.map_err(|e| anyhow::anyhow!("DELE: {e}"))?;
        self.log(LogLevel::Received, "250 Deleted");
        Ok(())
    }

    async fn remove_directory(&self, path: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("RMD {}", path));
        let mut g = self.stream.lock().await;
        let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        s.rmdir(path)
            .await
            .map_err(|e| anyhow::anyhow!("RMD: {e}"))?;
        self.log(LogLevel::Received, "250 Removed");
        Ok(())
    }

    async fn upload_file(
        &self,
        local: &Path,
        remote: &str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        self.log(LogLevel::Sent, &format!("STOR {}", remote));
        let data = tokio::fs::read(local)
            .await
            .with_context(|| format!("读取文件 {} 失败", local.display()))?;
        let size = data.len() as u64;

        let mut g = self.stream.lock().await;
        let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;

        // 使用 put_with_stream 上传内存数据
        let mut writer = s
            .put_with_stream(remote)
            .await
            .map_err(|e| anyhow::anyhow!("STOR: {e}"))?;

        use tokio::io::AsyncWriteExt;
        writer.write_all(&data).await?;
        writer.flush().await?;
        drop(writer);

        let _ = progress_tx.send(TransferProgress {
            transferred_bytes: size,
            total_bytes: size,
            speed_bytes_per_sec: 0.0,
        });
        self.log(LogLevel::Received, "226 Transfer complete");
        Ok(())
    }

    async fn download_file(
        &self,
        remote: &str,
        local: &Path,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        self.log(LogLevel::Sent, &format!("RETR {}", remote));
        let mut g = self.stream.lock().await;
        let s = g.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;

        // 使用 retr_as_stream 下载
        let mut stream = s
            .retr_as_stream(remote)
            .await
            .map_err(|e| anyhow::anyhow!("下载失败: {e}"))?;

        let mut file = tokio::fs::File::create(local)
            .await
            .with_context(|| format!("创建本地文件 {} 失败", local.display()))?;

        use tokio::io::AsyncWriteExt;
        let mut buf = vec![0u8; 65536];
        let mut total: u64 = 0;
        let start = std::time::Instant::now();
        loop {
            let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).await?;
            total += n as u64;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                total as f64 / elapsed
            } else {
                0.0
            };
            let _ = progress_tx.send(TransferProgress {
                transferred_bytes: total,
                total_bytes: total.max(1),
                speed_bytes_per_sec: speed,
            });
        }

        self.log(LogLevel::Received, "226 Transfer complete");
        Ok(())
    }
}
