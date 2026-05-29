use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use image::{ImageBuffer, ImageFormat, ImageReader, Luma};
use qrcode::{Color, EcLevel, QrCode};
use quircs::Quirc;
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};

use crate::{
    core::storage::AppPaths,
    features::qr_code::{
        manifest::PLUGIN_ID,
        store::{QrHistoryKind, QrHistoryRecord, QrHistoryStore},
    },
};

const QR_BORDER_MODULES: u32 = 4;
const QR_SCALE: u32 = 12;
static FILE_STAMP_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]_[hour][minute][second]");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrMatrix {
    pub size: usize,
    pub cells: Vec<bool>,
}

#[derive(Clone)]
pub struct QrCodeService {
    history_store: QrHistoryStore,
}

impl QrCodeService {
    pub fn new(paths: AppPaths) -> Result<Self> {
        let history_path = paths.feature_state(PLUGIN_ID, "history.json");
        Self::from_history_path(history_path)
    }

    pub fn from_history_path(history_path: PathBuf) -> Result<Self> {
        Ok(Self {
            history_store: QrHistoryStore::open(history_path)?,
        })
    }

    pub fn preview(&self, text: &str) -> Result<QrMatrix> {
        generate(text)
    }

    pub fn save_to_dir(&self, text: &str, target_dir: &Path) -> Result<PathBuf> {
        let matrix = self.preview(text)?;
        fs::create_dir_all(target_dir)
            .with_context(|| format!("无法创建输出目录 {}", target_dir.display()))?;

        let digest = short_hash(text);
        let timestamp = OffsetDateTime::now_local()
            .unwrap_or_else(|_| OffsetDateTime::now_utc())
            .format(FILE_STAMP_FORMAT)
            .unwrap_or_else(|_| String::from("qr"));
        let base = target_dir.join(format!("qr_{timestamp}_{digest}.png"));
        let target = unique_path(&base);

        save_png(&matrix, &target)?;
        self.history_store.push(
            QrHistoryKind::Save,
            text.trim(),
            target.to_string_lossy().to_string(),
        )?;
        Ok(target)
    }

    pub fn record_copy(&self, text: &str) -> Result<QrHistoryRecord> {
        let normalized = normalize_content(text)?;
        self.history_store
            .push(QrHistoryKind::Copy, &normalized, "")
    }

    #[allow(clippy::never_loop)]
    pub fn scan_image(&self, path: &Path) -> Result<String> {
        ensure!(path.exists(), "图片不存在: {}", path.display());

        let image = ImageReader::open(path)
            .with_context(|| format!("无法打开图片 {}", path.display()))?
            .decode()
            .with_context(|| format!("无法解码图片 {}", path.display()))?;
        let luma = image.to_luma8();

        let mut decoder = Quirc::default();
        let codes = decoder.identify(luma.width() as usize, luma.height() as usize, &luma);
        for code in codes {
            let code = code.context("二维码定位失败")?;
            let decoded = code.decode().context("二维码解码失败")?;
            let text = String::from_utf8(decoded.payload)
                .context("二维码内容不是有效 UTF-8 文本")?
                .trim()
                .to_string();
            ensure!(!text.is_empty(), "二维码内容为空");
            self.history_store.push(
                QrHistoryKind::Scan,
                &text,
                path.to_string_lossy().to_string(),
            )?;
            return Ok(text);
        }

        Err(anyhow::anyhow!("未识别到二维码"))
    }

    pub fn scan_image_input(&self, raw_path: &str) -> Result<(String, PathBuf)> {
        let path = normalize_local_path(raw_path);
        ensure!(!path.as_os_str().is_empty(), "请先选择二维码图片");
        let text = self.scan_image(&path)?;
        Ok((text, path))
    }

    pub fn list_history(&self, query: &str) -> Result<Vec<QrHistoryRecord>> {
        self.history_store.list(query)
    }

    pub fn clear_history(&self) -> Result<()> {
        self.history_store.clear()
    }

    pub fn remove_history(&self, id: &str) -> Result<bool> {
        self.history_store.remove(id)
    }

    pub fn export_history_to_dir(&self, target_dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(target_dir)
            .with_context(|| format!("无法创建输出目录 {}", target_dir.display()))?;
        let timestamp = OffsetDateTime::now_local()
            .unwrap_or_else(|_| OffsetDateTime::now_utc())
            .format(FILE_STAMP_FORMAT)
            .unwrap_or_else(|_| String::from("history"));
        let target = unique_path(&target_dir.join(format!("qr_history_{timestamp}.txt")));
        self.history_store.export(&target)
    }
}

pub fn generate(text: &str) -> Result<QrMatrix> {
    let normalized = normalize_content(text)?;
    let qr = QrCode::with_error_correction_level(normalized.as_bytes(), EcLevel::M)
        .map_err(|error| anyhow::anyhow!("二维码生成失败: {error:?}"))?;
    let size = qr.width();
    let cells = qr
        .to_colors()
        .into_iter()
        .map(|color| color == Color::Dark)
        .collect();

    Ok(QrMatrix { size, cells })
}

pub fn save_png(matrix: &QrMatrix, target: &Path) -> Result<()> {
    ensure!(matrix.size > 0, "二维码矩阵为空");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("无法创建二维码输出目录 {}", parent.display()))?;
    }

    let modules = matrix.size as u32;
    let image_size = (modules + QR_BORDER_MODULES * 2) * QR_SCALE;
    let mut image = ImageBuffer::from_pixel(image_size, image_size, Luma([255u8]));

    for row in 0..matrix.size {
        for col in 0..matrix.size {
            let index = row * matrix.size + col;
            if !matrix.cells.get(index).copied().unwrap_or(false) {
                continue;
            }

            let base_x = (col as u32 + QR_BORDER_MODULES) * QR_SCALE;
            let base_y = (row as u32 + QR_BORDER_MODULES) * QR_SCALE;
            for dy in 0..QR_SCALE {
                for dx in 0..QR_SCALE {
                    image.put_pixel(base_x + dx, base_y + dy, Luma([0u8]));
                }
            }
        }
    }

    image
        .save_with_format(target, ImageFormat::Png)
        .with_context(|| format!("无法保存二维码图片 {}", target.display()))
}

fn normalize_content(text: &str) -> Result<String> {
    let normalized = text.trim();
    ensure!(!normalized.is_empty(), "请输入要生成二维码的文本");
    Ok(normalized.to_string())
}

pub fn normalize_local_path(raw: &str) -> PathBuf {
    let mut cleaned = raw.trim().trim_matches(&['"', '\''][..]).to_string();
    if cleaned.is_empty() {
        return PathBuf::new();
    }

    if let Some(rest) = cleaned.strip_prefix("file://") {
        let path_part = if let Some(local) = rest.strip_prefix("localhost/") {
            format!("/{local}")
        } else if rest.starts_with('/') {
            rest.to_string()
        } else {
            format!("/{rest}")
        };
        cleaned = percent_decode(&path_part);
    }

    if looks_like_windows_drive_with_leading_slash(&cleaned) {
        cleaned.remove(0);
    }

    if let Some(rest) = cleaned.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }

    PathBuf::from(cleaned)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push((high << 4) | low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn looks_like_windows_drive_with_leading_slash(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':'
}

fn short_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}

fn unique_path(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }

    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let stem = target
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("qr");
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");

    for counter in 1.. {
        let candidate = parent.join(format!("{stem}_{counter}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("counter loop should always return")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!("qingqi-qr-{name}-{millis}"));
        let _ = fs::create_dir_all(&path);
        path
    }

    #[test]
    fn rejects_empty_text() {
        let result = generate("   ");
        assert!(result.is_err());
    }

    #[test]
    fn generates_matrix() {
        let matrix = generate("https://openai.com").expect("qr generation should succeed");
        assert!(matrix.size > 0);
        assert_eq!(matrix.cells.len(), matrix.size * matrix.size);
        assert!(matrix.cells.iter().any(|cell| *cell));
    }

    #[test]
    fn saves_png_and_history() {
        let root = temp_dir("save");
        let history_path = root.join("history.json");
        let service = QrCodeService::from_history_path(history_path).expect("service should build");

        let saved = service
            .save_to_dir("https://openai.com", &root)
            .expect("save should succeed");
        let metadata = fs::metadata(&saved).expect("png should exist");
        assert!(metadata.len() > 0);

        let history = service.list_history("").expect("history should load");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].kind, QrHistoryKind::Save);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn records_copy_and_exports_history() {
        let root = temp_dir("export");
        let history_path = root.join("history.json");
        let service = QrCodeService::from_history_path(history_path).expect("service should build");

        service
            .record_copy("hello")
            .expect("copy record should succeed");
        let exported = service
            .export_history_to_dir(&root)
            .expect("history export should succeed");
        let raw = fs::read_to_string(exported).expect("history export should be readable");
        assert!(raw.contains("hello"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scans_generated_png_and_records_history() {
        let root = temp_dir("scan");
        let history_path = root.join("history.json");
        let service = QrCodeService::from_history_path(history_path).expect("service should build");
        let png_path = root.join("scan-target.png");
        let content = "https://openai.com/research";

        let matrix = generate(content).expect("qr matrix should build");
        save_png(&matrix, &png_path).expect("png should save");

        let scanned = service
            .scan_image(&png_path)
            .expect("scan should decode saved png");
        assert_eq!(scanned, content);

        let history = service.list_history("").expect("history should load");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].kind, QrHistoryKind::Scan);
        assert_eq!(history[0].content, content);

        let _ = fs::remove_dir_all(root);
    }
}
