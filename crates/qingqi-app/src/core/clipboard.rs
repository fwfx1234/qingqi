use std::fs;

use gpui::App;
use qingqi_plugin::command::ClipboardPayload;

pub fn current_payload(cx: &App) -> Option<ClipboardPayload> {
    let snapshot = qingqi_platform::clipboard::read_snapshot(cx, None);
    let payload = ClipboardPayload {
        text: snapshot.text,
        image_path: snapshot.image.and_then(materialize_image),
        file_paths: snapshot.files,
    };
    (!payload.is_empty()).then_some(payload)
}

fn materialize_image(image: qingqi_platform::clipboard::ClipboardImage) -> Option<String> {
    let dir = std::env::temp_dir().join("qingqi-clipboard");
    if let Err(error) = fs::create_dir_all(&dir) {
        tracing::warn!(error = %error, dir = %dir.display(), "cannot create clipboard temp dir");
        return None;
    }

    let ext = qingqi_platform::clipboard::image_format_extension(image.format);
    let path = dir.join(format!("clipboard-{}.{}", image.id, ext));
    if !path.exists()
        && let Err(error) = fs::write(&path, image.bytes)
    {
        tracing::warn!(error = %error, path = %path.display(), "cannot write clipboard image");
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}
