use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, ensure};
use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder, ImageFormat, ImageReader,
    codecs::{
        jpeg::JpegEncoder,
        png::{CompressionType, FilterType, PngEncoder},
        webp::WebPEncoder,
    },
};

use crate::core::storage::AppPaths;

use super::manifest::PLUGIN_ID;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMode {
    VisuallyLossless,
    Standard,
}

impl CompressionMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::VisuallyLossless => "视觉无损",
            Self::Standard => "普通压缩",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueStatus {
    Pending,
    Running,
    Success,
    Failed,
}

impl QueueStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "待压缩",
            Self::Running => "压缩中…",
            Self::Success => "已压缩",
            Self::Failed => "失败",
        }
    }

    #[allow(dead_code)]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Success | Self::Failed)
    }
}

#[derive(Clone, Debug)]
pub struct ImportPreview {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
}

#[derive(Clone, Debug)]
pub struct ImportedImage {
    pub path: PathBuf,
    pub file_name: String,
    pub original_size: u64,
    pub preview: ImportPreview,
}

#[derive(Clone, Debug)]
pub struct CompressionResult {
    pub output_path: PathBuf,
    pub output_size: u64,
    pub reduction_ratio: f32,
}

pub struct ImageCompressService {
    default_output_dir: PathBuf,
}

impl ImageCompressService {
    pub fn new(paths: AppPaths) -> Result<Self> {
        Ok(Self {
            default_output_dir: paths.feature_output_dir(PLUGIN_ID),
        })
    }

    pub fn default_output_dir(&self) -> &Path {
        &self.default_output_dir
    }

    /// Create a lightweight clone suitable for sending to a background thread.
    pub fn clone_for_background(&self) -> Self {
        Self {
            default_output_dir: self.default_output_dir.clone(),
        }
    }

    /// Test-only constructor that accepts a raw output directory.
    #[cfg(test)]
    pub fn for_test(output_dir: PathBuf) -> Self {
        Self {
            default_output_dir: output_dir,
        }
    }

    /// Write raw clipboard image bytes to a temp file and return an ImportedImage.
    /// Sets file_name to "(剪贴板)" and the path has no real source — callers
    /// must tag the entry with `from_clipboard = true` to disable overwrite-original.
    pub fn materialize_clipboard_image(
        &self,
        bytes: &[u8],
        extension: &str,
    ) -> Result<ImportedImage> {
        fs::create_dir_all(&self.default_output_dir).context("无法创建输出目录")?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let ext = if extension.is_empty() {
            "png"
        } else {
            extension
        };
        let path = self
            .default_output_dir
            .join(format!("clipboard_{ts}.{ext}"));
        fs::write(&path, bytes)
            .with_context(|| format!("无法写入剪贴板图片 {}", path.display()))?;

        let reader = ImageReader::open(&path)
            .with_context(|| format!("无法打开图片 {}", path.display()))?
            .with_guessed_format()
            .with_context(|| format!("无法识别图片格式 {}", path.display()))?;
        let image = reader
            .decode()
            .with_context(|| format!("无法解码图片 {}", path.display()))?;
        let (width, height) = image.dimensions();

        Ok(ImportedImage {
            path: path.clone(),
            file_name: String::from("(剪贴板)"),
            original_size: bytes.len() as u64,
            preview: ImportPreview {
                width,
                height,
                has_alpha: image.has_alpha(),
            },
        })
    }

    /// Re-compress an entry at the given index. Returns Ok(()) on success.
    pub fn retry_entry(
        &self,
        output_dir: &Path,
        mode: CompressionMode,
        quality: u8,
        overwrite_original: bool,
        source_path: &Path,
    ) -> Result<CompressionResult> {
        self.compress_file(source_path, output_dir, mode, quality, overwrite_original)
    }

    /// Read the raw bytes of a completed output file.
    pub fn output_bytes(path: &Path) -> Result<Vec<u8>> {
        fs::read(path).with_context(|| format!("无法读取输出文件 {}", path.display()))
    }

    pub fn import_path(&self, path: &Path) -> Result<ImportedImage> {
        ensure!(path.exists(), "图片不存在: {}", path.display());
        ensure!(path.is_file(), "不是文件: {}", path.display());

        let reader = ImageReader::open(path)
            .with_context(|| format!("无法打开图片 {}", path.display()))?
            .with_guessed_format()
            .with_context(|| format!("无法识别图片格式 {}", path.display()))?;
        let format = reader.format();
        ensure!(
            matches!(
                format,
                Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
            ),
            "仅支持 PNG、JPEG、WebP"
        );

        let image = reader
            .decode()
            .with_context(|| format!("无法解码图片 {}", path.display()))?;
        let metadata =
            fs::metadata(path).with_context(|| format!("无法读取文件信息 {}", path.display()))?;
        let (width, height) = image.dimensions();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image")
            .to_string();

        Ok(ImportedImage {
            path: path.to_path_buf(),
            file_name,
            original_size: metadata.len(),
            preview: ImportPreview {
                width,
                height,
                has_alpha: image.has_alpha(),
            },
        })
    }

    pub fn compress_file(
        &self,
        source_path: &Path,
        output_dir: &Path,
        mode: CompressionMode,
        quality: u8,
        overwrite_original: bool,
    ) -> Result<CompressionResult> {
        ensure!(
            source_path.exists(),
            "图片不存在: {}",
            source_path.display()
        );
        let reader = ImageReader::open(source_path)
            .with_context(|| format!("无法打开图片 {}", source_path.display()))?
            .with_guessed_format()
            .with_context(|| format!("无法识别图片格式 {}", source_path.display()))?;
        let format = reader.format().ok_or_else(|| anyhow!("无法识别图片格式"))?;
        let image = reader
            .decode()
            .with_context(|| format!("无法解码图片 {}", source_path.display()))?;
        let source_meta = fs::metadata(source_path)
            .with_context(|| format!("无法读取文件信息 {}", source_path.display()))?;

        let output_path = if overwrite_original {
            source_path.to_path_buf()
        } else {
            fs::create_dir_all(output_dir)
                .with_context(|| format!("无法创建输出目录 {}", output_dir.display()))?;
            unique_output_path(source_path, output_dir, format)
        };

        write_image(&image, &output_path, format, mode, quality)?;

        let output_size = fs::metadata(&output_path)
            .with_context(|| format!("无法读取压缩结果 {}", output_path.display()))?
            .len();
        let original_size = source_meta.len().max(1);
        let reduction_ratio = 1.0 - (output_size as f32 / original_size as f32);

        Ok(CompressionResult {
            output_path,
            output_size,
            reduction_ratio: reduction_ratio.max(-9.0),
        })
    }
}

fn write_image(
    image: &DynamicImage,
    output_path: &Path,
    format: ImageFormat,
    mode: CompressionMode,
    quality: u8,
) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("无法创建输出文件 {}", output_path.display()))?;
    let writer = BufWriter::new(file);

    match format {
        ImageFormat::Jpeg => {
            let rgb = image.to_rgb8();
            let encoder_quality = match mode {
                CompressionMode::VisuallyLossless => quality.max(88),
                CompressionMode::Standard => quality,
            };
            let encoder = JpegEncoder::new_with_quality(writer, encoder_quality);
            rgb.write_with_encoder(encoder)
                .with_context(|| format!("无法写入 JPEG {}", output_path.display()))?;
        }
        ImageFormat::Png => {
            let rgba = image.to_rgba8();
            let compression = match mode {
                CompressionMode::VisuallyLossless => CompressionType::Best,
                CompressionMode::Standard => {
                    if quality >= 70 {
                        CompressionType::Best
                    } else {
                        CompressionType::Fast
                    }
                }
            };
            let encoder = PngEncoder::new_with_quality(writer, compression, FilterType::Adaptive);
            encoder
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .with_context(|| format!("无法写入 PNG {}", output_path.display()))?;
        }
        ImageFormat::WebP => {
            let rgba = image.to_rgba8();
            let encoder = WebPEncoder::new_lossless(writer);
            encoder
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .with_context(|| format!("无法写入 WebP {}", output_path.display()))?;
        }
        _ => return Err(anyhow!("仅支持 PNG、JPEG、WebP")),
    }

    Ok(())
}

fn unique_output_path(source_path: &Path, output_dir: &Path, format: ImageFormat) -> PathBuf {
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let extension = match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        _ => source_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("img"),
    };

    let initial = output_dir.join(format!("{stem}_compressed.{extension}"));
    if !initial.exists() {
        return initial;
    }

    for index in 1.. {
        let candidate = output_dir.join(format!("{stem}_compressed_{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("output candidate loop should return")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-image-compress-{name}-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn imports_png_file() {
        let dir = temp_dir("import");
        let path = dir.join("sample.png");
        let image = ImageBuffer::from_fn(16, 12, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([30u8, 90u8, 200u8, 255u8])
            } else {
                Rgba([240u8, 240u8, 250u8, 255u8])
            }
        });
        image
            .save(&path)
            .expect("png fixture should be written for import test");

        let service = ImageCompressService {
            default_output_dir: dir.join("out"),
        };
        let imported = service.import_path(&path).expect("png should import");
        assert_eq!(imported.preview.width, 16);
        assert_eq!(imported.preview.height, 12);
        assert_eq!(imported.file_name, "sample.png");
    }

    #[test]
    fn compresses_jpeg_to_output_dir() {
        let dir = temp_dir("compress");
        let source = dir.join("sample.jpg");
        let out = dir.join("output");
        let image = ImageBuffer::from_fn(60, 40, |x, y| {
            Rgba([
                ((x * 3) % 255) as u8,
                ((y * 5) % 255) as u8,
                ((x + y) % 255) as u8,
                255,
            ])
        });
        DynamicImage::ImageRgba8(image)
            .save_with_format(&source, ImageFormat::Jpeg)
            .expect("jpeg fixture should be written");

        let service = ImageCompressService {
            default_output_dir: out.clone(),
        };
        let result = service
            .compress_file(&source, &out, CompressionMode::Standard, 72, false)
            .expect("jpeg should compress");

        assert!(result.output_path.exists());
        assert!(result.output_size > 0);
    }

    #[test]
    fn queue_status_running_properties() {
        assert_eq!(QueueStatus::Running.label(), "压缩中…");
        assert!(!QueueStatus::Running.is_terminal());
        assert!(QueueStatus::Pending.is_terminal() == false);
        assert!(QueueStatus::Success.is_terminal());
        assert!(QueueStatus::Failed.is_terminal());
    }

    #[test]
    fn clone_for_background_preserves_output_dir() {
        let dir = temp_dir("clone-bg");
        let service = ImageCompressService {
            default_output_dir: dir.join("output"),
        };
        let cloned = service.clone_for_background();
        assert_eq!(service.default_output_dir(), cloned.default_output_dir());
    }

    #[test]
    fn batch_compress_sequential() {
        // Verify that compress_file works when called sequentially (simulating batch).
        let dir = temp_dir("batch");
        let out = dir.join("output");

        let sources: Vec<PathBuf> = (0..3)
            .map(|i| {
                let path = dir.join(format!("img_{i}.png"));
                let image = ImageBuffer::from_fn(20, 16, |x, y| {
                    Rgba([
                        ((x + i * 10) % 255) as u8,
                        ((y + i * 20) % 255) as u8,
                        128,
                        255,
                    ])
                });
                image.save(&path).expect("fixture should write");
                path
            })
            .collect();

        let service = ImageCompressService {
            default_output_dir: out.clone(),
        };

        let mut results = Vec::new();
        for source in &sources {
            let result = service.compress_file(source, &out, CompressionMode::Standard, 75, false);
            results.push(result);
        }

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(success_count, 3);
        for result in &results {
            assert!(result.as_ref().unwrap().output_path.exists());
        }
    }
}
