//! 协议抽象层

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::{mpsc, Notify};
use tracing::debug;

use crate::model::{Profile, ProtocolType, RemoteEntry};

/// SSH PTY 输出缓冲上限（字节），超出后丢弃最旧数据。
pub const PTY_OUTPUT_BUFFER_CAP_BYTES: usize = 10 * 1024 * 1024;

/// russh 每 channel 未处理消息条数上限。
pub const PTY_CHANNEL_BUFFER_MSGS: usize = 8192;

// ============ 协议能力 ============

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtocolCapability {
    /// SSH: 交互式 shell
    InteractiveTerminal,
    /// FTP/FTPS: 命令响应日志
    LogTerminal,
}

// ============ 终端输出 ============

#[derive(Clone, Debug)]
pub enum TerminalOutput {
    /// SSH: PTY 原始输出（含 ANSI 转义序列）
    PtyOutput(Vec<u8>),
    /// FTP: 日志行
    LogLine { level: LogLevel, text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Sent,
    Received,
    Error,
}

/// SSH 有界 PTY 缓冲：消费慢时丢弃最旧输出，避免无限堆积。
pub struct PtyOutputHub {
    inner: Mutex<PtyOutputHubInner>,
    notify: Arc<Notify>,
}

struct PtyOutputHubInner {
    chunks: VecDeque<Vec<u8>>,
    total_bytes: usize,
}

impl PtyOutputHub {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(PtyOutputHubInner {
                chunks: VecDeque::new(),
                total_bytes: 0,
            }),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn push(&self, data: Vec<u8>) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.total_bytes += data.len();
        inner.chunks.push_back(data);
        while inner.total_bytes > PTY_OUTPUT_BUFFER_CAP_BYTES {
            let Some(old) = inner.chunks.pop_front() else {
                break;
            };
            inner.total_bytes -= old.len();
            debug!(
                target: "qingqi_ssh",
                dropped = old.len(),
                remaining = inner.total_bytes,
                cap = PTY_OUTPUT_BUFFER_CAP_BYTES,
                "term_diag: pty_output 丢弃旧数据"
            );
        }
        self.notify.notify_waiters();
    }

    pub fn drain(&self) -> Vec<Vec<u8>> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let drained: Vec<_> = inner.chunks.drain(..).collect();
        inner.total_bytes = 0;
        drained
    }

    pub fn notify(&self) -> Arc<Notify> {
        Arc::clone(&self.notify)
    }
}

/// 终端输出来源：FTP 用 channel，SSH 用有界 PTY hub。
pub enum TerminalOutputSource {
    Channel(mpsc::UnboundedReceiver<TerminalOutput>),
    PtyHub(Arc<PtyOutputHub>),
}

// ============ 传输进度 ============

#[derive(Clone, Debug)]
pub struct TransferProgress {
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: f64,
}

// ============ RemoteProtocol trait ============

#[async_trait]
pub trait RemoteProtocol: Send + Sync {
    /// 连接到远程服务器
    async fn connect(&self) -> Result<()>;

    /// 断开连接
    async fn disconnect(&self);

    /// 是否已连接
    fn is_connected(&self) -> bool;

    /// 返回协议能力列表
    fn capabilities(&self) -> Vec<ProtocolCapability>;

    /// 打开终端通道
    async fn open_terminal(&self) -> Result<TerminalOutputSource>;

    /// 发送终端输入
    async fn send_terminal_input(&self, data: &[u8]) -> Result<()>;

    /// 调整终端大小（仅 SSH）
    async fn resize_terminal(&self, _cols: u16, _rows: u16) -> Result<()> {
        Ok(())
    }

    /// 列出目录内容
    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteEntry>>;

    /// 最近一次 `list_directory` 实际使用的远程路径（如 `~` 已展开）
    fn last_list_path(&self) -> Option<String> {
        None
    }

    /// 创建目录
    async fn create_directory(&self, path: &str) -> Result<()>;

    /// 重命名
    async fn rename_entry(&self, old_path: &str, new_path: &str) -> Result<()>;

    /// 删除文件
    async fn remove_file(&self, path: &str) -> Result<()>;

    /// 删除目录
    async fn remove_directory(&self, path: &str) -> Result<()>;

    /// 上传文件
    async fn upload_file(
        &self,
        local: &Path,
        remote: &str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()>;

    /// 下载文件
    async fn download_file(
        &self,
        remote: &str,
        local: &Path,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()>;

    /// 读取远程文件内容（用于在线编辑）
    async fn read_file(&self, remote: &str) -> Result<Vec<u8>>;

    /// 写入远程文件内容（用于保存编辑）
    async fn write_file(&self, remote: &str, data: &[u8]) -> Result<()>;
}

// ============ ProtocolRegistry ============

pub type ProtocolFactory = Box<dyn Fn(&Profile) -> Result<Box<dyn RemoteProtocol>> + Send + Sync>;

pub struct ProtocolRegistry {
    factories: std::collections::HashMap<ProtocolType, ProtocolFactory>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            factories: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, proto: ProtocolType, factory: ProtocolFactory) {
        self.factories.insert(proto, factory);
    }

    pub fn create(&self, profile: &Profile) -> Result<Box<dyn RemoteProtocol>> {
        let factory = self
            .factories
            .get(&profile.protocol)
            .ok_or_else(|| anyhow::anyhow!("不支持的协议: {:?}", profile.protocol))?;
        factory(profile)
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// 子模块
pub mod ftp;
pub mod ssh;
