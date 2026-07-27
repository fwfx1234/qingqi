use std::{
    borrow::Cow,
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use gpui::{AssetSource, Result, SharedString};
use include_dir::{Dir, include_dir};

static LUCIDE_ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/lucide");
const LUCIDE_VERSION: &str = include_str!("../assets/LUCIDE_VERSION");

const ASSETS_DIR: &str = "assets";

const EMBEDDED_ASSETS: &[(&str, &[u8])] = &[
    (
        "app-icon.svg",
        include_bytes!("../../qingqi/assets/app-icon.svg"),
    ),
    (
        "app_icon_128.png",
        include_bytes!("../../qingqi/assets/app_icon_128.png"),
    ),
    (
        "app_icon_16.png",
        include_bytes!("../../qingqi/assets/app_icon_16.png"),
    ),
    (
        "app_icon_256.png",
        include_bytes!("../../qingqi/assets/app_icon_256.png"),
    ),
    (
        "app_icon_32.png",
        include_bytes!("../../qingqi/assets/app_icon_32.png"),
    ),
    (
        "app_icon_512.png",
        include_bytes!("../../qingqi/assets/app_icon_512.png"),
    ),
    (
        "app_icon_64.png",
        include_bytes!("../../qingqi/assets/app_icon_64.png"),
    ),
    (
        "icons/about.svg",
        include_bytes!("../../qingqi/assets/icons/about.svg"),
    ),
    (
        "icons/antenna.svg",
        include_bytes!("../../qingqi/assets/icons/antenna.svg"),
    ),
    (
        "icons/api.svg",
        include_bytes!("../../qingqi/assets/icons/api.svg"),
    ),
    (
        "icons/bolt.svg",
        include_bytes!("../../qingqi/assets/icons/bolt.svg"),
    ),
    (
        "icons/capture.svg",
        include_bytes!("../../qingqi/assets/icons/capture.svg"),
    ),
    (
        "icons/chevron-down.svg",
        include_bytes!("../../qingqi/assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-up.svg",
        include_bytes!("../../qingqi/assets/icons/chevron-up.svg"),
    ),
    (
        "icons/clipboard.svg",
        include_bytes!("../../qingqi/assets/icons/clipboard.svg"),
    ),
    (
        "icons/download.svg",
        include_bytes!("../../qingqi/assets/icons/download.svg"),
    ),
    (
        "icons/folder-network.svg",
        include_bytes!("../../qingqi/assets/icons/folder-network.svg"),
    ),
    (
        "icons/image.svg",
        include_bytes!("../../qingqi/assets/icons/image.svg"),
    ),
    (
        "icons/json.svg",
        include_bytes!("../../qingqi/assets/icons/json.svg"),
    ),
    (
        "icons/qr.svg",
        include_bytes!("../../qingqi/assets/icons/qr.svg"),
    ),
    (
        "icons/remote.svg",
        include_bytes!("../../qingqi/assets/icons/remote.svg"),
    ),
    (
        "icons/school.svg",
        include_bytes!("../../qingqi/assets/icons/school.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../../qingqi/assets/icons/settings.svg"),
    ),
    (
        "icons/shield-eye.svg",
        include_bytes!("../../qingqi/assets/icons/shield-eye.svg"),
    ),
    (
        "tray-icon.svg",
        include_bytes!("../../qingqi/assets/tray-icon.svg"),
    ),
];

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

fn normalize(relative: impl AsRef<Path>) -> String {
    relative.as_ref().to_string_lossy().replace('\\', "/")
}

fn embedded_path(relative: &str) -> Option<&str> {
    let normalized = relative.trim_start_matches("./").trim_start_matches('/');
    if EMBEDDED_ASSETS.iter().any(|(path, _)| *path == normalized) {
        return Some(normalized);
    }

    if let Some(lucide_path) = normalized.strip_prefix("lucide/")
        && LUCIDE_ASSETS.get_file(lucide_path).is_some()
    {
        return Some(normalized);
    }

    if !normalized.contains('/') {
        let icon_path = format!("icons/{normalized}");
        if let Some((path, _)) = EMBEDDED_ASSETS.iter().find(|(path, _)| *path == icon_path) {
            return Some(*path);
        }
    }

    None
}

pub fn embedded(relative: impl AsRef<Path>) -> Option<&'static [u8]> {
    let normalized = normalize(relative);
    let path = embedded_path(&normalized)?;
    if let Some(lucide_path) = path.strip_prefix("lucide/") {
        return LUCIDE_ASSETS
            .get_file(lucide_path)
            .map(|file| file.contents());
    }
    EMBEDDED_ASSETS
        .iter()
        .find_map(|(asset_path, bytes)| (*asset_path == path).then_some(*bytes))
}

pub fn lucide_version() -> &'static str {
    LUCIDE_VERSION.trim()
}

pub fn lucide_paths() -> impl Iterator<Item = String> {
    LUCIDE_ASSETS
        .files()
        .map(|file| format!("lucide/{}", file.path().to_string_lossy()))
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

    let path = normalize(relative);
    if embedded_path(&path).is_some() {
        return path;
    }

    resolve(relative).to_string_lossy().to_string()
}

pub struct ProjectAssets;

impl AssetSource for ProjectAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = embedded(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }

        for candidate in candidates(path) {
            if let Ok(bytes) = fs::read(candidate) {
                return Ok(Some(Cow::Owned(bytes)));
            }
        }

        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let dir = resolve(path);
        let normalized_dir = normalize(path).trim_end_matches('/').to_string();
        let embedded_prefix = if normalized_dir.is_empty() || normalized_dir == "." {
            String::new()
        } else {
            format!("{normalized_dir}/")
        };
        let mut entries = BTreeSet::new();

        for (asset_path, _) in EMBEDDED_ASSETS {
            if !asset_path.starts_with(&embedded_prefix) {
                continue;
            }
            let remaining = &asset_path[embedded_prefix.len()..];
            if let Some(name) = remaining.split('/').next()
                && !name.is_empty()
            {
                entries.insert(name.to_string());
            }
        }

        if normalized_dir.is_empty() || normalized_dir == "." {
            entries.insert("lucide".to_string());
        } else if normalized_dir == "lucide" {
            entries.extend(
                LUCIDE_ASSETS
                    .files()
                    .filter_map(|file| file.path().file_name()?.to_str().map(str::to_owned)),
            );
        }

        if let Ok(read_dir) = fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    entries.insert(name.to_string());
                }
            }
        }

        Ok(entries.into_iter().map(Into::into).collect())
    }
}
