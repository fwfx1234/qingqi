//! FTP/FTPS 协议实现（suppaftp 8.0.3）
//!
//! 使用 tokio::sync::Mutex 允许跨 .await 持有流引用。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use suppaftp::tokio::{AsyncFtpStream, AsyncRustlsConnector, AsyncRustlsFtpStream};
use suppaftp::types::{Features, FtpError, Mode, Response};
use suppaftp::Status;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio::sync::{Mutex, mpsc};
use tokio_rustls::TlsConnector;
use tracing::debug;

use super::{
    LogLevel, ProtocolCapability, RemoteProtocol, TerminalOutput, TerminalOutputSource,
    TransferProgress,
};
use crate::model::{AuthConfig, Profile, ProfileAdvanced, ProtocolType, RemoteEntry};

enum FtpStreamHandle {
    /// 明文 FTP，或显式 FTPS（AUTH TLS 后仍为此类型）
    Standard(AsyncFtpStream),
    /// 隐式 FTPS（通常端口 990）
    ImplicitTls(AsyncRustlsFtpStream),
}

impl FtpStreamHandle {
    async fn nlst(&mut self, path: Option<&str>) -> suppaftp::types::FtpResult<Vec<String>> {
        match self {
            Self::Standard(s) => s.nlst(path).await,
            Self::ImplicitTls(s) => s.nlst(path).await,
        }
    }

    async fn mkdir(&mut self, path: &str) -> suppaftp::types::FtpResult<()> {
        match self {
            Self::Standard(s) => s.mkdir(path).await,
            Self::ImplicitTls(s) => s.mkdir(path).await,
        }
    }

    async fn rename(&mut self, old: &str, new: &str) -> suppaftp::types::FtpResult<()> {
        match self {
            Self::Standard(s) => s.rename(old, new).await,
            Self::ImplicitTls(s) => s.rename(old, new).await,
        }
    }

    async fn rm(&mut self, path: &str) -> suppaftp::types::FtpResult<()> {
        match self {
            Self::Standard(s) => s.rm(path).await,
            Self::ImplicitTls(s) => s.rm(path).await,
        }
    }

    async fn rmdir(&mut self, path: &str) -> suppaftp::types::FtpResult<()> {
        match self {
            Self::Standard(s) => s.rmdir(path).await,
            Self::ImplicitTls(s) => s.rmdir(path).await,
        }
    }

    fn apply_transfer_mode(&mut self, advanced: &ProfileAdvanced, features: Option<&Features>) {
        let mode = if !advanced.ftp_passive_mode {
            Mode::Active
        } else if features.is_some_and(|f| f.contains_key("EPSV")) {
            Mode::ExtendedPassive
        } else {
            Mode::Passive
        };
        match self {
            Self::Standard(s) => {
                s.set_mode(mode);
                s.set_passive_nat_workaround(advanced.ftp_passive_nat_workaround);
            }
            Self::ImplicitTls(s) => {
                s.set_mode(mode);
                s.set_passive_nat_workaround(advanced.ftp_passive_nat_workaround);
            }
        }
    }

    fn peer_addr(&self) -> Option<std::net::SocketAddr> {
        match self {
            Self::Standard(s) => s.get_ref().peer_addr().ok(),
            Self::ImplicitTls(s) => s.get_ref().peer_addr().ok(),
        }
    }

    async fn custom_command_logged(
        &mut self,
        log: &FtpProtocol,
        cmd: &str,
        expected: &[Status],
    ) -> Result<Response> {
        log.log_sent(cmd);
        let result = match self {
            Self::Standard(s) => s.custom_command(cmd, expected).await,
            Self::ImplicitTls(s) => s.custom_command(cmd, expected).await,
        };
        match result {
            Ok(resp) => {
                log.log_response(&resp);
                Ok(resp)
            }
            Err(err) => {
                log.log_ftp_error("suppaftp", &err);
                Err(anyhow::anyhow!("{err}"))
            }
        }
    }

    async fn pass_logged(&mut self, log: &FtpProtocol, pass: &str) -> Result<()> {
        log.log_sent("PASS ***");
        let result = match self {
            Self::Standard(s) => {
                s.custom_command(&format!("PASS {pass}"), &[Status::LoggedIn])
                    .await
            }
            Self::ImplicitTls(s) => {
                s.custom_command(&format!("PASS {pass}"), &[Status::LoggedIn])
                    .await
            }
        };
        match result {
            Ok(resp) => {
                log.log_response(&resp);
                Ok(())
            }
            Err(err) => {
                log.log_ftp_error("PASS", &err);
                Err(anyhow::anyhow!("{err}"))
            }
        }
    }

    async fn login_logged_fixed(
        &mut self,
        log: &FtpProtocol,
        user: &str,
        pass: &str,
    ) -> Result<()> {
        let user_resp = self
            .custom_command_logged(
                log,
                &format!("USER {user}"),
                &[Status::LoggedIn, Status::NeedPassword],
            )
            .await?;
        if user_resp.status == Status::NeedPassword {
            self.pass_logged(log, pass).await?;
        }
        Ok(())
    }

    async fn transfer_type_logged(&mut self, log: &FtpProtocol) -> Result<()> {
        self.custom_command_logged(log, "TYPE I", &[Status::CommandOk])
            .await
            .map(|_| ())
    }

    async fn feat_logged(&mut self, log: &FtpProtocol) -> Result<Features> {
        let result = match self {
            Self::Standard(s) => s.feat().await,
            Self::ImplicitTls(s) => s.feat().await,
        };
        match result {
            Ok(features) => {
                log.log(LogLevel::Info, &format!("FEAT: 服务器支持 {} 项能力", features.len()));
                for (name, value) in &features {
                    let line = match value {
                        Some(v) => format!("  {name} {v}"),
                        None => format!("  {name}"),
                    };
                    log.log(LogLevel::Received, &line);
                }
                Ok(features)
            }
            Err(err) => {
                log.log_ftp_error("FEAT", &err);
                Err(anyhow::anyhow!("{err}"))
            }
        }
    }

    async fn pwd_logged(&mut self, log: &FtpProtocol) -> Result<String> {
        log.log_sent("PWD");
        let result = match self {
            Self::Standard(s) => s.pwd().await,
            Self::ImplicitTls(s) => s.pwd().await,
        };
        match result {
            Ok(path) => {
                log.log(LogLevel::Received, &format!("[257] \"{path}\""));
                Ok(path)
            }
            Err(err) => {
                log.log_ftp_error("PWD", &err);
                Err(anyhow::anyhow!("{err}"))
            }
        }
    }

}

pub struct FtpProtocol {
    host: String,
    port: u16,
    username: String,
    password: String,
    use_tls: bool,
    advanced: ProfileAdvanced,
    stream: Mutex<Option<FtpStreamHandle>>,
    log_tx: std::sync::Mutex<Option<mpsc::UnboundedSender<TerminalOutput>>>,
    pending_logs: std::sync::Mutex<Vec<TerminalOutput>>,
    last_list_path: Mutex<Option<String>>,
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
            advanced: profile.advanced.clone(),
            stream: Mutex::new(None),
            log_tx: std::sync::Mutex::new(None),
            pending_logs: std::sync::Mutex::new(Vec::new()),
            last_list_path: Mutex::new(None),
        })
    }

    fn tls_connector() -> Result<AsyncRustlsConnector> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(AsyncRustlsConnector::from(TlsConnector::from(Arc::new(config))))
    }

    fn log(&self, level: LogLevel, text: &str) {
        let line = TerminalOutput::LogLine {
            level,
            text: text.into(),
        };
        if let Ok(guard) = self.log_tx.lock()
            && let Some(tx) = guard.as_ref()
        {
            let _ = tx.send(line);
            return;
        }
        if let Ok(mut pending) = self.pending_logs.lock() {
            pending.push(line);
        }
    }

    fn log_sent(&self, cmd: &str) {
        self.log(LogLevel::Sent, cmd);
    }

    fn log_response(&self, resp: &Response) {
        self.log(LogLevel::Received, &format!("{resp}"));
        if let Ok(body) = resp.as_string() {
            for line in body.lines().skip(1) {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    self.log(LogLevel::Received, trimmed);
                }
            }
        }
    }

    fn log_ftp_error(&self, context: &str, err: &FtpError) {
        self.log(LogLevel::Error, &format!("{context}: {err}"));
        if let FtpError::UnexpectedResponse(resp) = err {
            self.log_response(resp);
        }
    }

    fn log_welcome(&self, welcome: Option<&str>) {
        match welcome.filter(|s| !s.is_empty()) {
            Some(msg) => {
                for line in msg.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.log(LogLevel::Received, trimmed);
                    }
                }
            }
            None => self.log(LogLevel::Info, "suppaftp: 无欢迎消息"),
        }
    }

    async fn resolve_list_path(&self, path: &str) -> Result<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() || trimmed == "~" || trimmed == "~/" {
            let mut guard = self.stream.lock().await;
            let stream = guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
            return stream.pwd_logged(self).await;
        }
        Ok(trimmed.to_string())
    }

    fn parse_unix_list(lines: &[String], base_path: &str) -> Vec<RemoteEntry> {
        let base = base_path.trim_end_matches('/');
        lines
            .iter()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 9 {
                    return None;
                }
                let is_dir = line.starts_with('d');
                let name = parts[8..].join(" ");
                if name == "." || name == ".." {
                    return None;
                }
                let size: u64 = parts[4].parse().unwrap_or(0);
                Some(RemoteEntry {
                    path: format!("{base}/{name}"),
                    name,
                    is_dir,
                    size,
                    modified_at: parts[5..8].join(" "),
                })
            })
            .collect()
    }

    fn data_timeout(&self) -> Duration {
        let secs = self.advanced.connection_timeout_secs;
        Duration::from_secs(if secs > 0 { secs as u64 } else { 30 })
    }
}

#[async_trait]
impl RemoteProtocol for FtpProtocol {
    async fn connect(&self) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        debug!(target: "qingqi_ssh", endpoint = %addr, tls = self.use_tls, "ftp: 连接开始");

        let transfer_mode = if !self.advanced.ftp_passive_mode {
            "PORT (主动)"
        } else {
            "被动 (PASV/EPSV)"
        };
        self.log(
            LogLevel::Info,
            &format!("── suppaftp 8.0.3 │ {addr} │ 开始连接 ──"),
        );
        self.log(
            LogLevel::Info,
            &format!(
                "配置: 协议={} 端口={} 用户={} 传输={} NAT穿透={}",
                if self.use_tls { "FTPS" } else { "FTP" },
                self.port,
                self.username,
                transfer_mode,
                self.advanced.ftp_passive_nat_workaround
            ),
        );
        self.log_sent(&format!("CONNECT {addr}"));

        let mut stream = if self.use_tls {
            let connector = Self::tls_connector().context("初始化 FTPS TLS 失败")?;
            let domain = self.host.clone();
            if self.port == ProtocolType::Ftps.default_port() {
                self.log(LogLevel::Info, "suppaftp: connect_secure_implicit()");
                let result = AsyncRustlsFtpStream::connect_secure_implicit(
                    &addr,
                    connector,
                    &domain,
                )
                .await;
                let stream = match result {
                    Ok(s) => s,
                    Err(err) => {
                        self.log_ftp_error("connect_secure_implicit", &err);
                        return Err(anyhow::anyhow!("FTPS 隐式连接 {addr} 失败: {err}"));
                    }
                };
                self.log_welcome(stream.get_welcome_msg());
                FtpStreamHandle::ImplicitTls(stream)
            } else {
                self.log(LogLevel::Info, "suppaftp: connect() → into_secure()");
                let plain = AsyncRustlsFtpStream::connect(&addr).await.map_err(|err| {
                    self.log_ftp_error("connect", &err);
                    anyhow::anyhow!("FTP 连接 {addr} 失败: {err}")
                })?;
                self.log_welcome(plain.get_welcome_msg());
                self.log(LogLevel::Info, "suppaftp: into_secure(AUTH TLS, PBSZ 0, PROT P)");
                let secured = plain.into_secure(connector, &domain).await.map_err(|err| {
                    self.log_ftp_error("into_secure", &err);
                    anyhow::anyhow!("FTPS TLS 握手失败: {err}")
                })?;
                self.log(LogLevel::Info, "suppaftp: TLS 命令通道就绪");
                FtpStreamHandle::ImplicitTls(secured)
            }
        } else {
            self.log(LogLevel::Info, "suppaftp: AsyncFtpStream::connect()");
            let result = AsyncFtpStream::connect(&addr).await;
            let stream = match result {
                Ok(s) => s,
                Err(err) => {
                    self.log_ftp_error("connect", &err);
                    return Err(anyhow::anyhow!("FTP 连接 {addr} 失败: {err}"));
                }
            };
            self.log_welcome(stream.get_welcome_msg());
            FtpStreamHandle::Standard(stream)
        };

        if let Some(peer) = stream.peer_addr() {
            self.log(LogLevel::Info, &format!("TCP peer={peer}"));
        }

        stream
            .login_logged_fixed(self, &self.username, &self.password)
            .await?;
        stream.transfer_type_logged(self).await?;
        let features = stream.feat_logged(self).await.ok();
        stream.apply_transfer_mode(&self.advanced, features.as_ref());
        let data_mode = if !self.advanced.ftp_passive_mode {
            "PORT (主动)".into()
        } else if features.as_ref().is_some_and(|f| f.contains_key("EPSV")) {
            "EPSV (扩展被动)".into()
        } else {
            format!(
                "PASV (被动, NAT修正={})",
                self.advanced.ftp_passive_nat_workaround
            )
        };
        self.log(
            LogLevel::Info,
            &format!("suppaftp: 数据通道模式={data_mode}"),
        );
        self.log(LogLevel::Info, "── 连接完成 ──");

        let mut guard = self.stream.lock().await;
        *guard = Some(stream);
        Ok(())
    }

    async fn disconnect(&self) {
        let stream = {
            let mut guard = self.stream.lock().await;
            guard.take()
        };
        if let Some(mut stream) = stream {
            self.log_sent("QUIT");
            let result = match &mut stream {
                FtpStreamHandle::Standard(s) => s.quit().await,
                FtpStreamHandle::ImplicitTls(s) => s.quit().await,
            };
            if let Err(err) = result {
                self.log_ftp_error("QUIT", &err);
            } else {
                self.log(LogLevel::Received, "[221] 会话结束");
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.stream.try_lock().map(|g| g.is_some()).unwrap_or(false)
    }

    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::LogTerminal]
    }

    async fn open_terminal(&self) -> Result<TerminalOutputSource> {
        let (tx, rx) = mpsc::unbounded_channel();
        let pending = {
            let mut guard = self.log_tx.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(tx);
            std::mem::take(&mut *self.pending_logs.lock().unwrap_or_else(|e| e.into_inner()))
        };
        for line in pending {
            if let Ok(guard) = self.log_tx.lock()
                && let Some(sender) = guard.as_ref()
            {
                let _ = sender.send(line);
            }
        }
        Ok(TerminalOutputSource::Channel(rx))
    }

    async fn send_terminal_input(&self, data: &[u8]) -> Result<()> {
        let cmd = String::from_utf8_lossy(data).trim().to_string();
        if cmd.is_empty() {
            return Ok(());
        }

        let mut guard = self.stream.lock().await;
        let stream = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        match cmd.to_uppercase().as_str() {
            "PWD" => {
                stream.pwd_logged(self).await?;
            }
            "NOOP" => {
                stream
                    .custom_command_logged(self, "NOOP", &[Status::CommandOk])
                    .await?;
            }
            "FEAT" => {
                stream.feat_logged(self).await?;
            }
            "HELP" => {
                self.log(LogLevel::Received, "[214] 支持: PWD, NOOP, FEAT, HELP");
            }
            _ => {
                self.log(
                    LogLevel::Error,
                    &format!("本地终端不支持命令: {cmd}"),
                );
            }
        }
        Ok(())
    }

    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        let resolved = self.resolve_list_path(path).await?;
        *self.last_list_path.lock().await = Some(resolved.clone());

        let timeout = self.data_timeout();
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        self.log(
            LogLevel::Info,
            &format!("suppaftp: list() 数据连接 → {resolved}"),
        );
        self.log_sent(&format!("LIST {resolved}"));

        let list_future = async {
            match stream {
                FtpStreamHandle::Standard(s) => s.list(Some(&resolved)).await,
                FtpStreamHandle::ImplicitTls(s) => s.list(Some(&resolved)).await,
            }
        };
        let lines = match tokio::time::timeout(timeout, list_future).await {
            Ok(Ok(lines)) => lines,
            Ok(Err(err)) => {
                self.log_ftp_error("LIST", &err);
                return Err(anyhow::anyhow!("LIST {resolved} 失败: {err}"));
            }
            Err(_) => {
                let secs = timeout.as_secs();
                self.log(
                    LogLevel::Error,
                    &format!(
                        "LIST {resolved} 超时（{secs} 秒）：数据连接未建立，请开启「被动模式 NAT 修正」或检查防火墙"
                    ),
                );
                return Err(anyhow::anyhow!(
                    "LIST {resolved} 超时（{secs} 秒），数据连接失败"
                ));
            }
        };
        self.log(
            LogLevel::Received,
            &format!("[226] 列表完成, {} 行原始数据", lines.len()),
        );

        let mut result = Self::parse_unix_list(&lines, &resolved);
        if result.is_empty() {
            self.log(LogLevel::Info, "LIST 无 Unix 格式条目，尝试 NLST");
            let names = stream
                .nlst(Some(&resolved))
                .await
                .with_context(|| format!("NLST {} 失败", resolved))?;
            let base = resolved.trim_end_matches('/');
            result = names
                .into_iter()
                .filter(|name| name != "." && name != "..")
                .map(|name| RemoteEntry {
                    path: format!("{base}/{name}"),
                    name: name.clone(),
                    is_dir: false,
                    size: 0,
                    modified_at: String::new(),
                })
                .collect();
        }

        self.log(LogLevel::Received, &format!("{} 个条目", result.len()));
        Ok(result)
    }

    fn last_list_path(&self) -> Option<String> {
        self.last_list_path
            .try_lock()
            .ok()
            .and_then(|g| g.clone())
    }

    async fn create_directory(&self, path: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("MKD {}", path));
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        stream
            .mkdir(path)
            .await
            .map_err(|e| anyhow::anyhow!("MKD: {e}"))?;
        self.log(LogLevel::Received, "257 Created");
        Ok(())
    }

    async fn rename_entry(&self, old: &str, new: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("RNFR {} -> {}", old, new));
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        stream
            .rename(old, new)
            .await
            .map_err(|e| anyhow::anyhow!("RNFR: {e}"))?;
        self.log(LogLevel::Received, "250 OK");
        Ok(())
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("DELE {}", path));
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        stream
            .rm(path)
            .await
            .map_err(|e| anyhow::anyhow!("DELE: {e}"))?;
        self.log(LogLevel::Received, "250 Deleted");
        Ok(())
    }

    async fn remove_directory(&self, path: &str) -> Result<()> {
        self.log(LogLevel::Sent, &format!("RMD {}", path));
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        stream
            .rmdir(path)
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

        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;

        use tokio::io::AsyncWriteExt;
        match stream {
            FtpStreamHandle::Standard(s) => {
                let mut writer = s
                    .put_with_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("STOR: {e}"))?;
                writer.write_all(&data).await?;
                writer.flush().await?;
            }
            FtpStreamHandle::ImplicitTls(s) => {
                let mut writer = s
                    .put_with_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("STOR: {e}"))?;
                writer.write_all(&data).await?;
                writer.flush().await?;
            }
        }

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
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;

        let mut file = tokio::fs::File::create(local)
            .await
            .with_context(|| format!("创建本地文件 {} 失败", local.display()))?;

        use tokio::io::AsyncWriteExt;
        let mut buf = vec![0u8; 65536];
        let mut total: u64 = 0;
        let start = std::time::Instant::now();
        match stream {
            FtpStreamHandle::Standard(s) => {
                let mut reader = s
                    .retr_as_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("下载失败: {e}"))?;
                loop {
                    let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await?;
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
            }
            FtpStreamHandle::ImplicitTls(s) => {
                let mut reader = s
                    .retr_as_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("下载失败: {e}"))?;
                loop {
                    let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await?;
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
            }
        }

        self.log(LogLevel::Received, "226 Transfer complete");
        Ok(())
    }

    async fn read_file(&self, remote: &str) -> Result<Vec<u8>> {
        self.log(LogLevel::Sent, &format!("RETR {}", remote));
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        let mut data = Vec::new();
        let mut buf = vec![0u8; 65536];
        match stream {
            FtpStreamHandle::Standard(s) => {
                let mut reader = s
                    .retr_as_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("读取失败: {e}"))?;
                loop {
                    let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buf[..n]);
                }
            }
            FtpStreamHandle::ImplicitTls(s) => {
                let mut reader = s
                    .retr_as_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("读取失败: {e}"))?;
                loop {
                    let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buf[..n]);
                }
            }
        }
        self.log(LogLevel::Received, "226 Transfer complete");
        Ok(data)
    }

    async fn write_file(&self, remote: &str, data: &[u8]) -> Result<()> {
        self.log(LogLevel::Sent, &format!("STOR {}", remote));
        let mut guard = self.stream.lock().await;
        let stream = guard.as_mut().ok_or_else(|| anyhow::anyhow!("FTP 未连接"))?;
        use tokio::io::AsyncWriteExt;
        match stream {
            FtpStreamHandle::Standard(s) => {
                let mut writer = s
                    .put_with_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("写入失败: {e}"))?;
                writer.write_all(data).await?;
                writer.flush().await?;
            }
            FtpStreamHandle::ImplicitTls(s) => {
                let mut writer = s
                    .put_with_stream(remote)
                    .await
                    .map_err(|e| anyhow::anyhow!("写入失败: {e}"))?;
                writer.write_all(data).await?;
                writer.flush().await?;
            }
        }
        self.log(LogLevel::Received, "226 Transfer complete");
        Ok(())
    }
}
