//! 协议抽象层

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::model::{Profile, ProtocolType, RemoteEntry};

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
    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>>;

    /// 发送终端输入
    async fn send_terminal_input(&self, data: &[u8]) -> Result<()>;

    /// 调整终端大小（仅 SSH）
    async fn resize_terminal(&self, _cols: u16, _rows: u16) -> Result<()> {
        Ok(())
    }

    /// 列出目录内容
    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteEntry>>;

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
}

// ============ ProtocolRegistry ============

pub type ProtocolFactory =
    Box<dyn Fn(&Profile) -> Result<Box<dyn RemoteProtocol>> + Send + Sync>;

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
pub mod ssh;
pub mod ftp;
