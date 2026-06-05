use std::{
    borrow::Cow,
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use gpui::{AssetSource, Result, SharedString};

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
    // ── 图标 ──────────────────────────────────────────────
    ("icons/a-large-small.svg", include_bytes!("../../qingqi/assets/icons/a-large-small.svg")),
    ("icons/about.svg", include_bytes!("../../qingqi/assets/icons/about.svg")),
    ("icons/antenna.svg", include_bytes!("../../qingqi/assets/icons/antenna.svg")),
    ("icons/api.svg", include_bytes!("../../qingqi/assets/icons/api.svg")),
    ("icons/arrow-down.svg", include_bytes!("../../qingqi/assets/icons/arrow-down.svg")),
    ("icons/arrow-left.svg", include_bytes!("../../qingqi/assets/icons/arrow-left.svg")),
    ("icons/arrow-right.svg", include_bytes!("../../qingqi/assets/icons/arrow-right.svg")),
    ("icons/arrow-up.svg", include_bytes!("../../qingqi/assets/icons/arrow-up.svg")),
    ("icons/asterisk.svg", include_bytes!("../../qingqi/assets/icons/asterisk.svg")),
    ("icons/bell.svg", include_bytes!("../../qingqi/assets/icons/bell.svg")),
    ("icons/bolt.svg", include_bytes!("../../qingqi/assets/icons/bolt.svg")),
    ("icons/book-open.svg", include_bytes!("../../qingqi/assets/icons/book-open.svg")),
    ("icons/bot.svg", include_bytes!("../../qingqi/assets/icons/bot.svg")),
    ("icons/building-2.svg", include_bytes!("../../qingqi/assets/icons/building-2.svg")),
    ("icons/calendar.svg", include_bytes!("../../qingqi/assets/icons/calendar.svg")),
    ("icons/capture.svg", include_bytes!("../../qingqi/assets/icons/capture.svg")),
    ("icons/case-sensitive.svg", include_bytes!("../../qingqi/assets/icons/case-sensitive.svg")),
    ("icons/chart-pie.svg", include_bytes!("../../qingqi/assets/icons/chart-pie.svg")),
    ("icons/check.svg", include_bytes!("../../qingqi/assets/icons/check.svg")),
    ("icons/chevron-down.svg", include_bytes!("../../qingqi/assets/icons/chevron-down.svg")),
    ("icons/chevron-left.svg", include_bytes!("../../qingqi/assets/icons/chevron-left.svg")),
    ("icons/chevron-right.svg", include_bytes!("../../qingqi/assets/icons/chevron-right.svg")),
    ("icons/chevron-up.svg", include_bytes!("../../qingqi/assets/icons/chevron-up.svg")),
    ("icons/chevrons-up-down.svg", include_bytes!("../../qingqi/assets/icons/chevrons-up-down.svg")),
    ("icons/circle-check.svg", include_bytes!("../../qingqi/assets/icons/circle-check.svg")),
    ("icons/circle-user.svg", include_bytes!("../../qingqi/assets/icons/circle-user.svg")),
    ("icons/circle-x.svg", include_bytes!("../../qingqi/assets/icons/circle-x.svg")),
    ("icons/clipboard.svg", include_bytes!("../../qingqi/assets/icons/clipboard.svg")),
    ("icons/close.svg", include_bytes!("../../qingqi/assets/icons/close.svg")),
    ("icons/copy.svg", include_bytes!("../../qingqi/assets/icons/copy.svg")),
    ("icons/dash.svg", include_bytes!("../../qingqi/assets/icons/dash.svg")),
    ("icons/delete.svg", include_bytes!("../../qingqi/assets/icons/delete.svg")),
    ("icons/download.svg", include_bytes!("../../qingqi/assets/icons/download.svg")),
    ("icons/edit.svg", include_bytes!("../../qingqi/assets/icons/edit.svg")),
    ("icons/ellipsis-vertical.svg", include_bytes!("../../qingqi/assets/icons/ellipsis-vertical.svg")),
    ("icons/ellipsis.svg", include_bytes!("../../qingqi/assets/icons/ellipsis.svg")),
    ("icons/external-link.svg", include_bytes!("../../qingqi/assets/icons/external-link.svg")),
    ("icons/eye-off.svg", include_bytes!("../../qingqi/assets/icons/eye-off.svg")),
    ("icons/eye.svg", include_bytes!("../../qingqi/assets/icons/eye.svg")),
    ("icons/file.svg", include_bytes!("../../qingqi/assets/icons/file.svg")),
    ("icons/folder-closed.svg", include_bytes!("../../qingqi/assets/icons/folder-closed.svg")),
    ("icons/folder-network.svg", include_bytes!("../../qingqi/assets/icons/folder-network.svg")),
    ("icons/folder-open.svg", include_bytes!("../../qingqi/assets/icons/folder-open.svg")),
    ("icons/folder.svg", include_bytes!("../../qingqi/assets/icons/folder.svg")),
    ("icons/frame.svg", include_bytes!("../../qingqi/assets/icons/frame.svg")),
    ("icons/gallery-vertical-end.svg", include_bytes!("../../qingqi/assets/icons/gallery-vertical-end.svg")),
    ("icons/github.svg", include_bytes!("../../qingqi/assets/icons/github.svg")),
    ("icons/globe.svg", include_bytes!("../../qingqi/assets/icons/globe.svg")),
    ("icons/heart-off.svg", include_bytes!("../../qingqi/assets/icons/heart-off.svg")),
    ("icons/heart.svg", include_bytes!("../../qingqi/assets/icons/heart.svg")),
    ("icons/history.svg", include_bytes!("../../qingqi/assets/icons/history.svg")),
    ("icons/image.svg", include_bytes!("../../qingqi/assets/icons/image.svg")),
    ("icons/inbox.svg", include_bytes!("../../qingqi/assets/icons/inbox.svg")),
    ("icons/info.svg", include_bytes!("../../qingqi/assets/icons/info.svg")),
    ("icons/inspector.svg", include_bytes!("../../qingqi/assets/icons/inspector.svg")),
    ("icons/json.svg", include_bytes!("../../qingqi/assets/icons/json.svg")),
    ("icons/layout-dashboard.svg", include_bytes!("../../qingqi/assets/icons/layout-dashboard.svg")),
    ("icons/loader-circle.svg", include_bytes!("../../qingqi/assets/icons/loader-circle.svg")),
    ("icons/loader.svg", include_bytes!("../../qingqi/assets/icons/loader.svg")),
    ("icons/map.svg", include_bytes!("../../qingqi/assets/icons/map.svg")),
    ("icons/maximize.svg", include_bytes!("../../qingqi/assets/icons/maximize.svg")),
    ("icons/menu.svg", include_bytes!("../../qingqi/assets/icons/menu.svg")),
    ("icons/minimize.svg", include_bytes!("../../qingqi/assets/icons/minimize.svg")),
    ("icons/minus.svg", include_bytes!("../../qingqi/assets/icons/minus.svg")),
    ("icons/moon.svg", include_bytes!("../../qingqi/assets/icons/moon.svg")),
    ("icons/palette.svg", include_bytes!("../../qingqi/assets/icons/palette.svg")),
    ("icons/panel-bottom-open.svg", include_bytes!("../../qingqi/assets/icons/panel-bottom-open.svg")),
    ("icons/panel-bottom.svg", include_bytes!("../../qingqi/assets/icons/panel-bottom.svg")),
    ("icons/panel-left-close.svg", include_bytes!("../../qingqi/assets/icons/panel-left-close.svg")),
    ("icons/panel-left-open.svg", include_bytes!("../../qingqi/assets/icons/panel-left-open.svg")),
    ("icons/panel-left.svg", include_bytes!("../../qingqi/assets/icons/panel-left.svg")),
    ("icons/panel-right-close.svg", include_bytes!("../../qingqi/assets/icons/panel-right-close.svg")),
    ("icons/panel-right-open.svg", include_bytes!("../../qingqi/assets/icons/panel-right-open.svg")),
    ("icons/panel-right.svg", include_bytes!("../../qingqi/assets/icons/panel-right.svg")),
    ("icons/paste.svg", include_bytes!("../../qingqi/assets/icons/paste.svg")),
    ("icons/plus.svg", include_bytes!("../../qingqi/assets/icons/plus.svg")),
    ("icons/qr.svg", include_bytes!("../../qingqi/assets/icons/qr.svg")),
    ("icons/redo-2.svg", include_bytes!("../../qingqi/assets/icons/redo-2.svg")),
    ("icons/redo.svg", include_bytes!("../../qingqi/assets/icons/redo.svg")),
    ("icons/replace.svg", include_bytes!("../../qingqi/assets/icons/replace.svg")),
    ("icons/resize-corner.svg", include_bytes!("../../qingqi/assets/icons/resize-corner.svg")),
    ("icons/rocket.svg", include_bytes!("../../qingqi/assets/icons/rocket.svg")),
    ("icons/school.svg", include_bytes!("../../qingqi/assets/icons/school.svg")),
    ("icons/search.svg", include_bytes!("../../qingqi/assets/icons/search.svg")),
    ("icons/settings-2.svg", include_bytes!("../../qingqi/assets/icons/settings-2.svg")),
    ("icons/settings.svg", include_bytes!("../../qingqi/assets/icons/settings.svg")),
    ("icons/shield-eye.svg", include_bytes!("../../qingqi/assets/icons/shield-eye.svg")),
    ("icons/smartphone.svg", include_bytes!("../../qingqi/assets/icons/smartphone.svg")),
    ("icons/sort-ascending.svg", include_bytes!("../../qingqi/assets/icons/sort-ascending.svg")),
    ("icons/sort-descending.svg", include_bytes!("../../qingqi/assets/icons/sort-descending.svg")),
    ("icons/square-terminal.svg", include_bytes!("../../qingqi/assets/icons/square-terminal.svg")),
    ("icons/star-off.svg", include_bytes!("../../qingqi/assets/icons/star-off.svg")),
    ("icons/star.svg", include_bytes!("../../qingqi/assets/icons/star.svg")),
    ("icons/sun.svg", include_bytes!("../../qingqi/assets/icons/sun.svg")),
    ("icons/thumbs-down.svg", include_bytes!("../../qingqi/assets/icons/thumbs-down.svg")),
    ("icons/thumbs-up.svg", include_bytes!("../../qingqi/assets/icons/thumbs-up.svg")),
    ("icons/triangle-alert.svg", include_bytes!("../../qingqi/assets/icons/triangle-alert.svg")),
    ("icons/undo-2.svg", include_bytes!("../../qingqi/assets/icons/undo-2.svg")),
    ("icons/undo.svg", include_bytes!("../../qingqi/assets/icons/undo.svg")),
    ("icons/user.svg", include_bytes!("../../qingqi/assets/icons/user.svg")),
    ("icons/window-close.svg", include_bytes!("../../qingqi/assets/icons/window-close.svg")),
    ("icons/window-maximize.svg", include_bytes!("../../qingqi/assets/icons/window-maximize.svg")),
    ("icons/window-minimize.svg", include_bytes!("../../qingqi/assets/icons/window-minimize.svg")),
    ("icons/window-restore.svg", include_bytes!("../../qingqi/assets/icons/window-restore.svg")),
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
    EMBEDDED_ASSETS
        .iter()
        .find_map(|(asset_path, bytes)| (*asset_path == path).then_some(*bytes))
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
