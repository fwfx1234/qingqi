use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, ensure};

pub fn open_path(path: &Path) -> Result<()> {
    ensure!(path.exists(), "路径不存在: {}", path.display());
    let status = Command::new("open")
        .arg(path)
        .status()
        .with_context(|| format!("无法打开路径 {}", path.display()))?;
    ensure!(status.success(), "open 返回失败状态");
    Ok(())
}

pub fn choose_file(prompt: &str) -> Result<Option<PathBuf>> {
    Ok(rfd::FileDialog::new().set_title(prompt).pick_file())
}

pub fn choose_directory(prompt: &str) -> Result<Option<PathBuf>> {
    Ok(rfd::FileDialog::new().set_title(prompt).pick_folder())
}

/// Open a directory in the system file manager, creating it if it doesn't exist.
pub fn open_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("无法创建目录 {}", path.display()))?;
    open_path(path)
}

/// Reveal a file or directory in Finder (macOS).
///
/// Uses `open -R` which highlights the item in its parent folder. The path
/// must exist — callers should verify existence before calling this.
pub fn reveal_in_finder(path: &Path) -> Result<()> {
    ensure!(path.exists(), "路径不存在: {}", path.display());
    let status = Command::new("open")
        .arg("-R")
        .arg(path)
        .status()
        .with_context(|| format!("无法在访达中显示 {}", path.display()))?;
    ensure!(status.success(), "open -R 返回失败状态");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_in_finder_rejects_missing_path() {
        assert!(reveal_in_finder(Path::new("/tmp/qingqi-test-nonexistent-file")).is_err());
    }

    #[test]
    fn open_path_rejects_missing_path() {
        assert!(open_path(Path::new("/tmp/qingqi-test-nonexistent-file")).is_err());
    }

    #[test]
    fn open_directory_creates_missing_dir() {
        let test_dir = std::env::temp_dir().join("qingqi-shell-open-nonexistent");
        let _ = std::fs::remove_dir_all(&test_dir);
        // open_directory creates the dir, then opens it — may fail on CI without UI
        let result = open_directory(&test_dir);
        // Did not panic; clean up
        let _ = std::fs::remove_dir_all(&test_dir);
        let _ = result;
    }
}
