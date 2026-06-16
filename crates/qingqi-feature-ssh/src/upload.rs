//! 本地上传任务收集与冲突检测

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use russh_sftp::client::error::Error as SftpClientError;
use russh_sftp::protocol::StatusCode;

use crate::model::{RemoteEntry, SessionId, TransferId};
use crate::service::SshService;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadItem {
    pub local: PathBuf,
    pub remote: String,
}

/// 将拖入/选择的本地路径展开为上传任务（支持文件与文件夹递归）。
pub fn collect_upload_items(local_paths: &[PathBuf], remote_base: &str) -> Result<Vec<UploadItem>> {
    let base = normalize_remote_dir(remote_base);
    let mut items = Vec::new();
    for path in local_paths {
        if path.is_file() {
            push_file(path, &base, &mut items)?;
        } else if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .with_context(|| format!("无法读取目录名 {}", path.display()))?;
            walk_dir(path, &join_remote(&base, name), &mut items)?;
        }
    }
    Ok(items)
}

/// 找出远程已存在同名条目的上传任务。
pub fn find_upload_conflicts(
    service: &SshService,
    session_id: &SessionId,
    items: &[UploadItem],
) -> Result<Vec<UploadItem>> {
    let cwd = normalize_remote_dir(&service.session_cwd(session_id));
    let cwd_entries = service.session_entries(session_id);
    let mut cache = RemoteListingCache::new(service, *session_id, cwd, cwd_entries);
    let mut conflicts = Vec::new();
    for item in items {
        let uploading_file = item.local.is_file();
        if cache.entry_conflicts(&item.remote, uploading_file)? {
            conflicts.push(item.clone());
        }
    }
    Ok(conflicts)
}

/// 在后台线程入队上传任务（含远程目录准备）。
pub fn enqueue_upload_batch(
    service: &SshService,
    session_id: &SessionId,
    items: &[UploadItem],
    cwd: &str,
) -> (Vec<TransferId>, usize) {
    let cwd = normalize_remote_dir(cwd);
    let mut transfer_ids = Vec::new();
    let mut failures = 0usize;
    for item in items {
        let parent = remote_parent(&item.remote).unwrap_or_default();
        let needs_parent = !parent.is_empty() && parent != "/" && !remote_dirs_match(&parent, &cwd);
        if needs_parent {
            if let Err(error) = service.ensure_remote_parent_dirs(session_id, &item.remote) {
                tracing::warn!(
                    target: "qingqi_ssh",
                    remote = %item.remote,
                    error = %error,
                    "创建远程目录失败"
                );
                failures += 1;
                continue;
            }
        }
        match service.upload_file(session_id, &item.local, &item.remote) {
            Ok(tid) => transfer_ids.push(tid),
            Err(error) => {
                failures += 1;
                tracing::warn!(
                    target: "qingqi_ssh",
                    path = %item.local.display(),
                    error = %error,
                    "上传入队失败"
                );
            }
        }
    }
    (transfer_ids, failures)
}

/// 判断远程路径是否已存在（不修改 Session 当前目录）。
pub fn remote_entry_exists(
    service: &SshService,
    session_id: &SessionId,
    remote: &str,
) -> Result<bool> {
    let cwd = normalize_remote_dir(&service.session_cwd(session_id));
    let cwd_entries = service.session_entries(session_id);
    let mut cache = RemoteListingCache::new(service, *session_id, cwd, cwd_entries);
    cache.entry_exists(remote)
}

pub fn join_remote(base: &str, rel: &str) -> String {
    let base = normalize_remote_dir(base);
    let rel = rel.trim_start_matches('/');
    if base.is_empty() || base == "/" {
        if rel.is_empty() {
            "/".to_string()
        } else if rel.starts_with('/') {
            rel.to_string()
        } else {
            format!("/{rel}")
        }
    } else if rel.is_empty() {
        base
    } else {
        format!("{base}/{rel}")
    }
}

fn normalize_remote_dir(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::new();
    }
    if path == "/" {
        return "/".to_string();
    }
    path.trim_end_matches('/').to_string()
}

pub fn remote_dirs_match(a: &str, b: &str) -> bool {
    normalize_remote_dir(a) == normalize_remote_dir(b)
}

fn push_file(local: &Path, remote_base: &str, items: &mut Vec<UploadItem>) -> Result<()> {
    let name = local
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("无法读取文件名 {}", local.display()))?;
    items.push(UploadItem {
        local: local.to_path_buf(),
        remote: join_remote(remote_base, name),
    });
    Ok(())
}

fn walk_dir(local_dir: &Path, remote_dir: &str, items: &mut Vec<UploadItem>) -> Result<()> {
    for entry in std::fs::read_dir(local_dir)
        .with_context(|| format!("读取目录 {} 失败", local_dir.display()))?
    {
        let entry = entry?;
        let local_path = entry.path();
        let name = entry.file_name();
        let name = name
            .to_str()
            .with_context(|| format!("无法读取文件名 {}", local_path.display()))?;
        let remote_path = join_remote(remote_dir, name);
        if local_path.is_dir() {
            walk_dir(&local_path, &remote_path, items)?;
        } else if local_path.is_file() {
            items.push(UploadItem {
                local: local_path,
                remote: remote_path,
            });
        }
    }
    Ok(())
}

struct RemoteListingCache<'a> {
    service: &'a SshService,
    session_id: SessionId,
    cwd: String,
    cwd_entries: Vec<RemoteEntry>,
    listings: HashMap<String, Vec<RemoteEntry>>,
}

impl<'a> RemoteListingCache<'a> {
    fn new(
        service: &'a SshService,
        session_id: SessionId,
        cwd: String,
        cwd_entries: Vec<RemoteEntry>,
    ) -> Self {
        Self {
            service,
            session_id,
            cwd,
            cwd_entries,
            listings: HashMap::new(),
        }
    }

    fn entry_exists(&mut self, remote: &str) -> Result<bool> {
        let remote = remote.trim_end_matches('/');
        let Some((parent, name)) = split_remote_parent(remote) else {
            return Ok(false);
        };
        if name.is_empty() || name == "." || name == ".." {
            return Ok(false);
        }
        let matches = |entry: &RemoteEntry| entry.name == name;
        if remote_dirs_match(&parent, &self.cwd) {
            return Ok(self.cwd_entries.iter().any(matches));
        }
        let entries = match self.listings(&parent) {
            Ok(entries) => entries,
            Err(e) if is_remote_not_found(&e) => return Ok(false),
            Err(e) => return Err(e),
        };
        Ok(entries.iter().any(matches))
    }

    fn entry_conflicts(&mut self, remote: &str, uploading_file: bool) -> Result<bool> {
        let remote = remote.trim_end_matches('/');
        let Some((parent, name)) = split_remote_parent(remote) else {
            return Ok(false);
        };
        if name.is_empty() || name == "." || name == ".." {
            return Ok(false);
        }
        if !remote_dirs_match(&parent, &self.cwd) {
            return Ok(false);
        }
        let matches = |entry: &RemoteEntry| entry.name == name && entry.is_dir != uploading_file;
        Ok(self.cwd_entries.iter().any(matches))
    }

    fn listings(&mut self, parent: &str) -> Result<&Vec<RemoteEntry>> {
        let key = normalize_remote_dir(parent);
        if !self.listings.contains_key(&key) {
            let entries = self.service.peek_directory(&self.session_id, &key)?;
            self.listings.insert(key, entries);
        }
        Ok(self
            .listings
            .get(&normalize_remote_dir(parent))
            .expect("listing cached"))
    }
}

pub fn remote_parent(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    let pos = path.rfind('/')?;
    if pos == 0 {
        Some(String::from("/"))
    } else {
        Some(path[..pos].to_string())
    }
}

/// 检查错误是否为远端目录不存在（SFTP SSH_FX_NO_SUCH_FILE）。
fn is_remote_not_found(err: &anyhow::Error) -> bool {
    if let Some(sftp_err) = err.downcast_ref::<SftpClientError>() {
        if let SftpClientError::Status(status) = sftp_err {
            return status.status_code == StatusCode::NoSuchFile;
        }
    }
    for cause in err.chain().skip(1) {
        if let Some(sftp_err) = cause.downcast_ref::<SftpClientError>() {
            if let SftpClientError::Status(status) = sftp_err {
                return status.status_code == StatusCode::NoSuchFile;
            }
        }
    }
    false
}

pub fn split_remote_parent(remote: &str) -> Option<(String, String)> {
    let remote = remote.trim_end_matches('/');
    if remote.is_empty() {
        return None;
    }
    let pos = remote.rfind('/')?;
    if pos == 0 {
        Some((String::from("/"), remote[1..].to_string()))
    } else {
        Some((remote[..pos].to_string(), remote[pos + 1..].to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_remote_paths() {
        assert_eq!(join_remote("/a/b", "c"), "/a/b/c");
        assert_eq!(join_remote("/a/b/", "/c"), "/a/b/c");
        assert_eq!(join_remote("", "file"), "/file");
        assert_eq!(join_remote("/var/www", "index.html"), "/var/www/index.html");
    }

    #[test]
    fn collect_single_file() {
        let dir = std::env::temp_dir().join(format!("ssh-upload-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hello.txt");
        std::fs::write(&file, b"hi").unwrap();

        let items = collect_upload_items(&[file.clone()], "/remote").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].remote, "/remote/hello.txt");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_folder_recursive() {
        let dir = std::env::temp_dir().join(format!("ssh-upload-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"a").unwrap();
        std::fs::write(dir.join("sub/b.txt"), b"b").unwrap();

        let items = collect_upload_items(&[dir.clone()], "/remote").unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.remote.ends_with("/a.txt")));
        assert!(items.iter().any(|i| i.remote.ends_with("/sub/b.txt")));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
