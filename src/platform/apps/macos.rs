use std::{
    collections::HashSet,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use plist::Value;

use super::{
    InstalledApp, app_aliases, bundle_id_suffix, convert_icon_with_image, icon_cache_path,
};

const SCAN_DIRS: &[&str] = &[
    "/Applications",
    "/System/Applications",
    "/System/Library/CoreServices/Applications",
];
const APP_SCAN_MAX_DEPTH: usize = 4;

pub(super) fn scan_application_metadata() -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    let mut seen_names = HashSet::new();

    for directory in SCAN_DIRS {
        collect_apps(
            Path::new(directory),
            0,
            APP_SCAN_MAX_DEPTH,
            &mut seen_names,
            &mut apps,
        );
    }

    if let Ok(home) = std::env::var("HOME") {
        collect_apps(
            Path::new(&home).join("Applications").as_path(),
            0,
            APP_SCAN_MAX_DEPTH,
            &mut seen_names,
            &mut apps,
        );
    }

    apps.sort_by_key(|left| left.name.to_lowercase());
    apps
}

pub(super) fn scan_application_paths() -> Vec<String> {
    let mut paths = HashSet::new();
    for directory in SCAN_DIRS {
        collect_app_paths(Path::new(directory), 0, APP_SCAN_MAX_DEPTH, &mut paths);
    }
    if let Ok(home) = std::env::var("HOME") {
        collect_app_paths(
            Path::new(&home).join("Applications").as_path(),
            0,
            APP_SCAN_MAX_DEPTH,
            &mut paths,
        );
    }
    let mut list = paths.into_iter().collect::<Vec<_>>();
    list.sort();
    list
}

pub(super) fn populate_application_icons(apps: &mut [InstalledApp]) {
    for app in apps {
        if app.icon_path.is_none() {
            app.icon_path = extract_icon_for_application(Path::new(&app.path));
        }
    }
}

pub(super) fn open_application(path: &str) -> Result<(), String> {
    ProcessCommand::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("启动失败: {error}"))
}

fn collect_apps(
    directory: &Path,
    depth: usize,
    max_depth: usize,
    seen_names: &mut HashSet<String>,
    apps: &mut Vec<InstalledApp>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("app") {
            if is_nested_helper_app(&path) {
                continue;
            }

            collect_apps(
                path.join("Contents").join("Applications").as_path(),
                depth + 1,
                max_depth,
                seen_names,
                apps,
            );
            push_app_metadata(path, seen_names, apps);
            continue;
        }

        if depth < max_depth && path.is_dir() && should_descend_app_scan_dir(&path) {
            collect_apps(&path, depth + 1, max_depth, seen_names, apps);
        }
    }
}

fn push_app_metadata(
    path: PathBuf,
    seen_names: &mut HashSet<String>,
    apps: &mut Vec<InstalledApp>,
) {
    let Some(stem) = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
    else {
        return;
    };

    let info = read_info_dictionary(&path);
    let bundle_id = bundle_id_from_info(&info);
    let name = display_name_from_info(&info, &stem);
    let dedupe_key = bundle_id
        .clone()
        .unwrap_or_else(|| name.clone())
        .to_lowercase();
    if stem.is_empty() || dedupe_key.is_empty() || !seen_names.insert(dedupe_key) {
        return;
    }

    let aliases = app_aliases(
        [
            Some(stem.as_str()),
            info.get("CFBundleDisplayName").and_then(Value::as_string),
            info.get("CFBundleName").and_then(Value::as_string),
            info.get("CFBundleExecutable").and_then(Value::as_string),
            bundle_id.as_deref(),
            bundle_id.as_deref().and_then(bundle_id_suffix),
        ],
        &name,
    );
    apps.push(InstalledApp {
        icon_letter: name
            .chars()
            .next()
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| String::from("A")),
        aliases,
        bundle_id,
        icon_path: None,
        name,
        path: path.to_string_lossy().to_string(),
    });
}

fn collect_app_paths(
    directory: &Path,
    depth: usize,
    max_depth: usize,
    paths: &mut HashSet<String>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("app") {
            if !is_nested_helper_app(&path) {
                collect_app_paths(
                    path.join("Contents").join("Applications").as_path(),
                    depth + 1,
                    max_depth,
                    paths,
                );
                paths.insert(path.to_string_lossy().to_string());
            }
            continue;
        }

        if depth < max_depth && path.is_dir() && should_descend_app_scan_dir(&path) {
            collect_app_paths(&path, depth + 1, max_depth, paths);
        }
    }
}

fn should_descend_app_scan_dir(path: &Path) -> bool {
    !path
        .components()
        .any(|component| os_str_eq_ignore_ascii_case(component.as_os_str(), "Contents"))
}

fn is_nested_helper_app(path: &Path) -> bool {
    path.components()
        .any(|component| os_str_eq_ignore_ascii_case(component.as_os_str(), "Contents"))
        && !has_component_pair(path, "Contents", "Applications")
}

fn has_component_pair(path: &Path, left: &str, right: &str) -> bool {
    let mut saw_left = false;
    for component in path.components() {
        let value = component.as_os_str();
        if saw_left && os_str_eq_ignore_ascii_case(value, right) {
            return true;
        }
        saw_left = os_str_eq_ignore_ascii_case(value, left);
    }
    false
}

fn os_str_eq_ignore_ascii_case(value: &std::ffi::OsStr, expected: &str) -> bool {
    value.to_string_lossy().eq_ignore_ascii_case(expected)
}

fn bundle_id_from_info(info: &plist::Dictionary) -> Option<String> {
    info.get("CFBundleIdentifier")
        .cloned()
        .and_then(|value| value.as_string().map(ToOwned::to_owned))
        .filter(|bundle_id| !bundle_id.is_empty())
}

fn extract_icon_for_application(path: &Path) -> Option<String> {
    let info = read_info_dictionary(path);
    extract_icon(path, &info)
}

fn extract_icon(path: &Path, info: &plist::Dictionary) -> Option<String> {
    let icon_source = find_icon_source(path, info)?;
    convert_icon_to_png(&icon_source, &icon_cache_path(path)).ok()
}

fn read_info_dictionary(path: &Path) -> plist::Dictionary {
    read_info_plist(path)
        .and_then(|plist| plist.as_dictionary().cloned())
        .unwrap_or_default()
}

fn read_info_plist(path: &Path) -> Option<Value> {
    let plist = path.join("Contents").join("Info.plist");
    let bytes = std::fs::read(plist).ok()?;
    Value::from_reader_xml(bytes.as_slice())
        .or_else(|_| Value::from_reader(Cursor::new(bytes)))
        .ok()
}

fn display_name_from_info(info: &plist::Dictionary, stem: &str) -> String {
    super::unique_nonempty_strings([
        info.get("CFBundleDisplayName").and_then(Value::as_string),
        info.get("CFBundleName").and_then(Value::as_string),
        Some(stem),
    ])
    .into_iter()
    .next()
    .unwrap_or_else(|| stem.to_string())
}

fn find_icon_source(app_path: &Path, info: &plist::Dictionary) -> Option<PathBuf> {
    let resources = app_path.join("Contents").join("Resources");
    let mut candidates = Vec::new();

    if let Some(icon_file) = info.get("CFBundleIconFile").and_then(Value::as_string) {
        candidates.push(icon_file.to_string());
    }
    if let Some(icon_name) = info.get("CFBundleIconName").and_then(Value::as_string) {
        candidates.push(icon_name.to_string());
    }
    if let Some(Value::Dictionary(bundle_icons)) = info.get("CFBundleIcons")
        && let Some(Value::Dictionary(primary_icon)) = bundle_icons.get("CFBundlePrimaryIcon")
        && let Some(Value::Array(icon_files)) = primary_icon.get("CFBundleIconFiles")
    {
        for entry in icon_files {
            if let Some(name) = entry.as_string() {
                candidates.push(name.to_string());
            }
        }
    }

    for candidate in candidates {
        if let Some(icon) = resolve_resource_icon(&resources, &candidate) {
            return Some(icon);
        }
    }

    std::fs::read_dir(&resources)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("icns"))
}

fn resolve_resource_icon(resources: &Path, value: &str) -> Option<PathBuf> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }

    let raw_path = Path::new(raw);
    if raw_path.is_absolute() && raw_path.is_file() {
        return Some(raw_path.to_path_buf());
    }

    let mut names = vec![raw.to_string()];
    if raw_path.extension().is_none() {
        names.push(format!("{raw}.icns"));
        names.push(format!("{raw}.png"));
    }

    names
        .into_iter()
        .map(|name| resources.join(name))
        .find(|candidate| candidate.is_file())
}

fn convert_icon_to_png(icon_path: &Path, out_path: &Path) -> Result<String, String> {
    if icon_path.extension().and_then(|ext| ext.to_str()) == Some("icns")
        && let Ok(path) = convert_icns_with_iconutil(icon_path, out_path)
    {
        return Ok(path);
    }

    convert_icon_with_image(icon_path, out_path)
}

fn convert_icns_with_iconutil(icon_path: &Path, out_path: &Path) -> Result<String, String> {
    let iconutil = std::process::Command::new("which")
        .arg("iconutil")
        .output()
        .map_err(|error| error.to_string())?;
    if !iconutil.status.success() {
        return Err(String::from("iconutil unavailable"));
    }

    let temp_dir = std::env::temp_dir().join(format!("qingqi-app-icon-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    let iconset_dir = temp_dir.join("icon.iconset");

    let output = ProcessCommand::new("iconutil")
        .arg("-c")
        .arg("iconset")
        .arg(icon_path)
        .arg("-o")
        .arg(&iconset_dir)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut pngs = std::fs::read_dir(&iconset_dir)
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
        .collect::<Vec<_>>();
    pngs.sort();

    let best = pngs
        .into_iter()
        .max_by_key(|path| {
            image::ImageReader::open(path)
                .ok()
                .and_then(|reader| reader.with_guessed_format().ok())
                .and_then(|reader| reader.decode().ok())
                .map(|image| image.width() * image.height())
                .unwrap_or(0)
        })
        .ok_or_else(|| String::from("no png extracted from icns"))?;

    let result = convert_icon_with_image(&best, out_path);
    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

#[cfg(test)]
mod tests {
    use super::{collect_app_paths, collect_apps, resolve_resource_icon};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-apps-macos-{name}-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn resolve_resource_icon_prefers_existing_named_files() {
        let resources = temp_dir("resources");
        let icon = resources.join("AppIcon.icns");
        fs::write(&icon, b"test").expect("icon file");

        let resolved = resolve_resource_icon(&resources, "AppIcon");
        assert_eq!(resolved.as_deref(), Some(icon.as_path()));
    }

    #[test]
    fn resolve_resource_icon_accepts_png_with_extension() {
        let resources = temp_dir("resources-png");
        let icon = resources.join("AppIcon.png");
        fs::write(&icon, b"test").expect("icon file");

        let resolved = resolve_resource_icon(&resources, "AppIcon.png");
        assert_eq!(resolved.as_deref(), Some(icon.as_path()));
    }

    #[test]
    fn recursive_scan_includes_public_nested_apps_and_skips_helpers() {
        let root = temp_dir("recursive-scan");
        let visible = root.join("Utilities").join("Visible.app");
        let bundled_visible = root
            .join("Xcode.app")
            .join("Contents")
            .join("Applications")
            .join("Instruments.app");
        let helper = root
            .join("Code.app")
            .join("Contents")
            .join("Frameworks")
            .join("Code Helper.app");

        fs::create_dir_all(visible.join("Contents")).expect("visible app");
        fs::create_dir_all(bundled_visible.join("Contents")).expect("bundled app");
        fs::create_dir_all(helper.join("Contents")).expect("helper app");

        let mut apps = Vec::new();
        let mut seen = std::collections::HashSet::new();
        collect_apps(&root, 0, 4, &mut seen, &mut apps);

        assert!(apps.iter().any(|app| app.path == visible.to_string_lossy()));
        assert!(
            apps.iter()
                .any(|app| app.path == bundled_visible.to_string_lossy())
        );
        assert!(
            !apps.iter().any(|app| app.path == helper.to_string_lossy()),
            "helper apps inside Contents should stay hidden"
        );

        let mut paths = std::collections::HashSet::new();
        collect_app_paths(&root, 0, 4, &mut paths);
        assert!(paths.contains(&visible.to_string_lossy().to_string()));
        assert!(paths.contains(&bundled_visible.to_string_lossy().to_string()));
        assert!(!paths.contains(&helper.to_string_lossy().to_string()));
    }
}
