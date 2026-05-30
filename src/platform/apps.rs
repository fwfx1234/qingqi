use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageReader};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
#[path = "apps/macos.rs"]
mod platform_impl;
#[cfg(target_os = "windows")]
#[path = "apps/windows.rs"]
mod platform_impl;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[path = "apps/unsupported.rs"]
mod platform_impl;

const ICON_SIZE_PX: u32 = 64;
const ICON_CACHE_VERSION: u8 = 2;
const ICON_TRIM_ALPHA_THRESHOLD: u8 = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    pub path: String,
    pub bundle_id: Option<String>,
    pub icon_path: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub icon_letter: String,
}

pub fn scan_application_metadata() -> Vec<InstalledApp> {
    platform_impl::scan_application_metadata()
}

pub fn scan_application_paths() -> Vec<String> {
    platform_impl::scan_application_paths()
}

pub fn populate_application_icons(apps: &mut [InstalledApp]) {
    platform_impl::populate_application_icons(apps);
}

pub fn open_application(path: &str) -> Result<(), String> {
    platform_impl::open_application(path)
}

/// Clear icon_path for entries whose cached file is missing, zero-byte, or corrupt.
/// Call this after loading from cache to avoid handing broken paths to the UI.
pub fn clear_broken_icon_paths(apps: &mut [InstalledApp]) {
    for app in apps {
        if let Some(ref path) = app.icon_path
            && validate_cached_icon(Path::new(path)).is_none()
        {
            app.icon_path = None;
        }
    }
}

pub fn icon_cache_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("qingqi")
        .join("cache")
        .join("app_icons")
}

pub fn clear_icon_cache_dir() -> Result<usize, String> {
    clear_dir_files(&icon_cache_dir())
}

fn clear_dir_files(dir: &Path) -> Result<usize, String> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in std::fs::read_dir(dir).map_err(|e| format!("读取缓存目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取缓存条目失败: {e}"))?;
        let path = entry.path();
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| format!("删除缓存文件失败: {e}"))?;
            count += 1;
        }
    }
    Ok(count)
}

fn icon_cache_path(app_path: &Path) -> PathBuf {
    let digest = std::collections::hash_map::DefaultHasher::new();
    let mut hasher = digest;
    use std::hash::{Hash, Hasher};
    app_path.to_string_lossy().hash(&mut hasher);
    let key = format!("{:016x}", hasher.finish());

    let base = icon_cache_dir();
    let _ = std::fs::create_dir_all(&base);
    base.join(format!("{key}-v{ICON_CACHE_VERSION}.png"))
}

/// Validate a cached icon file: must exist, be non-zero, and decode as a valid image.
/// Returns the path string if valid; removes broken cache files and returns None otherwise.
fn validate_cached_icon(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }

    let meta = path.metadata().ok()?;
    if meta.len() == 0 {
        let _ = std::fs::remove_file(path);
        return None;
    }

    if ImageReader::open(path)
        .ok()
        .and_then(|reader| reader.with_guessed_format().ok())
        .and_then(|reader| reader.decode().ok())
        .is_none()
    {
        let _ = std::fs::remove_file(path);
        return None;
    }

    Some(path.to_string_lossy().to_string())
}

fn save_icon_image(image: DynamicImage, out_path: &Path) -> Result<String, String> {
    if let Some(valid) = validate_cached_icon(out_path) {
        return Ok(valid);
    }

    let trimmed = trim_transparent_padding(&image).unwrap_or(image);
    let resized = trimmed.thumbnail(ICON_SIZE_PX, ICON_SIZE_PX);
    resized.save(out_path).map_err(|error| error.to_string())?;
    Ok(out_path.to_string_lossy().to_string())
}

fn convert_icon_with_image(icon_path: &Path, out_path: &Path) -> Result<String, String> {
    let image = ImageReader::open(icon_path)
        .map_err(|error| error.to_string())?
        .with_guessed_format()
        .map_err(|error| error.to_string())?
        .decode()
        .map_err(|error| error.to_string())?;
    save_icon_image(image, out_path)
}

fn trim_transparent_padding(image: &DynamicImage) -> Option<DynamicImage> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return None;
    }

    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for (x, y, pixel) in rgba.enumerate_pixels() {
        if pixel.0[3] <= ICON_TRIM_ALPHA_THRESHOLD {
            continue;
        }
        found = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    if !found {
        return None;
    }

    let crop_width = max_x - min_x + 1;
    let crop_height = max_y - min_y + 1;
    if crop_width == width && crop_height == height {
        return None;
    }

    Some(image.crop_imm(min_x, min_y, crop_width, crop_height))
}

fn app_aliases<'a>(
    values: impl IntoIterator<Item = Option<&'a str>>,
    display_name: &str,
) -> Vec<String> {
    let raw = unique_nonempty_strings(values);

    // Expand with normalized and CamelCase-split variants.
    let mut expanded: Vec<String> = Vec::new();
    for alias in &raw {
        expanded.push(alias.clone());
        let normalized = normalize_search_text(alias);
        if !normalized.is_empty() && normalized != *alias.to_lowercase() {
            expanded.push(normalized);
        }
        let split = camel_case_split(alias);
        if !split.is_empty() && !raw.iter().any(|r| r.eq_ignore_ascii_case(&split)) {
            expanded.push(split);
        }
    }

    let mut aliases = unique_nonempty_strings(expanded.iter().map(|s| Some(s.as_str())));
    aliases.retain(|alias| !alias.eq_ignore_ascii_case(display_name));
    aliases
}

/// Split CamelCase/PascalCase into space-separated lowercase words.
/// "Visual Studio Code" stays as-is (already has spaces).
/// "VSCode" -> "vs code", "MicrosoftWord" -> "microsoft word".
fn camel_case_split(value: &str) -> String {
    if value.contains(' ') || value.contains('-') || value.contains('_') {
        return String::new();
    }
    if !value.chars().any(|ch| ch.is_uppercase()) {
        return String::new();
    }

    let mut words: Vec<String> = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let mut start = 0;

    while start < chars.len() {
        let mut end = start + 1;

        if chars[start].is_uppercase() {
            // Collect consecutive uppercase (acronym).
            while end < chars.len() && chars[end].is_uppercase() {
                end += 1;
            }
            // If multiple uppercase followed by lowercase, keep last uppercase for next word.
            // e.g. "VSCode": collected "VSC", next is 'o' (lowercase) → back up to "VS".
            if end - start > 1 && end < chars.len() && chars[end].is_lowercase() {
                end -= 1;
            }
            // If single uppercase followed by lowercase, consume the lowercase run too.
            // e.g. "Code": start='C', end already past 'C', next is 'o' → consume "ode".
            if end - start == 1 && end < chars.len() && chars[end].is_lowercase() {
                while end < chars.len() && chars[end].is_lowercase() {
                    end += 1;
                }
            }
        } else {
            // Lowercase or digit: collect until uppercase.
            while end < chars.len() && !chars[end].is_uppercase() {
                end += 1;
            }
        }

        let word: String = chars[start..end].iter().collect();
        words.push(word.to_lowercase());
        start = end;
    }

    let result = words.join(" ");
    if result == value.to_lowercase() {
        String::new()
    } else {
        result
    }
}

/// Normalize text for search: strip spaces, hyphens, underscores, dots, slashes.
fn normalize_search_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_' | '.' | '/' | '\\'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn bundle_id_suffix(bundle_id: &str) -> Option<&str> {
    bundle_id
        .split('.')
        .next_back()
        .filter(|suffix| !suffix.is_empty())
}

fn unique_nonempty_strings<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for value in values.into_iter().flatten() {
        let trimmed = value.trim();
        let key = trimmed.to_lowercase();
        if !trimmed.is_empty() && seen.insert(key) {
            out.push(trimmed.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        InstalledApp, app_aliases, bundle_id_suffix, camel_case_split, clear_broken_icon_paths,
        clear_dir_files, icon_cache_dir, normalize_search_text, trim_transparent_padding,
        validate_cached_icon,
    };
    use image::{DynamicImage, ImageBuffer, Rgba};
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
        let dir = std::env::temp_dir().join(format!("qingqi-apps-{name}-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn bundle_id_suffix_uses_last_component() {
        assert_eq!(bundle_id_suffix("com.microsoft.VSCode"), Some("VSCode"));
        assert_eq!(bundle_id_suffix("safari"), Some("safari"));
        assert_eq!(bundle_id_suffix(""), None);
    }

    #[test]
    fn aliases_merge_variants_without_duplicates() {
        let aliases = app_aliases(
            [
                Some("Visual Studio Code"),
                Some("VSCode"),
                Some("Electron"),
                Some("com.microsoft.VSCode"),
                bundle_id_suffix("com.microsoft.VSCode"),
            ],
            "Visual Studio Code",
        );

        assert!(aliases.iter().any(|alias| alias == "com.microsoft.VSCode"));
        assert!(aliases.iter().any(|alias| alias == "VSCode"));
        assert!(aliases.iter().any(|alias| alias == "Electron"));
        assert!(!aliases.iter().any(|alias| alias == "Visual Studio Code"));
    }

    #[test]
    fn aliases_include_normalized_bundle_id() {
        let aliases = app_aliases(
            [
                Some("Safari"),
                Some("com.apple.Safari"),
                bundle_id_suffix("com.apple.Safari"),
            ],
            "Safari",
        );

        assert!(aliases.iter().any(|alias| alias == "comapplesafari"));
        assert!(aliases.iter().any(|alias| alias == "com.apple.Safari"));
    }

    #[test]
    fn camel_case_split_handles_pascal_case() {
        let vs = camel_case_split("VSCode");
        assert_eq!(vs, "vs code", "VSCode split: {:?}", vs);
        let mw = camel_case_split("MicrosoftWord");
        assert_eq!(mw, "microsoft word", "MicrosoftWord split: {:?}", mw);
        let vsc = camel_case_split("VisualStudioCode");
        assert_eq!(
            vsc, "visual studio code",
            "VisualStudioCode split: {:?}",
            vsc
        );
    }

    #[test]
    fn camel_case_split_skips_already_spaced() {
        assert_eq!(camel_case_split("Visual Studio Code"), "");
        assert_eq!(camel_case_split("already-separated"), "");
    }

    #[test]
    fn normalize_search_text_strips_special_chars() {
        assert_eq!(normalize_search_text("com.apple.Safari"), "comapplesafari");
        assert_eq!(normalize_search_text("VS Code"), "vscode");
        assert_eq!(normalize_search_text("my-app_name.v2"), "myappnamev2");
    }

    #[test]
    fn metadata_scan_shape_can_start_without_icon() {
        let app = InstalledApp {
            name: String::from("Safari"),
            path: String::from("/Applications/Safari.app"),
            bundle_id: Some(String::from("com.apple.Safari")),
            icon_path: None,
            aliases: vec![String::from("Safari")],
            icon_letter: String::from("S"),
        };

        assert!(app.icon_path.is_none());
        assert_eq!(app.icon_letter, "S");
    }

    #[test]
    fn trim_transparent_padding_crops_to_visible_pixels() {
        let mut image = ImageBuffer::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        for y in 2..6 {
            for x in 1..5 {
                image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let cropped = trim_transparent_padding(&DynamicImage::ImageRgba8(image))
            .expect("visible pixels should crop");

        assert_eq!(cropped.width(), 4);
        assert_eq!(cropped.height(), 4);
    }

    #[test]
    fn validate_cached_icon_returns_none_for_missing_file() {
        let dir = temp_dir("validate-missing");
        let path = dir.join("nonexistent.png");
        assert_eq!(validate_cached_icon(&path), None);
    }

    #[test]
    fn validate_cached_icon_returns_none_for_zero_byte_file() {
        let dir = temp_dir("validate-zero");
        let path = dir.join("empty.png");
        fs::write(&path, b"").expect("write empty file");

        assert_eq!(validate_cached_icon(&path), None);
        assert!(!path.exists(), "zero-byte cache file should be removed");
    }

    #[test]
    fn validate_cached_icon_returns_none_for_corrupt_file() {
        let dir = temp_dir("validate-corrupt");
        let path = dir.join("bad.png");
        fs::write(&path, b"not-a-real-png-at-all").expect("write corrupt file");

        assert_eq!(validate_cached_icon(&path), None);
        assert!(!path.exists(), "corrupt cache file should be removed");
    }

    #[test]
    fn validate_cached_icon_returns_path_for_valid_png() {
        let dir = temp_dir("validate-valid");
        let path = dir.join("good.png");
        let img = ImageBuffer::from_pixel(4, 4, Rgba([255, 0, 0, 255]));
        DynamicImage::ImageRgba8(img)
            .save(&path)
            .expect("save valid png");

        let result = validate_cached_icon(&path);
        assert!(result.is_some(), "valid PNG should pass validation");
        assert!(path.exists(), "valid cache file should not be removed");
    }

    #[test]
    fn clear_broken_icon_paths_removes_invalid_entries() {
        let dir = temp_dir("clear-broken");
        let valid_path = dir.join("valid.png");
        let img = ImageBuffer::from_pixel(4, 4, Rgba([255, 0, 0, 255]));
        DynamicImage::ImageRgba8(img)
            .save(&valid_path)
            .expect("save valid png");

        let mut apps = vec![
            InstalledApp {
                name: String::from("Good"),
                path: String::from("/Applications/Good.app"),
                bundle_id: None,
                icon_path: Some(valid_path.to_string_lossy().to_string()),
                aliases: vec![],
                icon_letter: String::from("G"),
            },
            InstalledApp {
                name: String::from("Broken"),
                path: String::from("/Applications/Broken.app"),
                bundle_id: None,
                icon_path: Some(dir.join("missing.png").to_string_lossy().to_string()),
                aliases: vec![],
                icon_letter: String::from("B"),
            },
            InstalledApp {
                name: String::from("NoIcon"),
                path: String::from("/Applications/NoIcon.app"),
                bundle_id: None,
                icon_path: None,
                aliases: vec![],
                icon_letter: String::from("N"),
            },
        ];

        clear_broken_icon_paths(&mut apps);

        assert!(apps[0].icon_path.is_some(), "valid icon should be kept");
        assert!(
            apps[1].icon_path.is_none(),
            "broken icon path should be cleared"
        );
        assert!(
            apps[2].icon_path.is_none(),
            "none icon path should stay none"
        );
    }

    #[test]
    fn icon_cache_dir_ends_with_app_icons() {
        let dir = icon_cache_dir();
        assert!(
            dir.ends_with("cache/app_icons"),
            "should end with cache/app_icons, got {}",
            dir.display()
        );
    }

    #[test]
    fn clear_dir_files_removes_only_files() {
        let dir = temp_dir("clear-files");
        fs::write(dir.join("a.png"), b"fake").expect("write a");
        fs::write(dir.join("b.png"), b"fake").expect("write b");
        fs::create_dir_all(dir.join("subdir")).expect("create subdir");

        let count = clear_dir_files(&dir).expect("clear should succeed");
        assert_eq!(count, 2, "should remove 2 files");
        assert!(dir.join("subdir").is_dir(), "subdir should be preserved");
        assert!(!dir.join("a.png").exists(), "file a should be removed");
        assert!(!dir.join("b.png").exists(), "file b should be removed");
    }

    #[test]
    fn clear_dir_files_returns_zero_for_missing_dir() {
        let dir = std::env::temp_dir().join("qingqi-nonexistent-clear-dir");
        let count = clear_dir_files(&dir).expect("should succeed on missing dir");
        assert_eq!(count, 0);
    }

    #[test]
    fn clear_dir_files_returns_zero_for_empty_dir() {
        let dir = temp_dir("clear-empty");
        let count = clear_dir_files(&dir).expect("should succeed on empty dir");
        assert_eq!(count, 0);
    }
}
