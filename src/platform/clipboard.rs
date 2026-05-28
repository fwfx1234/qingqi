use std::path::Path;
use std::process::Command;

use gpui::{App, ClipboardEntry, ClipboardItem, Image, ImageFormat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardImage {
    pub id: u64,
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}

pub fn read_text(cx: &App) -> Option<String> {
    cx.read_from_clipboard()
        .and_then(|item| item.text())
        .map(|text| text.trim_end_matches('\0').to_string())
        .filter(|text| !text.is_empty())
}

pub fn read_image(cx: &App) -> Option<ClipboardImage> {
    cx.read_from_clipboard().and_then(|item| {
        item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image) if !image.bytes.is_empty() => Some(ClipboardImage {
                id: image.id(),
                format: image.format,
                bytes: image.bytes.clone(),
            }),
            _ => None,
        })
    })
}

pub fn write_text(cx: &mut App, text: impl Into<String>) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.into()));
}

pub fn write_image(cx: &mut App, format: ImageFormat, bytes: Vec<u8>) {
    let image = Image::from_bytes(format, bytes);
    cx.write_to_clipboard(ClipboardItem::new_image(&image));
}

pub fn image_format_extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
    }
}

pub fn image_format_label(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "PNG",
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Webp => "WEBP",
        ImageFormat::Gif => "GIF",
        ImageFormat::Svg => "SVG",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Tiff => "TIFF",
    }
}

pub fn image_format_from_path(path: &Path) -> Option<ImageFormat> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::Webp),
        "gif" => Some(ImageFormat::Gif),
        "svg" => Some(ImageFormat::Svg),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        _ => None,
    }
}

/// Attempt to read file paths from the system clipboard on macOS.
///
/// On macOS, copying files from Finder puts both file URLs on the pasteboard
/// and a text representation of the paths. We use osascript to detect the
/// file-list clipboard type and extract paths.
pub fn read_file_list() -> Option<Vec<String>> {
    #[cfg(target_os = "macos")]
    {
        read_file_list_macos()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Write file paths to the system clipboard as a file list.
///
/// On macOS, uses osascript to set the clipboard to file references,
/// which preserves the file-list pasteboard type.
pub fn write_file_list(paths: &[String]) -> anyhow::Result<()> {
    if paths.is_empty() {
        anyhow::bail!("file list is empty");
    }
    for path in paths {
        if !Path::new(path).exists() {
            anyhow::bail!("file not found: {path}");
        }
    }
    #[cfg(target_os = "macos")]
    {
        write_file_list_macos(paths)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = paths;
        anyhow::bail!("file clipboard write is not supported on this platform")
    }
}

/// Check if the clipboard text looks like file paths (macOS heuristic).
///
/// When files are copied from Finder, the text representation sometimes
/// contains the paths. This checks whether a given text string looks like
/// one or more file paths.
pub fn text_looks_like_file_paths(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    // On macOS, Finder copies paths separated by newlines or carriage returns
    let lines: Vec<&str> = trimmed
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if lines.is_empty() {
        return false;
    }
    // At least the first line should look like an absolute path
    let first = lines[0];
    if !first.starts_with('/') {
        return false;
    }
    // Prefer if the majority of lines are absolute paths
    let path_count = lines.iter().filter(|l| l.starts_with('/')).count();
    path_count * 2 >= lines.len()
}

#[cfg(target_os = "macos")]
fn read_file_list_macos() -> Option<Vec<String>> {
    // Use osascript to check for file URLs on the clipboard.
    // "clipboard info" returns a list of type descriptors; if it contains
    // "file URL", we then try to get the POSIX paths.
    let check = Command::new("osascript")
        .args([
            "-e",
            "set infoList to (clipboard info) as text",
            "-e",
            "if infoList contains \"file URL\" then return \"has-files\"",
            "-e",
            "return \"no-files\"",
        ])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&check.stdout).trim().to_string();
    if stdout != "has-files" {
        return None;
    }
    // Get the actual file paths via AppleScript
    let get_paths = Command::new("osascript")
        .args([
            "-e",
            "set pathList to {}",
            "-e",
            "try",
            "-e",
            "set rawItems to (clipboard as «class furl»)",
            "-e",
            "repeat with rawItem in rawItems",
            "-e",
            "set end of pathList to POSIX path of rawItem",
            "-e",
            "end repeat",
            "-e",
            "end try",
            "-e",
            "set AppleScript's text item delimiters to linefeed",
            "-e",
            "return pathList as text",
        ])
        .output()
        .ok()?;
    let output = String::from_utf8_lossy(&get_paths.stdout)
        .trim()
        .to_string();
    if output.is_empty() {
        return None;
    }
    let paths: Vec<String> = output
        .split('\n')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if paths.is_empty() {
        return None;
    }
    Some(paths)
}

#[cfg(target_os = "macos")]
fn write_file_list_macos(paths: &[String]) -> anyhow::Result<()> {
    // Build an osascript that sets the clipboard to the given file list.
    // Each path becomes a POSIX file reference.
    let mut script = String::new();
    script.push_str("set fileList to {}\n");
    for path in paths {
        // Escape backslashes and quotes for AppleScript string literal
        let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
        script.push_str(&format!(
            "set end of fileList to POSIX file \"{escaped}\"\n"
        ));
    }
    script.push_str("set the clipboard to fileList\n");

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run osascript: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript failed to write files to clipboard: {stderr}");
    }
    Ok(())
}
