//! 本地上传任务收集与冲突检测

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::SessionId;
use crate::service::SshService;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadItem {
    pub local: PathBuf,
    pub remote: String,
}

/// 将拖入/选择的本地路径展开为上传任务（支持文件与文件夹递归）。
pub fn collect_upload_items(local_paths: &[PathBuf], remote_base: &str) -> Result<Vec<UploadItem>> {
    let base = remote_base.trim_end_matches('/');
    let mut items = Vec::new();
    for path in local_paths {
        if path.is_file() {
            push_file(path, base, &mut items)?;
        } else if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .with_context(|| format!("无法读取目录名 {}", path.display()))?;
            walk_dir(path, &join_remote(base, name), &mut items)?;
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
    let mut cache = RemoteListingCache::new(service, *session_id);
    let mut conflicts = Vec::new();
    for item in items {
        if cache.entry_exists(&item.remote)? {
            conflicts.push(item.clone());
        }
    }
    Ok(conflicts)
}

pub fn join_remote(base: &str, rel: &str) -> String {
    let base = base.trim_end_matches('/');
    let rel = rel.trim_start_matches('/');
    if base.is_empty() {
        rel.to_string()
    } else if rel.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{rel}")
    }
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
    listings: HashMap<String, Vec<crate::model::RemoteEntry>>,
}

impl<'a> RemoteListingCache<'a> {
    fn new(service: &'a SshService, session_id: SessionId) -> Self {
        Self {
            service,
            session_id,
            listings: HashMap::new(),
        }
    }

    fn entry_exists(&mut self, remote: &str) -> Result<bool> {
        let remote = remote.trim_end_matches('/');
        let Some((parent, name)) = split_remote_parent(remote) else {
            return Ok(false);
        };
        let entries = self.listings(&parent)?;
        Ok(entries.iter().any(|entry| entry.name == name))
    }

    fn listings(&mut self, parent: &str) -> Result<&Vec<crate::model::RemoteEntry>> {
        if !self.listings.contains_key(parent) {
            let entries = self
                .service
                .list_directory(&self.session_id, parent)?;
            self.listings.insert(parent.to_string(), entries);
        }
        Ok(self.listings.get(parent).expect("listing cached"))
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
        assert_eq!(join_remote("", "file"), "file");
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
