//! Steam 游戏扫描器 - 解析 Steam 库并提取游戏信息

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Steam 库文件夹信息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SteamLibrary {
    pub path: String,
    pub game_count: usize,
}

/// Steam 游戏信息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SteamGame {
    pub app_id: u32,
    pub name: String,
    pub install_dir: String,
    pub install_path: String,
    pub library_path: String,
    pub icon_path: Option<String>,
    pub size_bytes: Option<u64>,
    pub last_played: Option<i64>,
}

/// Steam 安装信息
#[derive(Clone, Debug)]
pub struct SteamInstallInfo {
    pub root_path: PathBuf,
    pub libraries: Vec<PathBuf>,
}

/// Steam 扫描器
pub struct SteamScanner {
    install_info: Option<SteamInstallInfo>,
}

impl SteamScanner {
    pub fn new() -> Self {
        Self { install_info: None }
    }

    /// 查找 Steam 安装路径
    pub fn find_steam_install(&mut self) -> Option<&SteamInstallInfo> {
        // 检查常见默认路径
        let default_paths = [
            PathBuf::from("C:\\Program Files (x86)\\Steam"),
            PathBuf::from("C:\\Program Files\\Steam"),
            PathBuf::from("D:\\Steam"),
            PathBuf::from("D:\\Program Files\\Steam"),
            PathBuf::from("E:\\Steam"),
            PathBuf::from("E:\\Program Files\\Steam"),
        ];

        for path in &default_paths {
            if self.is_steam_root(path) {
                self.install_info = Some(SteamInstallInfo {
                    root_path: path.clone(),
                    libraries: vec![path.clone()],
                });
                return self.install_info.as_ref();
            }
        }

        // 从注册表查找
        if let Some(path) = Self::find_steam_from_registry() {
            if self.is_steam_root(&path) {
                self.install_info = Some(SteamInstallInfo {
                    root_path: path.clone(),
                    libraries: vec![path.clone()],
                });
                return self.install_info.as_ref();
            }
        }

        None
    }

    /// 从注册表查找 Steam 安装路径
    fn find_steam_from_registry() -> Option<PathBuf> {
        // 使用 std 读取注册表需要额外依赖，这里返回 None
        // 实际路径通过默认路径检查获得
        None
    }

    /// 判断是否为 Steam 根目录
    fn is_steam_root(&self, path: &Path) -> bool {
        path.join("Steam.exe").exists() && path.join("steamapps").is_dir()
    }

    /// 解析所有库文件夹
    pub fn parse_library_folders(&mut self) -> Result<Vec<PathBuf>> {
        let info = self
            .install_info
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Steam 未安装"))?;

        let vdf_path = info
            .root_path
            .join("steamapps")
            .join("libraryfolders.vdf");

        if !vdf_path.exists() {
            return Ok(vec![info.root_path.clone()]);
        }

        let content = fs::read_to_string(&vdf_path)
            .map_err(|e| anyhow::anyhow!("读取 libraryfolders.vdf 失败: {}", e))?;

        let mut libraries = vec![info.root_path.clone()];

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("\"path\"") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let path_str = parts[1].trim_matches('"');
                    let path = PathBuf::from(path_str);
                    if !libraries.contains(&path) && path.join("steamapps").is_dir() {
                        libraries.push(path);
                    }
                }
            }
        }

        if let Some(info) = &mut self.install_info {
            info.libraries = libraries.clone();
        }

        Ok(libraries)
    }

    /// 扫描所有 Steam 游戏
    pub fn scan_games(&mut self) -> Result<Vec<SteamGame>> {
        let libraries = self.parse_library_folders()?;
        let mut games = Vec::new();

        for library in &libraries {
            let steamapps = library.join("steamapps");
            if !steamapps.is_dir() {
                continue;
            }

            let entries = fs::read_dir(&steamapps)
                .map_err(|e| anyhow::anyhow!("读取 steamapps 目录失败: {}", e))?;

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if file_name.starts_with("appmanifest_") && file_name.ends_with(".acf") {
                    if let Ok(game) = self.parse_app_manifest(&path, library) {
                        games.push(game);
                    }
                }
            }
        }

        Ok(games)
    }

    /// 解析 appmanifest_*.acf 文件
    fn parse_app_manifest(&self, path: &Path, library_path: &Path) -> Result<SteamGame> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("读取 appmanifest 失败: {}", e))?;

        let mut app_id = 0u32;
        let mut name = String::new();
        let mut install_dir = String::new();
        let mut size_bytes = None;
        let mut last_played = None;

        let mut in_app_state = false;
        let mut brace_depth = 0;

        for line in content.lines() {
            let line = line.trim();

            if line == "\"AppState\"" {
                in_app_state = true;
                continue;
            }

            if in_app_state {
                if line == "{" {
                    brace_depth += 1;
                    continue;
                }
                if line == "}" {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        in_app_state = false;
                    }
                    continue;
                }

                if let Some((key, value)) = Self::parse_vdf_line(line) {
                    match key.as_str() {
                        "appid" => app_id = value.parse().unwrap_or(0),
                        "name" => name = value,
                        "installdir" => install_dir = value,
                        "SizeOnDisk" => {
                            size_bytes = value.parse().ok();
                        }
                        "LastPlayed" => {
                            last_played = value.parse().ok();
                        }
                        _ => {}
                    }
                }
            }
        }

        let install_path = library_path
            .join("steamapps")
            .join("common")
            .join(&install_dir);

        let icon_path = self.find_steam_icon(app_id, library_path);

        Ok(SteamGame {
            app_id,
            name,
            install_dir,
            install_path: install_path.to_string_lossy().to_string(),
            library_path: library_path.to_string_lossy().to_string(),
            icon_path,
            size_bytes,
            last_played,
        })
    }

    /// 解析 VDF 行
    fn parse_vdf_line(line: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() != 2 {
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() != 2 {
                return None;
            }
            let key = parts[0].trim_matches('"').to_string();
            let value = parts[1].trim_matches('"').trim().to_string();
            return Some((key, value));
        }
        let key = parts[0].trim_matches('"').to_string();
        let value = parts[1].trim_matches('"').trim().to_string();
        Some((key, value))
    }

    /// 查找 Steam 游戏图标
    fn find_steam_icon(&self, app_id: u32, library_path: &Path) -> Option<String> {
        let cache_dir = library_path
            .join("steamapps")
            .join("appcache")
            .join("librarycache");

        if !cache_dir.is_dir() {
            return None;
        }

        let icon_names = [
            format!("{}_icon.jpg", app_id),
            format!("{}_header.jpg", app_id),
            format!("{}_hero.jpg", app_id),
            format!("{}.jpg", app_id),
        ];

        for name in &icon_names {
            let path = cache_dir.join(name);
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }

        None
    }

    /// 获取 Steam 游戏统计信息
    pub fn get_library_stats(&self) -> Vec<SteamLibrary> {
        let info = match &self.install_info {
            Some(i) => i,
            None => return Vec::new(),
        };

        info.libraries
            .iter()
            .map(|lib| {
                let steamapps = lib.join("steamapps");
                let game_count = if steamapps.is_dir() {
                    fs::read_dir(&steamapps)
                        .map(|entries| {
                            entries
                                .flatten()
                                .filter(|e| {
                                    let name = e
                                        .file_name()
                                        .to_string_lossy()
                                        .to_lowercase();
                                    name.starts_with("appmanifest_")
                                        && name.ends_with(".acf")
                                })
                                .count()
                        })
                        .unwrap_or(0)
                } else {
                    0
                };

                SteamLibrary {
                    path: lib.to_string_lossy().to_string(),
                    game_count,
                }
            })
            .collect()
    }
}

impl Default for SteamScanner {
    fn default() -> Self {
        Self::new()
    }
}
