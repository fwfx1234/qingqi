use serde::{Deserialize, Serialize};

use qingqi_plugin::job::JobStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "等待中",
            Self::Downloading => "下载中",
            Self::Paused => "已暂停",
            Self::Completed => "已完成",
            Self::Failed => "失败",
            Self::Cancelled => "已取消",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Downloading)
    }
}

impl From<TaskStatus> for JobStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Pending => Self::Pending,
            TaskStatus::Downloading => Self::Running,
            TaskStatus::Paused => Self::Paused,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Failed => Self::Failed,
            TaskStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileCategory {
    Video,
    Audio,
    Document,
    Archive,
    Image,
    Software,
    Other,
}

impl FileCategory {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" => Self::Video,
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" | "m4a" | "opus" => Self::Audio,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "csv"
            | "epub" => Self::Document,
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "iso" => Self::Archive,
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" => Self::Image,
            "exe" | "dmg" | "pkg" | "deb" | "rpm" | "msi" | "appimage" | "apk" => Self::Software,
            _ => Self::Other,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Video => "视频",
            Self::Audio => "音频",
            Self::Document => "文档",
            Self::Archive => "压缩包",
            Self::Image => "图片",
            Self::Software => "软件",
            Self::Other => "其他",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Video => "\u{1f3ac}",
            Self::Audio => "\u{1f3b5}",
            Self::Document => "\u{1f4c4}",
            Self::Archive => "\u{1f4e6}",
            Self::Image => "\u{1f5bc}",
            Self::Software => "\u{1f4bb}",
            Self::Other => "\u{1f4be}",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub file_name: String,
    pub save_path: String,
    pub file_size: Option<u64>,
    pub downloaded: u64,
    pub status: TaskStatus,
    pub category: FileCategory,
    pub error_msg: String,
    pub speed_bps: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl DownloadTask {
    pub fn progress_percent(&self) -> f64 {
        match self.file_size {
            Some(size) if size > 0 => (self.downloaded as f64 / size as f64 * 100.0).min(100.0),
            _ => 0.0,
        }
    }

    pub fn eta_seconds(&self) -> Option<u64> {
        if self.speed_bps <= 0.0 {
            return None;
        }
        let remaining = self.file_size?.checked_sub(self.downloaded)?;
        if remaining == 0 {
            return Some(0);
        }
        Some((remaining as f64 / self.speed_bps) as u64)
    }
}

pub fn sanitize_file_name(raw: &str) -> String {
    // 1. Take the last component when both '/' and '\' are treated as separators.
    let component = raw
        .trim()
        .split(|c: char| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("")
        .to_string();

    // 2. "." , ".." or empty -> "download".
    let mut name = if component.is_empty() || component == "." || component == ".." {
        "download".to_string()
    } else {
        component
    };

    // 3. Strip control characters.
    name.retain(|c| !c.is_control());

    // 4. Replace Windows-reserved characters.
    name = name.replace(
        |c: char| matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'),
        "_",
    );

    // 5. Strip trailing spaces and dots.
    while name.ends_with(' ') || name.ends_with('.') {
        name.pop();
    }

    // 6. Prefix case-insensitive Windows device names.
    let stem_lower = name.split('.').next().unwrap_or("").to_ascii_lowercase();
    let is_device = matches!(
        stem_lower.as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    );
    if is_device {
        name.insert(0, '_');
    }

    // 7. Final fallback.
    if name.is_empty() {
        name = "download".to_string();
    }

    name
}

pub fn extract_file_name(url: &str, content_disposition: Option<&str>) -> String {
    if let Some(cd) = content_disposition {
        if let Some(name) = parse_content_disposition(cd) {
            return sanitize_file_name(&name);
        }
    }

    let decoded = url
        .split('?')
        .next()
        .unwrap_or(url)
        .split('/')
        .last()
        .filter(|s| !s.is_empty())
        .map(decode_percent_lossy)
        .unwrap_or_else(|| "download".to_string());

    sanitize_file_name(&decoded)
}

fn parse_content_disposition(cd: &str) -> Option<String> {
    // Try RFC 5987 filename*=UTF-8''...
    if let Some(idx) = cd.find("filename*=") {
        let val = &cd[idx + 10..];
        let val = val.split(';').next().unwrap_or(val).trim();
        if let Some(pos) = val.rfind("''") {
            let encoded = &val[pos + 2..];
            if let Ok(decoded) = urlencoding::decode(encoded) {
                if !decoded.is_empty() {
                    return Some(decoded.into_owned());
                }
            }
        }
    }

    // Try filename="..."
    if let Some(idx) = cd.find("filename=") {
        let val = &cd[idx + 9..];
        let val = val.split(';').next().unwrap_or(val);
        let val = val.trim().trim_matches('"').trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }

    None
}

pub fn guess_file_name(url: &str) -> String {
    let path_part = url.split('?').next().unwrap_or(url);
    let name = path_part
        .split('/')
        .last()
        .filter(|s| !s.is_empty() && s.contains('.'))
        .unwrap_or("download");
    let decoded = decode_percent_lossy(name);
    sanitize_file_name(&decoded)
}

pub fn file_extension(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or("")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadSettings {
    pub save_root: String,
    pub max_concurrent: usize,
    pub speed_limit_kbps: u32,
    pub timeout_secs: u32,
    pub retry_limit: u32,
    pub proxy_url: String,
    pub user_agent: String,
    pub referer: String,
    pub cookie: String,
    pub custom_headers: String,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        let home = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Downloads")
            .join("Qingqi");
        Self {
            save_root: home.to_string_lossy().to_string(),
            max_concurrent: 3,
            speed_limit_kbps: 0,
            timeout_secs: 30,
            retry_limit: 2,
            proxy_url: String::new(),
            user_agent: String::new(),
            referer: String::new(),
            cookie: String::new(),
            custom_headers: String::new(),
        }
    }
}

pub fn extract_urls_from_text(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let re = regex::Regex::new(r#"https?://[^\s'"<>]+"#).unwrap();
    for m in re.find_iter(text) {
        let candidate = m
            .as_str()
            .trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ')'));
        if !candidate.is_empty() && seen.insert(candidate.to_string()) {
            urls.push(candidate.to_string());
        }
    }
    urls
}

pub fn parse_custom_headers(raw: &str) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains(':') {
            continue;
        }
        let (name, val) = trimmed.split_once(':').unwrap();
        let key = name.trim().to_string();
        let value = val.trim().to_string();
        if key.is_empty() {
            continue;
        }
        headers.push((key, value));
    }
    headers
}

fn decode_percent_lossy(value: &str) -> String {
    let bytes = urlencoding::decode_binary(value.as_bytes());
    String::from_utf8_lossy(bytes.as_ref()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_category_from_extension() {
        assert_eq!(FileCategory::from_extension("mp4"), FileCategory::Video);
        assert_eq!(FileCategory::from_extension("MP3"), FileCategory::Audio);
        assert_eq!(FileCategory::from_extension("pdf"), FileCategory::Document);
        assert_eq!(FileCategory::from_extension("zip"), FileCategory::Archive);
        assert_eq!(FileCategory::from_extension("jpg"), FileCategory::Image);
        assert_eq!(FileCategory::from_extension("exe"), FileCategory::Software);
        assert_eq!(FileCategory::from_extension("xyz"), FileCategory::Other);
    }

    #[test]
    fn extract_file_name_from_url() {
        assert_eq!(
            extract_file_name("https://example.com/file.zip", None),
            "file.zip"
        );
        assert_eq!(
            extract_file_name("https://example.com/file.zip?token=abc", None),
            "file.zip"
        );
    }

    #[test]
    fn extract_file_name_from_content_disposition() {
        assert_eq!(
            extract_file_name(
                "https://example.com/dl",
                Some(r#"attachment; filename="report.pdf""#)
            ),
            "report.pdf"
        );
        assert_eq!(
            extract_file_name(
                "https://example.com/dl",
                Some("attachment; filename*=UTF-8''%E4%B8%AD%E6%96%87%20%E6%8A%A5%E5%91%8A.pdf")
            ),
            "中文 报告.pdf"
        );
        assert_eq!(
            extract_file_name(
                "https://example.com/dl",
                Some(r#"attachment; filename="report.pdf"; size=123"#)
            ),
            "report.pdf"
        );
    }

    #[test]
    fn task_progress_calculation() {
        let task = DownloadTask {
            id: "1".into(),
            url: String::new(),
            file_name: "test.zip".into(),
            save_path: String::new(),
            file_size: Some(1000),
            downloaded: 500,
            status: TaskStatus::Downloading,
            category: FileCategory::Archive,
            error_msg: String::new(),
            speed_bps: 100.0,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!((task.progress_percent() - 50.0).abs() < 0.01);
        assert_eq!(task.eta_seconds(), Some(5));
    }

    #[test]
    fn extract_urls_single() {
        let urls = extract_urls_from_text("https://example.com/file.zip");
        assert_eq!(urls, vec!["https://example.com/file.zip"]);
    }

    #[test]
    fn extract_urls_multi_line() {
        let text = "https://a.com/1.zip\nhttps://b.com/2.zip";
        let urls = extract_urls_from_text(text);
        assert_eq!(urls, vec!["https://a.com/1.zip", "https://b.com/2.zip"]);
    }

    #[test]
    fn extract_urls_dedupe() {
        let text = "https://a.com/1.zip\nhttps://a.com/1.zip";
        let urls = extract_urls_from_text(text);
        assert_eq!(urls, vec!["https://a.com/1.zip"]);
    }

    #[test]
    fn extract_urls_from_mixed_text() {
        let text = "下载链接: https://a.com/1.zip 和 https://b.com/2.zip 测试";
        let urls = extract_urls_from_text(text);
        assert_eq!(urls, vec!["https://a.com/1.zip", "https://b.com/2.zip"]);
    }

    #[test]
    fn parse_custom_headers_parses_lines() {
        let headers = parse_custom_headers("X-Token: abc123\nAuthorization: Bearer xyz");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], ("X-Token".to_string(), "abc123".to_string()));
        assert_eq!(
            headers[1],
            ("Authorization".to_string(), "Bearer xyz".to_string())
        );
    }

    #[test]
    fn parse_custom_headers_skips_invalid() {
        let headers = parse_custom_headers("no-colon\nX-Key: val");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], ("X-Key".to_string(), "val".to_string()));
    }

    #[test]
    fn sanitize_file_name_table_driven() {
        let dir = std::path::PathBuf::from("/tmp/sandbox-dir");
        let cases = [
            "../secret.txt",
            "%2e%2e%2fsecret.txt",
            "..%5csecret.txt",
            "/etc/passwd",
            "C:\\Windows\\win.ini",
            "\\\\server\\share\\a.txt",
            "CON",
            "nul.txt",
            "name\0.txt",
            "normal file.zip",
            "中文图片.png",
        ];

        for raw in cases {
            // Inputs that are still percent-encoded get decoded first by the caller path,
            // but sanitize_file_name only sees the already-decoded string. Simulate the
            // decode step here to match real usage.
            let decoded = decode_percent_lossy(raw);
            let result = sanitize_file_name(&decoded);

            assert!(!result.is_empty(), "empty result for input: {raw}");
            assert_ne!(result, ".", "result is '.' for input: {raw}");
            assert_ne!(result, "..", "result is '..' for input: {raw}");
            assert_eq!(
                std::path::Path::new(&result).components().count(),
                1,
                "multi-component result '{}' for input: {raw}",
                result
            );
            assert_eq!(
                dir.join(&result).parent(),
                Some(dir.as_ref()),
                "directory escape for input: {raw}, result: {result}"
            );
        }
    }

    #[test]
    fn download_settings_defaults() {
        let settings = DownloadSettings::default();
        assert_eq!(settings.max_concurrent, 3);
        assert_eq!(settings.speed_limit_kbps, 0);
        assert_eq!(settings.timeout_secs, 30);
        assert_eq!(settings.retry_limit, 2);
    }
}
