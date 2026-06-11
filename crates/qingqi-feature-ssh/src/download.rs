//! 远程目录递归下载任务收集

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::SessionId;
use crate::service::SshService;
use crate::upload::join_remote;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadItem {
    pub remote: String,
    pub local: PathBuf,
}

/// 将远程目录递归展开为下载任务列表。
pub fn collect_download_items(
    service: &SshService,
    session_id: &SessionId,
    remote_dir: &str,
    local_dir: &Path,
) -> Result<Vec<DownloadItem>> {
    let mut items = Vec::new();
    walk_remote(service, session_id, remote_dir, local_dir, &mut items)?;
    Ok(items)
}

fn walk_remote(
    service: &SshService,
    session_id: &SessionId,
    remote_dir: &str,
    local_dir: &Path,
    items: &mut Vec<DownloadItem>,
) -> Result<()> {
    let entries = service
        .list_directory(session_id, remote_dir)
        .with_context(|| format!("列出远程目录 {remote_dir} 失败"))?;
    for entry in entries {
        let remote_path = if entry.path.is_empty() {
            join_remote(remote_dir, &entry.name)
        } else {
            entry.path.clone()
        };
        let local_path = local_dir.join(&entry.name);
        if entry.is_dir {
            std::fs::create_dir_all(&local_path)
                .with_context(|| format!("创建本地目录 {} 失败", local_path.display()))?;
            walk_remote(service, session_id, &remote_path, &local_path, items)?;
        } else {
            items.push(DownloadItem {
                remote: remote_path,
                local: local_path,
            });
        }
    }
    Ok(())
}
