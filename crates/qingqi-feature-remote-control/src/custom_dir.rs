//! 自定义目录管理 - 用户指定扫描目录

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 自定义扫描目录配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomDir {
    pub id: String,
    pub path: String,
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub max_depth: u32,
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,
    #[serde(default = "default_bool_true")]
    pub recursive: bool,
    pub created_at: i64,
    pub last_scanned: Option<i64>,
}

fn default_extensions() -> Vec<String> {
    vec!["exe".to_string(), "lnk".to_string()]
}

fn default_bool_true() -> bool {
    true
}

/// 自定义目录配置集合
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CustomDirConfig {
    #[serde(default)]
    pub dirs: Vec<CustomDir>,
}

/// 添加自定义目录请求
#[derive(Clone, Debug, Deserialize)]
pub struct AddCustomDirRequest {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_bool_true")]
    pub enabled: bool,
    #[serde(default)]
    pub max_depth: u32,
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
    #[serde(default = "default_bool_true")]
    pub recursive: bool,
}

/// 更新自定义目录请求
#[derive(Clone, Debug, Deserialize)]
pub struct UpdateCustomDirRequest {
    pub path: Option<String>,
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub max_depth: Option<u32>,
    pub extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

/// 目录验证结果
#[derive(Clone, Debug, Serialize)]
pub struct DirValidationResult {
    pub valid: bool,
    pub exists: bool,
    pub is_dir: bool,
    pub readable: bool,
    pub exe_count: usize,
    pub error: Option<String>,
}

/// 自定义目录管理器
pub struct CustomDirManager {
    config: CustomDirConfig,
    config_path: std::path::PathBuf,
}

impl CustomDirManager {
    pub fn new(config_path: std::path::PathBuf) -> Self {
        let config = Self::load_config(&config_path).unwrap_or_default();
        Self {
            config,
            config_path,
        }
    }

    /// 加载配置
    fn load_config(path: &Path) -> Option<CustomDirConfig> {
        if !path.exists() {
            return None;
        }
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// 保存配置
    fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// 获取所有自定义目录
    pub fn list(&self) -> &[CustomDir] {
        &self.config.dirs
    }

    /// 获取启用的目录
    pub fn list_enabled(&self) -> Vec<&CustomDir> {
        self.config.dirs.iter().filter(|d| d.enabled).collect()
    }

    /// 添加自定义目录
    pub fn add(&mut self, req: AddCustomDirRequest) -> Result<CustomDir> {
        let path = Path::new(&req.path);
        if !path.exists() {
            anyhow::bail!("路径不存在: {}", req.path);
        }
        if !path.is_dir() {
            anyhow::bail!("路径不是目录: {}", req.path);
        }

        if self.config.dirs.iter().any(|d| d.path == req.path) {
            anyhow::bail!("目录已存在: {}", req.path);
        }

        let dir = CustomDir {
            id: format!("custom_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            path: req.path.clone(),
            name: req.name.unwrap_or_else(|| {
                Path::new(&req.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("未命名")
                    .to_string()
            }),
            enabled: req.enabled,
            max_depth: req.max_depth,
            extensions: req.extensions.unwrap_or_else(default_extensions),
            recursive: req.recursive,
            created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            last_scanned: None,
        };

        self.config.dirs.push(dir.clone());
        self.save()?;

        Ok(dir)
    }

    /// 更新自定义目录
    pub fn update(&mut self, id: &str, req: UpdateCustomDirRequest) -> Result<CustomDir> {
        let dir = self
            .config
            .dirs
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| anyhow::anyhow!("目录不存在: {}", id))?;

        if let Some(path) = req.path {
            dir.path = path;
        }
        if let Some(name) = req.name {
            dir.name = name;
        }
        if let Some(enabled) = req.enabled {
            dir.enabled = enabled;
        }
        if let Some(max_depth) = req.max_depth {
            dir.max_depth = max_depth;
        }
        if let Some(extensions) = req.extensions {
            dir.extensions = extensions;
        }
        if let Some(recursive) = req.recursive {
            dir.recursive = recursive;
        }

        let dir = dir.clone();
        self.save()?;

        Ok(dir)
    }

    /// 删除自定义目录
    pub fn remove(&mut self, id: &str) -> Result<()> {
        let len_before = self.config.dirs.len();
        self.config.dirs.retain(|d| d.id != id);
        if self.config.dirs.len() == len_before {
            anyhow::bail!("目录不存在: {}", id);
        }
        self.save()
    }

    /// 验证目录
    pub fn validate(path: &str) -> DirValidationResult {
        let path = Path::new(path);

        if !path.exists() {
            return DirValidationResult {
                valid: false,
                exists: false,
                is_dir: false,
                readable: false,
                exe_count: 0,
                error: Some("路径不存在".to_string()),
            };
        }

        if !path.is_dir() {
            return DirValidationResult {
                valid: false,
                exists: true,
                is_dir: false,
                readable: false,
                exe_count: 0,
                error: Some("路径不是目录".to_string()),
            };
        }

        match fs::read_dir(path) {
            Ok(entries) => {
                let exe_count = entries
                    .flatten()
                    .filter(|e| {
                        e.path()
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext.eq_ignore_ascii_case("exe"))
                            .unwrap_or(false)
                    })
                    .count();

                DirValidationResult {
                    valid: true,
                    exists: true,
                    is_dir: true,
                    readable: true,
                    exe_count,
                    error: None,
                }
            }
            Err(e) => DirValidationResult {
                valid: false,
                exists: true,
                is_dir: true,
                readable: false,
                exe_count: 0,
                error: Some(format!("无法读取目录: {}", e)),
            },
        }
    }

    /// 扫描自定义目录生成 AppEntry
    pub fn scan(&self) -> Vec<super::service::app_scanner::AppEntry> {
        let mut apps = Vec::new();

        for dir in self.list_enabled() {
            let path = Path::new(&dir.path);
            if !path.exists() || !path.is_dir() {
                continue;
            }

            if dir.recursive {
                self.scan_recursive(path, dir, &mut apps, 0);
            } else {
                self.scan_flat(path, dir, &mut apps);
            }
        }

        apps
    }

    fn scan_recursive(
        &self,
        dir: &Path,
        config: &CustomDir,
        apps: &mut Vec<super::service::app_scanner::AppEntry>,
        depth: u32,
    ) {
        if config.max_depth > 0 && depth > config.max_depth {
            return;
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_recursive(&path, config, apps, depth + 1);
            } else if self.should_include_file(&path, config) {
                if let Some(app) = self.path_to_app_entry(&path, config) {
                    apps.push(app);
                }
            }
        }
    }

    fn scan_flat(
        &self,
        dir: &Path,
        config: &CustomDir,
        apps: &mut Vec<super::service::app_scanner::AppEntry>,
    ) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && self.should_include_file(&path, config) {
                if let Some(app) = self.path_to_app_entry(&path, config) {
                    apps.push(app);
                }
            }
        }
    }

    fn should_include_file(&self, path: &Path, config: &CustomDir) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| config.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
            .unwrap_or(false)
    }

    fn path_to_app_entry(
        &self,
        path: &Path,
        config: &CustomDir,
    ) -> Option<super::service::app_scanner::AppEntry> {
        let name = path.file_stem().and_then(|s| s.to_str())?.to_string();
        let original = name.clone();

        let category = if path.extension().and_then(|e| e.to_str()) == Some("lnk") {
            "Application".to_string()
        } else {
            "Game".to_string()
        };

        Some(super::service::app_scanner::AppEntry {
            id: format!("custom:{}:{}", config.id, path.display()),
            name,
            original_name: original,
            exe_path: path.to_str()?.to_string(),
            icon_base64: None,
            category,
            source: "Custom".to_string(),
        })
    }
}
