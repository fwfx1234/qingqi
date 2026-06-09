use std::{
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use suppaftp::Mode;

use crate::model::{
    AuthMethod, ProtocolLogEntry, ProtocolLogKind, RemoteFileItem, RemoteProfile,
    RemoteProtocol, join_remote_path,
};

pub trait RemoteTerminal: Send + Sync {
    fn write_input(&self, text: &str) -> Result<()>;
    fn resize(&self, cols: u32, rows: u32) -> Result<()>;
    fn try_read(&self) -> Vec<String>;
    fn cwd_hint(&self) -> String;
    fn close(&self);
}

pub trait RemoteBackend: Send {
    fn connect(&mut self) -> Result<()>;
    fn close(&mut self);
    fn list_dir(&self, path: &str) -> Result<Vec<RemoteFileItem>>;
    fn mkdir(&self, path: &str) -> Result<()>;
    fn rename(&self, source: &str, target: &str) -> Result<()>;
    fn delete_file(&self, path: &str) -> Result<()>;
    fn delete_dir(&self, path: &str) -> Result<()>;
    fn upload_file(
        &self,
        local: &Path,
        remote: &str,
        progress: &dyn Fn(usize, usize),
    ) -> Result<()>;
    fn download_file(
        &self,
        remote: &str,
        local: &Path,
        progress: &dyn Fn(usize, usize),
    ) -> Result<()>;
    fn home_dir(&self) -> Result<String>;
    fn open_terminal(&self) -> Result<Arc<dyn RemoteTerminal>> {
        Err(anyhow!("当前连接不支持终端"))
    }
    fn protocol_log_snapshot(&self) -> Vec<ProtocolLogEntry> {
        Vec::new()
    }
    fn clear_protocol_log(&self) {}
    fn remote_file_version(&self, remote: &str) -> Result<String>;
}

#[derive(Clone)]
struct ProtocolLogBuffer {
    items: Arc<Mutex<Vec<ProtocolLogEntry>>>,
}

impl ProtocolLogBuffer {
    fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn push(&self, kind: ProtocolLogKind, text: impl Into<String>) {
        if let Ok(mut items) = self.items.lock() {
            items.push(ProtocolLogEntry::new(kind, text));
            if items.len() > 1000 {
                let drain = items.len().saturating_sub(1000);
                items.drain(0..drain);
            }
        }
    }

    fn snapshot(&self) -> Vec<ProtocolLogEntry> {
        self.items
            .lock()
            .map(|items| items.clone())
            .unwrap_or_default()
    }

    fn clear(&self) {
        if let Ok(mut items) = self.items.lock() {
            items.clear();
        }
    }
}

pub struct SshTerminal {
    channel: Arc<Mutex<ssh2::Channel>>,
    outputs: Arc<Mutex<Vec<String>>>,
    cwd: Arc<Mutex<String>>,
    closed: Arc<AtomicBool>,
    _reader: Mutex<Option<thread::JoinHandle<()>>>,
}

impl SshTerminal {
    fn spawn(channel: ssh2::Channel) -> Result<Arc<dyn RemoteTerminal>> {
        let channel = Arc::new(Mutex::new(channel));
        let outputs = Arc::new(Mutex::new(Vec::new()));
        let cwd = Arc::new(Mutex::new(String::new()));
        let closed = Arc::new(AtomicBool::new(false));

        let reader_channel = Arc::clone(&channel);
        let reader_outputs = Arc::clone(&outputs);
        let reader_cwd = Arc::clone(&cwd);
        let reader_closed = Arc::clone(&closed);
        let handle = thread::Builder::new()
            .name("ftp-sftp-ssh-terminal".into())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                while !reader_closed.load(Ordering::SeqCst) {
                    let read = {
                        let mut guard = match reader_channel.lock() {
                            Ok(guard) => guard,
                            Err(_) => break,
                        };
                        match guard.read(&mut buf) {
                            Ok(n) => n,
                            Err(_) => {
                                thread::sleep(Duration::from_millis(80));
                                continue;
                            }
                        }
                    };
                    if read == 0 {
                        thread::sleep(Duration::from_millis(80));
                        continue;
                    }
                    let text = String::from_utf8_lossy(&buf[..read]).into_owned();
                    if let Ok(mut lines) = reader_outputs.lock() {
                        lines.extend(text.lines().map(|line| line.to_string()));
                        if lines.len() > 2000 {
                            let drain = lines.len().saturating_sub(2000);
                            lines.drain(0..drain);
                        }
                    }
                    if let Some(path) = text
                        .lines()
                        .rev()
                        .map(str::trim)
                        .find(|line| line.starts_with('/'))
                    {
                        if let Ok(mut cwd_value) = reader_cwd.lock() {
                            *cwd_value = path.to_string();
                        }
                    }
                }
            })
            .context("无法启动终端读取线程")?;

        Ok(Arc::new(Self {
            channel,
            outputs,
            cwd,
            closed,
            _reader: Mutex::new(Some(handle)),
        }))
    }
}

impl RemoteTerminal for SshTerminal {
    fn write_input(&self, text: &str) -> Result<()> {
        let mut channel = self.channel.lock().map_err(|_| anyhow!("终端锁被污染"))?;
        channel.write_all(text.as_bytes()).context("写入终端失败")?;
        channel.flush().context("刷新终端失败")?;
        Ok(())
    }

    fn resize(&self, cols: u32, rows: u32) -> Result<()> {
        let mut channel = self.channel.lock().map_err(|_| anyhow!("终端锁被污染"))?;
        channel
            .request_pty_size(cols.max(1), rows.max(1), None, None)
            .context("调整终端尺寸失败")?;
        Ok(())
    }

    fn try_read(&self) -> Vec<String> {
        self.outputs
            .lock()
            .map(|mut lines| std::mem::take(&mut *lines))
            .unwrap_or_default()
    }

    fn cwd_hint(&self) -> String {
        self.cwd.lock().map(|cwd| cwd.clone()).unwrap_or_default()
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Ok(mut channel) = self.channel.lock() {
            let _ = channel.send_eof();
            let _ = channel.close();
            let _ = channel.wait_close();
        }
    }
}

pub struct SftpBackend {
    profile: RemoteProfile,
    session: Option<ssh2::Session>,
    sftp: Option<ssh2::Sftp>,
}

impl SftpBackend {
    pub fn new(profile: RemoteProfile) -> Self {
        Self {
            profile,
            session: None,
            sftp: None,
        }
    }

    fn sftp(&self) -> Result<&ssh2::Sftp> {
        self.sftp.as_ref().context("SFTP 子系统未打开")
    }

    fn session(&self) -> Result<&ssh2::Session> {
        self.session.as_ref().context("SSH 会话未建立")
    }

    fn authenticate_session(
        sess: &ssh2::Session,
        username: &str,
        password: &str,
        key_path: &str,
        passphrase: &str,
        auth_method: AuthMethod,
    ) -> Result<()> {
        match auth_method {
            AuthMethod::PrivateKey if !key_path.is_empty() => {
                let key_path = expand_tilde(key_path);
                sess.userauth_pubkey_file(
                    username,
                    None,
                    Path::new(&key_path),
                    if passphrase.is_empty() {
                        None
                    } else {
                        Some(passphrase)
                    },
                )
                .context("私钥认证失败")?;
            }
            AuthMethod::Agent => {
                sess.userauth_agent(username)
                    .context("SSH Agent 认证失败")?;
            }
            _ => {
                sess.userauth_password(username, password)
                    .context("密码认证失败")?;
            }
        }
        if !sess.authenticated() {
            return Err(anyhow!("认证未通过"));
        }
        Ok(())
    }

    /// 通过跳板机建立 SSH 隧道连接目标主机
    /// 注：完整的跳板机 SSH 隧道需要双向 I/O 转发，ssh2 API 的 stream/set_tcp_stream
    /// 限制使直接实现较复杂。当前返回清晰的错误提示，完整实现将在后续版本中提供。
    fn connect_via_jump_host(&mut self, _timeout: Duration) -> Result<()> {
        return Err(anyhow!(
            "跳板机 SSH 隧道功能正在开发中。\n\
             当前已完成数据模型和 UI 支持，SSH 双向转发通道将在后续版本实现。\n\
             建议：可先在本地终端执行 ssh -J {jump_user}@{jump_host}:{jump_port} {user}@{host} -p {port} 测试连接。",
            jump_user = self.profile.jump_username,
            jump_host = self.profile.jump_host,
            jump_port = self.profile.jump_port,
            user = self.profile.username,
            host = self.profile.host,
            port = self.profile.port,
        ));
    }
}

impl RemoteBackend for SftpBackend {
    fn connect(&mut self) -> Result<()> {
        let timeout = Duration::from_secs(self.profile.connect_timeout_secs as u64);

        // 跳板机连接：先通过跳板机建立 SSH 隧道
        if self.profile.jump_enabled
            && !self.profile.jump_host.trim().is_empty()
        {
            return self.connect_via_jump_host(timeout);
        }

        let addr = format!("{}:{}", self.profile.host, self.profile.port);
        let tcp = TcpStream::connect_timeout(
            &addr.parse().map_err(|e| anyhow!("目标地址无效: {e}"))?,
            timeout,
        )
        .map_err(|e| anyhow!("连接失败: {e}"))?;

        let mut sess = ssh2::Session::new().context("创建 SSH 会话失败")?;
        sess.set_timeout(timeout.as_millis() as u32);
        sess.set_tcp_stream(tcp);
        sess.handshake().context("SSH 握手失败")?;

        Self::authenticate_session(
            &sess,
            &self.profile.username,
            &self.profile.password,
            &self.profile.private_key_path,
            &self.profile.private_key_passphrase,
            self.profile.auth_method,
        )?;

        if self.profile.protocol.supports_file_browser() {
            let sftp = sess.sftp().context("打开 SFTP 子系统失败")?;
            self.sftp = Some(sftp);
        } else {
            self.sftp = None;
        }
        self.session = Some(sess);
        Ok(())
    }

    fn close(&mut self) {
        self.sftp.take();
        if let Some(ref sess) = self.session {
            let _ = sess.disconnect(None, "client disconnect", None);
        }
        self.session.take();
    }

    fn list_dir(&self, path: &str) -> Result<Vec<RemoteFileItem>> {
        let sftp = self.sftp()?;
        let entries = sftp.readdir(Path::new(path)).context("读取目录失败")?;
        let mut items = Vec::new();
        for (entry_path, stat) in entries {
            let name = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name == "." || name == ".." {
                continue;
            }
            let full_path = join_remote_path(path, &name);
            let perms = format_mode(stat.perm.unwrap_or(0));
            if stat.is_dir() {
                items.push(RemoteFileItem::dir(name, full_path, perms));
            } else {
                items.push(RemoteFileItem::file(
                    name,
                    full_path,
                    stat.size.unwrap_or(0) as i64,
                    stat.mtime.unwrap_or(0) as i64,
                    perms,
                ));
            }
        }
        items.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(items)
    }

    fn mkdir(&self, path: &str) -> Result<()> {
        self.sftp()?
            .mkdir(Path::new(path), 0o755)
            .context("创建目录失败")?;
        Ok(())
    }

    fn rename(&self, source: &str, target: &str) -> Result<()> {
        self.sftp()?
            .rename(Path::new(source), Path::new(target), None)
            .context("重命名失败")?;
        Ok(())
    }

    fn delete_file(&self, path: &str) -> Result<()> {
        self.sftp()
            .context("SFTP 子系统未打开")?
            .unlink(Path::new(path))
            .context("删除文件失败")?;
        Ok(())
    }

    fn delete_dir(&self, path: &str) -> Result<()> {
        self.sftp()?
            .rmdir(Path::new(path))
            .context("删除目录失败")?;
        Ok(())
    }

    fn upload_file(
        &self,
        local: &Path,
        remote: &str,
        progress: &dyn Fn(usize, usize),
    ) -> Result<()> {
        let sftp = self.sftp()?;
        let mut local_file = std::fs::File::open(local).context("打开本地文件失败")?;
        let total = local_file.metadata().context("读取文件大小失败")?.len() as usize;

        let mut remote_file = sftp
            .open_mode(
                Path::new(remote),
                ssh2::OpenFlags::CREATE | ssh2::OpenFlags::TRUNCATE | ssh2::OpenFlags::WRITE,
                0o644,
                ssh2::OpenType::File,
            )
            .context("打开远程文件失败")?;

        let mut buf = [0u8; 32768];
        let mut sent = 0usize;
        loop {
            let n = local_file.read(&mut buf).context("读取本地数据失败")?;
            if n == 0 {
                break;
            }
            remote_file
                .write_all(&buf[..n])
                .context("写入远程数据失败")?;
            sent += n;
            progress(sent, total);
        }
        Ok(())
    }

    fn download_file(
        &self,
        remote: &str,
        local: &Path,
        progress: &dyn Fn(usize, usize),
    ) -> Result<()> {
        let sftp = self.sftp()?;
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent).context("创建本地目录失败")?;
        }

        let mut remote_file = sftp.open(Path::new(remote)).context("打开远程文件失败")?;
        let stat = remote_file.stat().context("读取远程文件信息失败")?;
        let total = stat.size.unwrap_or(0) as usize;
        let mut local_file = std::fs::File::create(local).context("创建本地文件失败")?;
        let mut buf = [0u8; 32768];
        let mut received = 0usize;
        loop {
            let n = remote_file.read(&mut buf).context("读取远程数据失败")?;
            if n == 0 {
                break;
            }
            local_file
                .write_all(&buf[..n])
                .context("写入本地数据失败")?;
            received += n;
            progress(received, total);
        }
        Ok(())
    }

    fn home_dir(&self) -> Result<String> {
        let home = self
            .sftp()?
            .realpath(Path::new("."))
            .unwrap_or_else(|_| PathBuf::from("/"));
        let home_str = home.to_string_lossy().into_owned();
        if home_str.starts_with('/') {
            Ok(home_str)
        } else {
            Ok(String::from("/"))
        }
    }

    fn open_terminal(&self) -> Result<Arc<dyn RemoteTerminal>> {
        let mut channel = self.session()?.channel_session().context("创建终端失败")?;
        channel
            .request_pty("xterm-256color", None, Some((120, 32, 0, 0)))
            .context("申请终端 PTY 失败")?;
        channel.shell().context("启动远程 shell 失败")?;
        SshTerminal::spawn(channel)
    }

    fn remote_file_version(&self, remote: &str) -> Result<String> {
        let stat = self
            .sftp()?
            .stat(Path::new(remote))
            .with_context(|| format!("读取远程文件信息失败: {remote}"))?;
        Ok(format!(
            "{}:{}",
            stat.size.unwrap_or(0),
            stat.mtime.unwrap_or(0)
        ))
    }
}

pub struct FtpBackend {
    profile: RemoteProfile,
    stream: Option<Mutex<suppaftp::FtpStream>>,
    log: ProtocolLogBuffer,
}

impl FtpBackend {
    pub fn new(profile: RemoteProfile) -> Self {
        Self {
            profile,
            stream: None,
            log: ProtocolLogBuffer::new(),
        }
    }

    fn ftp(&self) -> Result<std::sync::MutexGuard<'_, suppaftp::FtpStream>> {
        self.stream
            .as_ref()
            .context("FTP 连接未建立")?
            .lock()
            .map_err(|_| anyhow!("FTP 锁被污染"))
    }

    fn log_cmd(&self, text: impl Into<String>) {
        self.log.push(ProtocolLogKind::Command, text);
    }

    fn log_resp(&self, text: impl Into<String>) {
        self.log.push(ProtocolLogKind::Response, text);
    }

    fn log_info(&self, text: impl Into<String>) {
        self.log.push(ProtocolLogKind::Info, text);
    }

    fn log_error(&self, text: impl Into<String>) {
        self.log.push(ProtocolLogKind::Error, text);
    }
}

impl RemoteBackend for FtpBackend {
    fn connect(&mut self) -> Result<()> {
        let timeout = Duration::from_secs(self.profile.connect_timeout_secs as u64);
        let addr: std::net::SocketAddr = format!("{}:{}", self.profile.host, self.profile.port)
            .parse()
            .context("解析 FTP 地址失败")?;

        let host = self.profile.host.clone();

        if self.profile.protocol == RemoteProtocol::Ftps {
            // TODO: FTPS 完整实现需要解决 suppaftp TLS connector 类型匹配问题
            // FTPS 模式 (Explicit/Implicit) 的数据模型和 UI 配置均已就位
            self.log_error(format!("FTPS 连接被阻止：当前 {} 模式", self.profile.ftps_mode.label()));
            return Err(anyhow!(
                "FTPS TLS 连接正在完善中。suppaftp 6.x 的 TLS connector API 需要适配。\n\
                 建议使用 SFTP 作为安全的替代方案。"
            ));
        }

        self.log_cmd(format!(
            "CONNECT {}:{}",
            self.profile.host, self.profile.port
        ));
        let mut ftp =
            suppaftp::FtpStream::connect_timeout(addr, timeout).context("FTP 连接失败")?;
        if self.profile.passive_mode {
            ftp.set_mode(Mode::Passive);
        } else {
            ftp.set_mode(Mode::Active);
        }
        self.log_resp("220 connected");
        self.log_cmd(format!("USER {}", self.profile.username));
        ftp.login(&self.profile.username, &self.profile.password)
            .context("FTP 登录失败")?;
        self.log_resp("230 login success");
        self.stream = Some(Mutex::new(ftp));
        Ok(())
    }

    fn close(&mut self) {
        self.log_info("连接关闭");
        if let Some(stream) = self.stream.take() {
            if let Ok(mut ftp) = stream.lock() {
                let _ = ftp.quit();
            }
        }
    }

    fn list_dir(&self, path: &str) -> Result<Vec<RemoteFileItem>> {
        self.log_cmd(format!("NLST {path}"));
        let mut ftp = self.ftp()?;
        let names = ftp.nlst(Some(path)).context("读取目录失败")?;
        let mut items = Vec::new();
        for raw_name in names {
            let name = raw_name
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_string();
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }
            let full_path = join_remote_path(path, &name);
            let is_dir = ftp.cwd(&full_path).is_ok();
            if is_dir {
                let _ = ftp.cwd(path);
                items.push(RemoteFileItem::dir(name, full_path, String::new()));
            } else {
                let size = ftp.size(&full_path).unwrap_or(0) as i64;
                let modified_at = ftp
                    .mdtm(&full_path)
                    .map(|value| {
                        let ts = value.and_utc().timestamp();
                        ts.max(0)
                    })
                    .unwrap_or(0);
                items.push(RemoteFileItem::file(
                    name,
                    full_path,
                    size,
                    modified_at,
                    String::new(),
                ));
            }
        }
        self.log_resp(format!("226 {} 项", items.len()));
        items.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(items)
    }

    fn mkdir(&self, path: &str) -> Result<()> {
        self.log_cmd(format!("MKD {path}"));
        self.ftp()?.mkdir(path).context("创建目录失败")?;
        self.log_resp("257 directory created");
        Ok(())
    }

    fn rename(&self, source: &str, target: &str) -> Result<()> {
        self.log_cmd(format!("RNFR {source}"));
        self.log_cmd(format!("RNTO {target}"));
        self.ftp()?.rename(source, target).context("重命名失败")?;
        self.log_resp("250 renamed");
        Ok(())
    }

    fn delete_file(&self, path: &str) -> Result<()> {
        self.log_cmd(format!("DELE {path}"));
        self.ftp()?.rm(path).context("删除文件失败")?;
        self.log_resp("250 file deleted");
        Ok(())
    }

    fn delete_dir(&self, path: &str) -> Result<()> {
        self.log_cmd(format!("RMD {path}"));
        self.ftp()?.rmdir(path).context("删除目录失败")?;
        self.log_resp("250 directory deleted");
        Ok(())
    }

    fn upload_file(
        &self,
        local: &Path,
        remote: &str,
        progress: &dyn Fn(usize, usize),
    ) -> Result<()> {
        self.log_cmd(format!("STOR {remote}"));
        let mut ftp = self.ftp()?;
        let total = std::fs::metadata(local)
            .context("读取本地文件信息失败")?
            .len() as usize;
        let mut file = std::fs::File::open(local).context("打开本地文件失败")?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).context("读取本地数据失败")?;
        progress(data.len(), total);
        ftp.put_file(remote, &mut &data[..])
            .context("上传文件失败")?;
        progress(total, total);
        self.log_resp("226 upload complete");
        Ok(())
    }

    fn download_file(
        &self,
        remote: &str,
        local: &Path,
        progress: &dyn Fn(usize, usize),
    ) -> Result<()> {
        self.log_cmd(format!("RETR {remote}"));
        let mut ftp = self.ftp()?;
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent).context("创建本地目录失败")?;
        }
        let data = ftp.retr_as_buffer(remote).context("下载文件失败")?;
        let total = data.get_ref().len();
        progress(total, total);
        std::fs::write(local, data.get_ref()).context("写入本地文件失败")?;
        self.log_resp("226 download complete");
        Ok(())
    }

    fn home_dir(&self) -> Result<String> {
        self.log_cmd("PWD");
        let mut ftp = self.ftp()?;
        let pwd = ftp.pwd().unwrap_or_else(|_| String::from("/"));
        self.log_resp(format!("257 {pwd}"));
        if pwd.starts_with('/') {
            Ok(pwd)
        } else {
            Ok(format!("/{pwd}"))
        }
    }

    fn protocol_log_snapshot(&self) -> Vec<ProtocolLogEntry> {
        self.log.snapshot()
    }

    fn clear_protocol_log(&self) {
        self.log.clear();
    }

    fn remote_file_version(&self, remote: &str) -> Result<String> {
        let mut ftp = self.ftp()?;
        let size = ftp.size(remote).unwrap_or(0);
        let modified = ftp
            .mdtm(remote)
            .map(|value| value.and_utc().timestamp())
            .unwrap_or(0);
        Ok(format!("{size}:{modified}"))
    }
}

pub fn create_backend(profile: &RemoteProfile) -> Box<dyn RemoteBackend> {
    match profile.protocol {
        RemoteProtocol::Ftp | RemoteProtocol::Ftps => Box::new(FtpBackend::new(profile.clone())),
        _ => Box::new(SftpBackend::new(profile.clone())),
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

fn format_mode(perm: u32) -> String {
    let mut s = String::with_capacity(10);
    let chars = ['r', 'w', 'x'];
    for i in (0..9).rev() {
        if perm & (1 << i) != 0 {
            s.push(chars[2 - (i % 3)]);
        } else {
            s.push('-');
        }
    }
    s
}

pub fn make_remote_version_hint(size: i64, modified_at: i64) -> String {
    format!("{size}:{modified_at}")
}

pub fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

pub fn spawn_file_monitor(
    path: String,
    tick_ms: u64,
    on_change: impl Fn(i64) + Send + 'static,
) -> mpsc::Sender<()> {
    let (tx, rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let mut last_seen = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();
        loop {
            if rx.try_recv().is_ok() {
                break;
            }
            let current = std::fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(last_seen);
            if current > last_seen {
                last_seen = current;
                on_change(current);
            }
            thread::sleep(Duration::from_millis(tick_ms.max(150)));
        }
    });
    tx
}
