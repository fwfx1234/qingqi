use anyhow::Context;

use crate::model::{ApiEnvironment, ApiGroup};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ApiWorkspace {
    pub groups: Vec<ApiGroup>,
    pub environments: Vec<ApiEnvironment>,
}

impl ApiWorkspace {
    pub fn new(groups: Vec<ApiGroup>, environments: Vec<ApiEnvironment>) -> Self {
        Self {
            groups,
            environments,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ApiWorkspaceStore {
    path: std::path::PathBuf,
}

impl ApiWorkspaceStore {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> anyhow::Result<Option<ApiWorkspace>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&self.path)
            .with_context(|| format!("无法读取 API 工作区 {}", self.path.display()))?;
        let workspace = serde_json::from_str::<ApiWorkspace>(&raw)
            .with_context(|| format!("无法解析 API 工作区 {}", self.path.display()))?;
        Ok(Some(workspace))
    }

    pub fn save(&self, workspace: &ApiWorkspace) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建 API 工作区目录 {}", parent.display()))?;
        }
        let raw = serde_json::to_string_pretty(workspace)?;
        std::fs::write(&self.path, raw)
            .with_context(|| format!("无法写入 API 工作区 {}", self.path.display()))
    }
}
