use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    pub vendor_id: u32,
    pub product_id: u32,
    pub serial_number: u32,
    pub display_name: String,
    pub original_existed: bool,
    pub backup_file: String,
    pub original_sha256: Option<String>,
    pub installed_sha256: String,
}

impl InstallRecord {
    pub fn key(&self) -> String {
        format!("{:04x}-{:04x}", self.vendor_id, self.product_id)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OptimizerState {
    #[serde(default)]
    pub installations: Vec<InstallRecord>,
}

pub struct OptimizerStore {
    root: PathBuf,
    state_path: PathBuf,
}

impl OptimizerStore {
    pub fn new(root: PathBuf) -> Self {
        let state_path = root.join("state.json");
        Self { root, state_path }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load(&self) -> anyhow::Result<OptimizerState> {
        let bytes = match fs::read(&self.state_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(OptimizerState::default());
            }
            Err(error) => return Err(error).context("读取外接屏优化状态失败"),
        };
        serde_json::from_slice(&bytes).context("外接屏优化状态格式无效")
    }

    pub fn save(&self, state: &OptimizerState) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root).context("创建外接屏优化配置目录失败")?;
        let bytes = serde_json::to_vec_pretty(state).context("编码外接屏优化状态失败")?;
        atomic_write(&self.state_path, &bytes)
    }

    pub fn backup_path(&self, key: &str) -> PathBuf {
        self.root.join("backups").join(format!("{key}.plist"))
    }

    pub fn staged_path(&self, key: &str) -> PathBuf {
        self.root.join("staging").join(format!("{key}.plist"))
    }

    pub fn write_backup(&self, key: &str, bytes: &[u8]) -> anyhow::Result<PathBuf> {
        let path = self.backup_path(key);
        atomic_write(&path, bytes)?;
        Ok(path)
    }

    pub fn write_staged(&self, key: &str, bytes: &[u8]) -> anyhow::Result<PathBuf> {
        let path = self.staged_path(key);
        atomic_write(&path, bytes)?;
        Ok(path)
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("目标路径没有父目录")?;
    fs::create_dir_all(parent).with_context(|| format!("创建目录失败: {}", parent.display()))?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).with_context(|| format!("写入临时文件失败: {}", temp.display()))?;
    fs::rename(&temp, path).with_context(|| format!("提交文件失败: {}", path.display()))?;
    ensure!(path.is_file(), "文件提交后不存在: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(label: &str) -> OptimizerStore {
        let root = std::env::temp_dir().join(format!(
            "qingqi-display-optimizer-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        OptimizerStore::new(root)
    }

    #[test]
    fn state_round_trip_preserves_record() {
        let store = test_store("state");
        let state = OptimizerState {
            installations: vec![InstallRecord {
                vendor_id: 0x5a63,
                product_id: 0x8432,
                serial_number: 1,
                display_name: "VX2778".into(),
                original_existed: true,
                backup_file: "backup.plist".into(),
                original_sha256: Some("before".into()),
                installed_sha256: "after".into(),
            }],
        };
        store.save(&state).expect("save state");
        assert_eq!(
            store.load().expect("load state").installations,
            state.installations
        );
        let _ = fs::remove_dir_all(store.root());
    }

    #[test]
    fn backup_is_byte_exact() {
        let store = test_store("backup");
        let bytes = b"original override bytes";
        let path = store
            .write_backup("5a63-8432", bytes)
            .expect("write backup");
        assert_eq!(fs::read(path).expect("read backup"), bytes);
        let _ = fs::remove_dir_all(store.root());
    }
}
