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
}

impl AppScanner {
    pub fn new() -> Self {
        Self {
            renames: Arc::new(Mutex::new(HashMap::new())),
            cache: Arc::new(Mutex::new(None)),
        }
    }

    pub fn scan(&self) -> Vec<AppEntry> {
        let mut apps = Vec::new();
        let renames = self.renames.lock().unwrap();
        apps.extend(self.scan_start_menu(&renames));
        apps.extend(self.scan_game_dirs(&renames));
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
}

impl Default for AppScanner {
    fn default() -> Self { Self::new() }
}
