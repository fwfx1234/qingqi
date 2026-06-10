//! SSH 协议实现（russh）
//!
//! 当前为骨架，实际连接逻辑后续填充。

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{ProtocolCapability, RemoteProtocol, TerminalOutput, TransferProgress};
use crate::model::{Profile, RemoteEntry};

pub struct SshProtocol {}

impl SshProtocol {
    pub fn new(_profile: &Profile) -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl RemoteProtocol for SshProtocol {
    async fn connect(&self) -> Result<()> {
        Err(anyhow::anyhow!("SSH 连接尚未实现"))
    }

    async fn disconnect(&self) {}

    fn is_connected(&self) -> bool {
        false
    }

    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::InteractiveTerminal]
    }

    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>> {
        Err(anyhow::anyhow!("SSH 终端尚未实现"))
    }

    async fn send_terminal_input(&self, _data: &[u8]) -> Result<()> {
        Err(anyhow::anyhow!("SSH 终端尚未实现"))
    }

    async fn resize_terminal(&self, _cols: u16, _rows: u16) -> Result<()> {
        Ok(())
    }

    async fn list_directory(&self, _path: &str) -> Result<Vec<RemoteEntry>> {
        Err(anyhow::anyhow!("SFTP 尚未实现"))
    }

    async fn create_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("SFTP 尚未实现"))
    }

    async fn rename_entry(&self, _old_path: &str, _new_path: &str) -> Result<()> {
        Err(anyhow::anyhow!("SFTP 尚未实现"))
    }

    async fn remove_file(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("SFTP 尚未实现"))
    }

    async fn remove_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("SFTP 尚未实现"))
    }

    async fn upload_file(
        &self,
        _local: &Path,
        _remote: &str,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("SFTP 尚未实现"))
    }

    async fn download_file(
        &self,
        _remote: &str,
        _local: &Path,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("SFTP 尚未实现"))
    }
}
