use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<FileEntry>,
    pub total_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: Option<String>,
}

pub fn browse_directory(path: &str) -> Result<DirectoryListing, String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err("路径不存在".to_string());
    }
    if !path.is_dir() {
        return Err("不是目录".to_string());
    }

    let mut entries: Vec<FileEntry> = match std::fs::read_dir(path) {
        Ok(read_dir) => read_dir
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let metadata = entry.metadata().ok()?;
                let name = entry.file_name().to_string_lossy().to_string();
                let is_hidden = is_hidden_file(&name, &metadata);
                Some(FileEntry {
                    name,
                    path: entry.path().to_string_lossy().to_string(),
                    is_dir: metadata.is_dir(),
                    size: metadata.len(),
                    modified: metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs()),
                    is_hidden,
                })
            })
            .collect(),
        Err(e) => return Err(format!("读取目录失败: {e}")),
    };

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let total_count = entries.len();

    Ok(DirectoryListing {
        current_path: path.to_string_lossy().to_string(),
        parent_path: path.parent().map(|p| p.to_string_lossy().to_string()),
        entries,
        total_count,
    })
}

#[cfg(target_os = "windows")]
fn is_hidden_file(name: &str, metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    name.starts_with('.') || (metadata.file_attributes() & 0x2) != 0
}

#[cfg(not(target_os = "windows"))]
fn is_hidden_file(name: &str, _metadata: &std::fs::Metadata) -> bool {
    name.starts_with('.')
}

pub fn get_quick_access() -> Vec<(String, String)> {
    let mut result = Vec::new();

    if let Some(dir) = dirs::desktop_dir() {
        result.push(("桌面".to_string(), dir.to_string_lossy().to_string()));
    }
    if let Some(dir) = dirs::document_dir() {
        result.push(("文档".to_string(), dir.to_string_lossy().to_string()));
    }
    if let Some(dir) = dirs::download_dir() {
        result.push(("下载".to_string(), dir.to_string_lossy().to_string()));
    }
    if let Some(dir) = dirs::home_dir() {
        result.push(("主目录".to_string(), dir.to_string_lossy().to_string()));
    }

    for drive in &["C:", "D:", "E:", "F:"] {
        let game_dir = format!("{}\\Games", drive);
        if Path::new(&game_dir).is_dir() {
            result.push((format!("{} 游戏", drive), game_dir));
        }
        let steam_dir = format!(
            "{}\\Program Files (x86)\\Steam\\steamapps\\common",
            drive
        );
        if Path::new(&steam_dir).is_dir() {
            result.push((format!("{} Steam", drive), steam_dir));
        }
    }

    result
}
