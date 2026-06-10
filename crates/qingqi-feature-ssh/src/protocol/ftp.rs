//! FTP/FTPS 协议实现（suppaftp）
//!
//! 当前为骨架，实际连接逻辑后续填充。

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{ProtocolCapability, RemoteProtocol, TerminalOutput, TransferProgress};
use crate::model::{Profile, RemoteEntry};

pub struct FtpProtocol {}

impl FtpProtocol {
    pub fn new(_profile: &Profile) -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl RemoteProtocol for FtpProtocol {
    async fn connect(&self) -> Result<()> {
        Err(anyhow::anyhow!("FTP 连接尚未实现"))
    }

    async fn disconnect(&self) {}

    fn is_connected(&self) -> bool {
        false
    }

    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::LogTerminal]
    }

    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>> {
        Err(anyhow::anyhow!("FTP 日志终端尚未实现"))
    }

    async fn send_terminal_input(&self, _data: &[u8]) -> Result<()> {
        Err(anyhow::anyhow!("FTP 命令尚未实现"))
    }

    async fn list_directory(&self, _path: &str) -> Result<Vec<RemoteEntry>> {
        Err(anyhow::anyhow!("FTP 目录列表尚未实现"))
    }

    async fn create_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("FTP 创建目录尚未实现"))
    }

    async fn rename_entry(&self, _old_path: &str, _new_path: &str) -> Result<()> {
        Err(anyhow::anyhow!("FTP 重命名尚未实现"))
    }

    async fn remove_file(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("FTP 删除文件尚未实现"))
    }

    async fn remove_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("FTP 删除目录尚未实现"))
    }

    async fn upload_file(
        &self,
        _local: &Path,
        _remote: &str,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("FTP 上传尚未实现"))
    }

    async fn download_file(
        &self,
        _remote: &str,
        _local: &Path,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("FTP 下载尚未实现"))
    }
}
