use std::path::{Path, PathBuf};

const ASSETS_DIR: &str = "assets";

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

    paths.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(ASSETS_DIR)
            .join(relative),
    );

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
