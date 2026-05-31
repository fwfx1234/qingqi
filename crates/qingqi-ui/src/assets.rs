use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
};

use gpui::{AssetSource, Result, SharedString};

const ASSETS_DIR: &str = "assets";

fn workspace_assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../qingqi")
        .join(ASSETS_DIR)
}

/// Returns possible filesystem locations for a bundled project asset.
///
/// Assets are authored under the project `assets/` directory, then may be run
/// from the repo root, from `target/{debug,release}`, or from an app bundle.
pub fn candidates(relative: impl AsRef<Path>) -> Vec<PathBuf> {
    let relative = relative.as_ref();
    let mut paths = Vec::new();

    if relative.is_absolute() {
        paths.push(relative.to_path_buf());
        return paths;
    }

    paths.push(workspace_assets_dir().join(relative));

    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        paths.push(parent.join("../Resources").join(ASSETS_DIR).join(relative));
        paths.push(parent.join(ASSETS_DIR).join(relative));
        paths.push(parent.join("../../").join(ASSETS_DIR).join(relative));
    }

    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join(ASSETS_DIR).join(relative));
    }

    paths
}

pub fn resolve(relative: impl AsRef<Path>) -> PathBuf {
    let relative = relative.as_ref();
    if relative.is_absolute() {
        return relative.to_path_buf();
    }

    candidates(relative)
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| Path::new(ASSETS_DIR).join(relative))
}

pub fn resolve_string(relative: &str) -> String {
    if relative.starts_with("~/") {
        return relative.to_string();
    }

    resolve(relative).to_string_lossy().to_string()
}

pub struct ProjectAssets;

impl AssetSource for ProjectAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        for candidate in candidates(path) {
            if let Ok(bytes) = fs::read(candidate) {
                return Ok(Some(Cow::Owned(bytes)));
            }
        }

        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let dir = resolve(path);
        let mut entries = Vec::new();

        if let Ok(read_dir) = fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    entries.push(name.to_string().into());
                }
            }
        }

        Ok(entries)
    }
}
