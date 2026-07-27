//! Application scanner - finds installed applications on Windows.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub original_name: String,
    pub exe_path: String,
    pub icon_base64: Option<String>,
    pub category: String,
    pub source: String,
}

pub use crate::platform::set_priority;

pub struct AppScanner {
    renames: Arc<Mutex<HashMap<String, String>>>,
    cache: Arc<Mutex<Option<Vec<AppEntry>>>>,
    steam_scanner: Arc<Mutex<crate::steam::SteamScanner>>,
    steam_cache: Arc<Mutex<Option<Vec<crate::steam::SteamGame>>>>,
    custom_dir_manager: Arc<Mutex<crate::custom_dir::CustomDirManager>>,
}

impl AppScanner {
    pub fn new(config_path: PathBuf) -> Self {
        let custom_dir_manager = crate::custom_dir::CustomDirManager::new(config_path);
        Self {
            renames: Arc::new(Mutex::new(HashMap::new())),
            cache: Arc::new(Mutex::new(None)),
            steam_scanner: Arc::new(Mutex::new(crate::steam::SteamScanner::new())),
            steam_cache: Arc::new(Mutex::new(None)),
            custom_dir_manager: Arc::new(Mutex::new(custom_dir_manager)),
        }
    }

    pub fn scan(&self) -> Vec<AppEntry> {
        let mut apps = Vec::new();
        let renames = self.renames.lock().unwrap();
        apps.extend(self.scan_start_menu(&renames));
        apps.extend(self.scan_game_dirs(&renames));
        apps.extend(self.scan_steam(&renames));
        apps.extend(self.scan_custom_dirs());
        let mut cache = self.cache.lock().unwrap();
        *cache = Some(apps.clone());
        apps
    }

    pub fn get_or_scan(&self) -> Vec<AppEntry> {
        let cache = self.cache.lock().unwrap();
        if let Some(apps) = cache.as_ref() { return apps.clone(); }
        drop(cache);
        self.scan()
    }

    pub fn rename(&self, original_name: &str, new_name: &str) {
        let mut renames = self.renames.lock().unwrap();
        renames.insert(original_name.to_string(), new_name.to_string());
        let mut cache = self.cache.lock().unwrap();
        *cache = None;
    }

    fn scan_start_menu(&self, renames: &HashMap<String, String>) -> Vec<AppEntry> {
        let mut apps = Vec::new();
        let paths = [
            PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
                .join("Microsoft").join("Windows").join("Start Menu").join("Programs"),
            PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_default())
                .join("Microsoft").join("Windows").join("Start Menu").join("Programs"),
        ];
        for base in &paths {
            if base.exists() { self.scan_dir(base, renames, &mut apps); }
        }
        apps
    }

    fn scan_dir(&self, dir: &std::path::Path, renames: &HashMap<String, String>, apps: &mut Vec<AppEntry>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.scan_dir(&path, renames, apps);
                } else if path.extension().and_then(|e| e.to_str()) == Some("lnk") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        let original = name.to_string();
                        let display = renames.get(&original).cloned().unwrap_or_else(|| original.clone());
                        apps.push(AppEntry {
                            id: format!("startmenu:{}", path.display()),
                            name: display,
                            original_name: original,
                            exe_path: path.to_str().unwrap_or("").to_string(),
                            icon_base64: None,
                            category: "Application".to_string(),
                            source: "StartMenu".to_string(),
                        });
                    }
                }
            }
        }
    }

    fn scan_game_dirs(&self, renames: &HashMap<String, String>) -> Vec<AppEntry> {
        let mut apps = Vec::new();
        for dir in &[PathBuf::from("C:\\Games"), PathBuf::from("D:\\Games"), PathBuf::from("E:\\Games")] {
            if dir.exists() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(exe) = Self::find_main_exe(&path) {
                                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                                let original = name.clone();
                                let display = renames.get(&original).cloned().unwrap_or_else(|| original.clone());
                                apps.push(AppEntry {
                                    id: format!("dir:{}", path.display()),
                                    name: display,
                                    original_name: original,
                                    exe_path: exe,
                                    icon_base64: None,
                                    category: "Game".to_string(),
                                    source: "Local".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        apps
    }

    fn find_main_exe(dir: &std::path::Path) -> Option<String> {
        let dir_name = dir.file_stem()?.to_str()?;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                    let exe_name = path.file_stem()?.to_str()?;
                    if exe_name.eq_ignore_ascii_case(dir_name) {
                        return Some(path.to_str()?.to_string());
                    }
                }
            }
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                    return Some(path.to_str()?.to_string());
                }
            }
        }
        None
    }

    /// 扫描 Steam 游戏
    fn scan_steam(&self, renames: &HashMap<String, String>) -> Vec<AppEntry> {
        let mut scanner = self.steam_scanner.lock().unwrap();
        let mut steam_cache = self.steam_cache.lock().unwrap();

        if let Some(games) = steam_cache.as_ref() {
            return games
                .iter()
                .map(|g| self.steam_game_to_app_entry(g, renames))
                .collect();
        }

        // 先查找 Steam 安装，再扫描游戏
        let installed = scanner.find_steam_install().is_some();
        let games = if installed {
            scanner.scan_games().unwrap_or_default()
        } else {
            Vec::new()
        };

        let apps: Vec<AppEntry> = games
            .iter()
            .map(|g| self.steam_game_to_app_entry(g, renames))
            .collect();

        *steam_cache = Some(games);
        apps
    }

    fn steam_game_to_app_entry(
        &self,
        game: &crate::steam::SteamGame,
        renames: &HashMap<String, String>,
    ) -> AppEntry {
        let original = game.name.clone();
        let display = renames
            .get(&original)
            .cloned()
            .unwrap_or_else(|| original.clone());

        // 查找游戏目录下的主可执行文件
        let exe_path = Self::find_steam_game_exe(&game.install_path, &game.name)
            .unwrap_or_else(|| game.install_path.clone());

        AppEntry {
            id: format!("steam:{}", game.app_id),
            name: display,
            original_name: original,
            exe_path,
            icon_base64: None,
            category: "Game".to_string(),
            source: "Steam".to_string(),
        }
    }

    /// 查找 Steam 游戏的主可执行文件
    fn find_steam_game_exe(install_path: &str, game_name: &str) -> Option<String> {
        let dir = std::path::Path::new(install_path);
        if !dir.is_dir() {
            return None;
        }

        // 1. 尝试在根目录查找与游戏名匹配的 exe
        let game_name_normalized = game_name.replace([' ', ':', '-', '.'], "").to_lowercase();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                    let exe_name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .replace([' ', ':', '-', '.'], "")
                        .to_lowercase();
                    if exe_name == game_name_normalized {
                        return Some(path.to_str()?.to_string());
                    }
                }
            }
        }

        // 2. 尝试在根目录查找任何 exe
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                    return Some(path.to_str()?.to_string());
                }
            }
        }

        // 3. 递归搜索子目录（深度 2）
        Self::find_exe_recursive(dir, 2)
    }

    fn find_exe_recursive(dir: &std::path::Path, max_depth: u32) -> Option<String> {
        if max_depth == 0 {
            return None;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("exe") {
                    return Some(path.to_str()?.to_string());
                }
            }

            for entry in std::fs::read_dir(dir).ok()?.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(exe) = Self::find_exe_recursive(&path, max_depth - 1) {
                        return Some(exe);
                    }
                }
            }
        }
        None
    }

    /// 扫描自定义目录
    fn scan_custom_dirs(&self) -> Vec<AppEntry> {
        let manager = self.custom_dir_manager.lock().unwrap();
        manager.scan()
    }

    /// 刷新 Steam 缓存
    pub fn refresh_steam(&self) -> Vec<crate::steam::SteamGame> {
        let mut scanner = self.steam_scanner.lock().unwrap();
        let mut steam_cache = self.steam_cache.lock().unwrap();

        let installed = scanner.find_steam_install().is_some();
        let games = if installed {
            scanner.scan_games().unwrap_or_default()
        } else {
            Vec::new()
        };

        *steam_cache = Some(games.clone());

        let mut cache = self.cache.lock().unwrap();
        *cache = None;

        games
    }

    /// 获取 Steam 游戏列表
    pub fn get_steam_games(&self) -> Vec<crate::steam::SteamGame> {
        let mut scanner = self.steam_scanner.lock().unwrap();
        let mut steam_cache = self.steam_cache.lock().unwrap();

        if let Some(games) = steam_cache.as_ref() {
            return games.clone();
        }

        let installed = scanner.find_steam_install().is_some();
        let games = if installed {
            scanner.scan_games().unwrap_or_default()
        } else {
            Vec::new()
        };

        *steam_cache = Some(games.clone());
        games
    }

    /// 获取 Steam 库统计
    pub fn get_steam_libraries(&self) -> Vec<crate::steam::SteamLibrary> {
        let scanner = self.steam_scanner.lock().unwrap();
        scanner.get_library_stats()
    }

    /// 检查 Steam 是否已安装
    pub fn is_steam_installed(&self) -> bool {
        let mut scanner = self.steam_scanner.lock().unwrap();
        scanner.find_steam_install().is_some()
    }

    /// 获取 Steam 安装路径
    pub fn get_steam_path(&self) -> Option<String> {
        let mut scanner = self.steam_scanner.lock().unwrap();
        scanner
            .find_steam_install()
            .map(|info| info.root_path.to_string_lossy().to_string())
    }

    /// 获取自定义目录管理器
    pub fn custom_dir_manager(&self) -> &Mutex<crate::custom_dir::CustomDirManager> {
        &self.custom_dir_manager
    }
}

impl Default for AppScanner {
    fn default() -> Self {
        Self::new(PathBuf::from(""))
    }
}
