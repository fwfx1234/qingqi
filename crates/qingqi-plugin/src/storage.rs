use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct AppPaths {
    data_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        let data_dir = match env::var_os("QINGQI_DATA_DIR") {
            Some(value) => PathBuf::from(value),
            None => dirs::data_dir()
                .context("cannot resolve system data directory")?
                .join("qingqi"),
        };
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("cannot create data directory {}", data_dir.display()))?;
        Ok(Self { data_dir })
    }

    pub fn for_test(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        self.data_dir.as_path()
    }

    pub fn database(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    pub fn config(&self, name: &str) -> PathBuf {
        let dir = self.data_dir.join("config");
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    pub fn log_file(&self, name: &str) -> PathBuf {
        let dir = self.data_dir.join("logs");
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    pub fn feature_dir(&self, feature: &str) -> PathBuf {
        let dir = self.data_dir.join("features").join(feature);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn feature_output_dir(&self, feature: &str) -> PathBuf {
        let dir = self.feature_dir(feature).join("output");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn feature_state(&self, feature: &str, name: &str) -> PathBuf {
        let dir = self.feature_dir(feature).join("state");
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    pub fn imported_plugins_dir(&self) -> PathBuf {
        let dir = self.data_dir.join("plugins").join("imported");
        let _ = fs::create_dir_all(&dir);
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_paths(label: &str) -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-storage-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir");
        AppPaths::for_test(dir)
    }

    #[test]
    fn imported_plugins_dir_creates_directory() {
        let paths = temp_paths("plugins");
        let dir = paths.imported_plugins_dir();
        assert!(dir.is_dir(), "imported plugins dir should exist");
        assert!(
            dir.ends_with("plugins/imported"),
            "should end with plugins/imported, got {}",
            dir.display()
        );
    }
}
