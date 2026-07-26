use serde::{Deserialize, Serialize};

/// Detect if a captured exchange is likely a video/audio stream
pub fn is_video_stream(url: &str, content_type: &str, response_size: i64) -> bool {
    let url_lower = url.to_lowercase();

    // Check URL patterns
    let video_extensions = [".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".ts", ".m3u8", ".mpd"];
    let is_video_url = video_extensions.iter().any(|ext| url_lower.ends_with(ext));

    // Check Content-Type
    let is_video_ct = content_type.starts_with("video/")
        || content_type.starts_with("audio/")
        || content_type.contains("application/x-mpegurl")
        || content_type.contains("application/dash+xml");

    // Large response with video content-type
    let is_large_media = response_size > 1024 * 1024
        && (content_type.contains("octet-stream") || content_type.contains("video") || content_type.contains("audio"));

    is_video_url || is_video_ct || is_large_media
}

/// Get a friendly name for the media type
pub fn media_type_label(content_type: &str) -> &str {
    if content_type.starts_with("video/") { "视频" }
    else if content_type.starts_with("audio/") { "音频" }
    else if content_type.contains("mpegurl") { "HLS" }
    else if content_type.contains("dash") { "DASH" }
    else { "媒体" }
}

/// Extract filename from URL
pub fn extract_media_filename(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let name = path.split('/').last().unwrap_or("media");
    if name.is_empty() { "media".to_string() } else { name.to_string() }
}

/// Represents a detected downloadable media
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectedMedia {
    pub url: String,
    pub file_name: String,
    pub media_type: String,
    pub content_type: String,
    pub size: i64,
    pub referer: String,
}

impl DetectedMedia {
    pub fn from_exchange(url: &str, content_type: &str, response_size: i64, referer: &str) -> Self {
        Self {
            url: url.to_string(),
            file_name: extract_media_filename(url),
            media_type: media_type_label(content_type).to_string(),
            content_type: content_type.to_string(),
            size: response_size,
            referer: referer.to_string(),
        }
    }
}
