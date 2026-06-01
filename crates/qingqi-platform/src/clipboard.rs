use std::path::Path;

#[cfg(not(target_os = "windows"))]
use gpui::ClipboardEntry;
use gpui::{App, ClipboardItem, Image, ImageFormat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardImage {
    pub id: u64,
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardSnapshot {
    pub files: Option<Vec<String>>,
    pub text: Option<String>,
    pub image: Option<ClipboardImage>,
}

impl ClipboardSnapshot {
    pub fn is_empty(&self) -> bool {
        self.files.as_ref().is_none_or(Vec::is_empty)
            && self.text.as_ref().is_none_or(String::is_empty)
            && self.image.is_none()
    }
}

pub fn read_snapshot(cx: &App, skip_image_id: Option<u64>) -> ClipboardSnapshot {
    #[cfg(target_os = "windows")]
    {
        let _ = cx;
        read_snapshot_windows(skip_image_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let Some(item) = cx.read_from_clipboard() else {
            return ClipboardSnapshot::default();
        };

        let text = item
            .text()
            .map(|text| text.trim_end_matches('\0').to_string())
            .filter(|text| !text.is_empty());

        let image = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image)
                if !image.bytes.is_empty() && Some(image.id()) != skip_image_id =>
            {
                Some(ClipboardImage {
                    id: image.id(),
                    format: image.format,
                    bytes: image.bytes.clone(),
                })
            }
            _ => None,
        });

        ClipboardSnapshot {
            files: read_file_list(),
            text,
            image,
        }
    }
}

/// Returns the system clipboard's monotonically increasing change counter,
/// used to cheaply skip polling work when nothing has changed.
///
/// Returns `None` when the value cannot be obtained or on non-macOS platforms,
/// in which case callers should fall back to reading the clipboard directly.
pub fn change_count() -> Option<i64> {
    #[cfg(target_os = "windows")]
    {
        return change_count_windows();
    }
    #[cfg(target_os = "macos")]
    {
        change_count_macos()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Reads `[[NSPasteboard generalPasteboard] changeCount]` via the Objective-C
/// runtime. AppKit and libobjc are already linked by gpui, so we only declare
/// the runtime entry points we need rather than pulling in a new dependency.
#[cfg(target_os = "macos")]
fn change_count_macos() -> Option<i64> {
    use std::ffi::c_void;
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    type SendToObject = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type SendToInteger = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i64;

    unsafe {
        let class = objc_getClass(c"NSPasteboard".as_ptr());
        if class.is_null() {
            return None;
        }
        let msg_send = objc_msgSend as unsafe extern "C" fn();
        let send_object: SendToObject = std::mem::transmute(msg_send);
        let pasteboard = send_object(class, sel_registerName(c"generalPasteboard".as_ptr()));
        if pasteboard.is_null() {
            return None;
        }
        let send_integer: SendToInteger = std::mem::transmute(msg_send);
        Some(send_integer(
            pasteboard,
            sel_registerName(c"changeCount".as_ptr()),
        ))
    }
}

pub fn read_text(cx: &App) -> Option<String> {
    read_snapshot(cx, None).text
}

pub fn read_image(cx: &App) -> Option<ClipboardImage> {
    read_snapshot(cx, None).image
}

pub fn read_background_snapshot(
    change_count: i64,
    skip_image_id: Option<u64>,
) -> ClipboardSnapshot {
    #[cfg(target_os = "windows")]
    {
        return read_background_snapshot_windows(change_count, skip_image_id);
    }
    #[cfg(target_os = "macos")]
    {
        read_background_snapshot_macos(change_count, skip_image_id)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (change_count, skip_image_id);
        ClipboardSnapshot::default()
    }
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
    #[cfg(target_os = "windows")]
    {
        read_file_list_windows()
    }
    #[cfg(target_os = "macos")]
    {
        read_file_list_macos()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
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

#[cfg(target_os = "windows")]
fn change_count_windows() -> Option<i64> {
    let value = unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() };
    (value != 0).then_some(value as i64)
}

#[cfg(target_os = "windows")]
fn read_background_snapshot_windows(
    change_count: i64,
    skip_image_id: Option<u64>,
) -> ClipboardSnapshot {
    let _ = change_count;
    read_snapshot_windows(skip_image_id)
}

#[cfg(target_os = "windows")]
fn read_snapshot_windows(skip_image_id: Option<u64>) -> ClipboardSnapshot {
    with_windows_clipboard(|| ClipboardSnapshot {
        files: read_file_list_windows_open(),
        text: read_text_windows_open(),
        image: read_image_windows_open(skip_image_id),
    })
    .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn read_file_list_windows() -> Option<Vec<String>> {
    with_windows_clipboard(read_file_list_windows_open).flatten()
}

#[cfg(target_os = "windows")]
fn with_windows_clipboard<T>(f: impl FnOnce() -> T) -> Option<T> {
    use windows::Win32::System::DataExchange::{CloseClipboard, OpenClipboard};

    if let Err(error) = unsafe { OpenClipboard(None) } {
        tracing::debug!(error = %error, "failed to open Windows clipboard");
        return None;
    }

    let result = f();
    if let Err(error) = unsafe { CloseClipboard() } {
        tracing::warn!(error = %error, "failed to close Windows clipboard");
    }
    Some(result)
}

#[cfg(target_os = "windows")]
fn read_text_windows_open() -> Option<String> {
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    with_windows_clipboard_data(CF_UNICODETEXT.0 as u32, |ptr, size| {
        let units = size / std::mem::size_of::<u16>();
        let text = unsafe {
            let wide = std::slice::from_raw_parts(ptr.cast::<u16>(), units);
            let len = wide.iter().position(|ch| *ch == 0).unwrap_or(wide.len());
            String::from_utf16_lossy(&wide[..len])
        };
        text.trim_end_matches('\0').to_string()
    })
    .filter(|text| !text.is_empty())
}

#[cfg(target_os = "windows")]
fn read_file_list_windows_open() -> Option<Vec<String>> {
    use windows::Win32::{
        System::{
            DataExchange::{GetClipboardData, IsClipboardFormatAvailable},
            Ole::CF_HDROP,
        },
        UI::Shell::{DragQueryFileW, HDROP},
    };

    if unsafe { IsClipboardFormatAvailable(CF_HDROP.0 as u32) }.is_err() {
        return None;
    }
    let handle = unsafe { GetClipboardData(CF_HDROP.0 as u32) }.ok()?;
    if handle.is_invalid() {
        return None;
    }

    let hdrop = HDROP(handle.0);
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    let mut files = Vec::with_capacity(count as usize);
    for index in 0..count {
        let len = unsafe { DragQueryFileW(hdrop, index, None) } as usize;
        if len == 0 {
            continue;
        }
        let mut buffer = vec![0u16; len + 1];
        let copied = unsafe { DragQueryFileW(hdrop, index, Some(buffer.as_mut_slice())) };
        if copied == 0 {
            continue;
        }
        files.push(String::from_utf16_lossy(&buffer[..len]));
    }
    (!files.is_empty()).then_some(files)
}

#[cfg(target_os = "windows")]
fn read_image_windows_open(skip_image_id: Option<u64>) -> Option<ClipboardImage> {
    const CF_BITMAP: u32 = 2;
    const CF_DIB: u32 = 8;
    const CF_DIBV5: u32 = 17;

    if let Some(image) = read_dib_windows_open(CF_DIBV5, skip_image_id) {
        return Some(image);
    }
    if let Some(image) = read_dib_windows_open(CF_DIB, skip_image_id) {
        return Some(image);
    }
    read_hbitmap_windows_open(CF_BITMAP, skip_image_id)
}

#[cfg(target_os = "windows")]
fn read_dib_windows_open(format: u32, skip_image_id: Option<u64>) -> Option<ClipboardImage> {
    with_windows_clipboard_data(format, |ptr, size| {
        if size == 0 {
            return None;
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), size).to_vec() };
        let id = clipboard_bytes_id(&bytes);
        if skip_image_id == Some(id) {
            return None;
        }
        dib_bytes_to_png(bytes).map(|png| ClipboardImage {
            id,
            format: ImageFormat::Png,
            bytes: png,
        })
    })?
}

#[cfg(target_os = "windows")]
fn read_hbitmap_windows_open(format: u32, skip_image_id: Option<u64>) -> Option<ClipboardImage> {
    use windows::Win32::{
        Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS,
            DeleteDC, GetDIBits, GetObjectW, HBITMAP, HGDIOBJ,
        },
        System::DataExchange::{GetClipboardData, IsClipboardFormatAvailable},
    };

    if unsafe { IsClipboardFormatAvailable(format) }.is_err() {
        return None;
    }
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    if handle.is_invalid() {
        return None;
    }
    let bitmap = HBITMAP(handle.0);
    let mut bitmap_info = BITMAP::default();
    let got = unsafe {
        GetObjectW(
            HGDIOBJ::from(bitmap),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut bitmap_info as *mut BITMAP).cast()),
        )
    };
    if got == 0 || bitmap_info.bmWidth <= 0 || bitmap_info.bmHeight <= 0 {
        return None;
    }

    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return None;
    }

    let width = bitmap_info.bmWidth;
    let height = bitmap_info.bmHeight;
    let mut dib = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut dib,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        let _ = DeleteDC(dc);
    }
    if lines == 0 {
        return None;
    }

    bgra_pixels_to_png(width as u32, height as u32, pixels, true).and_then(|png| {
        let id = clipboard_bytes_id(&png);
        (skip_image_id != Some(id)).then_some(ClipboardImage {
            id,
            format: ImageFormat::Png,
            bytes: png,
        })
    })
}

#[cfg(target_os = "windows")]
fn with_windows_clipboard_data<T>(
    format: u32,
    f: impl FnOnce(*mut std::ffi::c_void, usize) -> T,
) -> Option<T> {
    use windows::Win32::{
        Foundation::HGLOBAL,
        System::{
            DataExchange::{GetClipboardData, IsClipboardFormatAvailable},
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        },
    };

    if unsafe { IsClipboardFormatAvailable(format) }.is_err() {
        return None;
    }
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    if handle.is_invalid() {
        return None;
    }
    let global = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(global) };
    if size == 0 {
        return None;
    }
    let ptr = unsafe { GlobalLock(global) };
    if ptr.is_null() {
        return None;
    }
    let result = f(ptr, size);
    let _ = unsafe { GlobalUnlock(global) };
    Some(result)
}

#[cfg(target_os = "windows")]
fn dib_bytes_to_png(bytes: Vec<u8>) -> Option<Vec<u8>> {
    let header_size = read_u32_le(&bytes, 0)? as usize;
    if header_size < 40 || bytes.len() < header_size {
        return None;
    }

    let width = read_i32_le(&bytes, 4)?;
    let signed_height = read_i32_le(&bytes, 8)?;
    let planes = read_u16_le(&bytes, 12)?;
    let bit_count = read_u16_le(&bytes, 14)?;
    let compression = read_u32_le(&bytes, 16)?;
    let colors_used = read_u32_le(&bytes, 32)?;
    if planes != 1 || width == 0 || signed_height == 0 {
        return None;
    }

    let top_down = signed_height < 0;
    let height = signed_height.unsigned_abs();
    let width_u = width.unsigned_abs();
    let pixel_offset = dib_pixel_offset(header_size, bit_count, compression, colors_used)?;
    if bytes.len() < pixel_offset {
        return None;
    }

    match (bit_count, compression) {
        (32, 0) | (32, 3) | (32, 6) => {
            dib_32_to_png(&bytes, pixel_offset, width_u, height, top_down)
        }
        (24, 0) => dib_24_to_png(&bytes, pixel_offset, width_u, height, top_down),
        _ => {
            let mut bmp = Vec::with_capacity(bytes.len() + 14);
            bmp.extend_from_slice(b"BM");
            bmp.extend_from_slice(&((bytes.len() + 14) as u32).to_le_bytes());
            bmp.extend_from_slice(&[0, 0, 0, 0]);
            bmp.extend_from_slice(&(14u32 + pixel_offset as u32).to_le_bytes());
            bmp.extend_from_slice(&bytes);
            image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp)
                .ok()
                .and_then(dynamic_image_to_png)
        }
    }
}

#[cfg(target_os = "windows")]
fn dib_pixel_offset(
    header_size: usize,
    bit_count: u16,
    compression: u32,
    colors_used: u32,
) -> Option<usize> {
    let mask_bytes = if header_size == 40 {
        match compression {
            3 => 12,
            6 => 16,
            _ => 0,
        }
    } else {
        0
    };
    let color_count = if colors_used > 0 {
        colors_used as usize
    } else if bit_count <= 8 {
        1usize.checked_shl(bit_count as u32)?
    } else {
        0
    };
    header_size
        .checked_add(mask_bytes)?
        .checked_add(color_count.checked_mul(4)?)
}

#[cfg(target_os = "windows")]
fn dib_32_to_png(
    bytes: &[u8],
    pixel_offset: usize,
    width: u32,
    height: u32,
    top_down: bool,
) -> Option<Vec<u8>> {
    let row_len = width.checked_mul(4)? as usize;
    let pixel_len = row_len.checked_mul(height as usize)?;
    let end = pixel_offset.checked_add(pixel_len)?;
    if bytes.len() < end {
        return None;
    }
    let src = &bytes[pixel_offset..end];
    let mut rgba = vec![0u8; pixel_len];
    for y in 0..height as usize {
        let src_y = if top_down { y } else { height as usize - 1 - y };
        let src_row = &src[src_y * row_len..src_y * row_len + row_len];
        let dst_row = &mut rgba[y * row_len..y * row_len + row_len];
        for (dst, src) in dst_row.chunks_exact_mut(4).zip(src_row.chunks_exact(4)) {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
    }
    if rgba.chunks_exact(4).all(|pixel| pixel[3] == 0) {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }
    rgba_pixels_to_png(width, height, rgba)
}

#[cfg(target_os = "windows")]
fn dib_24_to_png(
    bytes: &[u8],
    pixel_offset: usize,
    width: u32,
    height: u32,
    top_down: bool,
) -> Option<Vec<u8>> {
    let row_stride = (((width as usize * 24) + 31) / 32) * 4;
    let pixel_len = row_stride.checked_mul(height as usize)?;
    let end = pixel_offset.checked_add(pixel_len)?;
    if bytes.len() < end {
        return None;
    }
    let src = &bytes[pixel_offset..end];
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height as usize {
        let src_y = if top_down { y } else { height as usize - 1 - y };
        let src_row = &src[src_y * row_stride..src_y * row_stride + row_stride];
        let dst_row = &mut rgba[y * width as usize * 4..(y + 1) * width as usize * 4];
        for x in 0..width as usize {
            let src = &src_row[x * 3..x * 3 + 3];
            let dst = &mut dst_row[x * 4..x * 4 + 4];
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = 255;
        }
    }
    rgba_pixels_to_png(width, height, rgba)
}

#[cfg(target_os = "windows")]
fn bgra_pixels_to_png(
    width: u32,
    height: u32,
    mut pixels: Vec<u8>,
    premultiplied: bool,
) -> Option<Vec<u8>> {
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        if premultiplied && chunk[3] == 0 {
            chunk[3] = 255;
        }
    }
    rgba_pixels_to_png(width, height, pixels)
}

#[cfg(target_os = "windows")]
fn rgba_pixels_to_png(width: u32, height: u32, pixels: Vec<u8>) -> Option<Vec<u8>> {
    let image = image::RgbaImage::from_raw(width, height, pixels)?;
    dynamic_image_to_png(image::DynamicImage::ImageRgba8(image))
}

#[cfg(target_os = "windows")]
fn dynamic_image_to_png(image: image::DynamicImage) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .ok()?;
    Some(bytes)
}

#[cfg(target_os = "windows")]
fn clipboard_bytes_id(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[cfg(target_os = "windows")]
fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

#[cfg(target_os = "windows")]
fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(target_os = "windows")]
fn read_i32_le(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    fn dib_header(width: i32, height: i32, bit_count: u16, image_size: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&bit_count.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(image_size as u32).to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes
    }

    fn decode_png(bytes: &[u8]) -> image::RgbaImage {
        image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .expect("png should decode")
            .to_rgba8()
    }

    #[test]
    fn dib_24_bit_bottom_up_converts_to_png() {
        let mut dib = dib_header(2, 2, 24, 16);
        dib.extend_from_slice(&[
            255, 0, 0, 255, 255, 255, 0, 0, // bottom row: blue, white, padding
            0, 0, 255, 0, 255, 0, 0, 0, // top row: red, green, padding
        ]);

        let png = dib_bytes_to_png(dib).expect("dib should convert");
        let image = decode_png(&png);

        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0, 255, 0, 255]);
        assert_eq!(image.get_pixel(0, 1).0, [0, 0, 255, 255]);
        assert_eq!(image.get_pixel(1, 1).0, [255, 255, 255, 255]);
    }

    #[test]
    fn dib_32_bit_top_down_all_zero_alpha_becomes_opaque() {
        let mut dib = dib_header(1, -1, 32, 4);
        dib.extend_from_slice(&[10, 20, 30, 0]);

        let png = dib_bytes_to_png(dib).expect("dib should convert");
        let image = decode_png(&png);

        assert_eq!(image.get_pixel(0, 0).0, [30, 20, 10, 255]);
    }

    #[test]
    fn dib_pixel_offset_accounts_for_masks_and_palettes() {
        assert_eq!(dib_pixel_offset(40, 32, 3, 0), Some(52));
        assert_eq!(dib_pixel_offset(40, 32, 6, 0), Some(56));
        assert_eq!(dib_pixel_offset(40, 8, 0, 0), Some(1064));
        assert_eq!(dib_pixel_offset(124, 32, 3, 0), Some(124));
    }
}

#[cfg(target_os = "macos")]
fn read_file_list_macos() -> Option<Vec<String>> {
    autoreleasepool(|| unsafe {
        let pasteboard = general_pasteboard()?;
        if !pasteboard_has_any_type(pasteboard, &[FILE_URL_TYPE, FILE_NAMES_TYPE]) {
            return None;
        }
        read_file_list_from_pasteboard(pasteboard)
    })
}

#[cfg(target_os = "macos")]
fn write_file_list_macos(paths: &[String]) -> anyhow::Result<()> {
    autoreleasepool(|| unsafe {
        let pasteboard = general_pasteboard()
            .ok_or_else(|| anyhow::anyhow!("failed to access general pasteboard"))?;
        clear_pasteboard(pasteboard)?;
        let urls = nsarray_of_file_urls(paths)?;
        let written = send_bool_arg(pasteboard, "writeObjects:", urls);
        if !written {
            anyhow::bail!("failed to write file URLs to pasteboard");
        }
        Ok(())
    })
}

#[cfg(target_os = "macos")]
const UTF8_ENCODING: usize = 4;
#[cfg(target_os = "macos")]
const STRING_TYPE: &str = "public.utf8-plain-text";
#[cfg(target_os = "macos")]
const FILE_URL_TYPE: &str = "public.file-url";
#[cfg(target_os = "macos")]
const FILE_NAMES_TYPE: &str = "NSFilenamesPboardType";
#[cfg(target_os = "macos")]
const IMAGE_TYPES: [(&str, ImageFormat); 6] = [
    ("public.png", ImageFormat::Png),
    ("public.jpeg", ImageFormat::Jpeg),
    ("public.tiff", ImageFormat::Tiff),
    ("org.webmproject.webp", ImageFormat::Webp),
    ("com.compuserve.gif", ImageFormat::Gif),
    ("com.microsoft.bmp", ImageFormat::Bmp),
];

#[cfg(target_os = "macos")]
fn read_background_snapshot_macos(
    change_count: i64,
    skip_image_id: Option<u64>,
) -> ClipboardSnapshot {
    autoreleasepool(|| unsafe {
        let Some(pasteboard) = general_pasteboard() else {
            return ClipboardSnapshot::default();
        };

        let files = if pasteboard_has_any_type(pasteboard, &[FILE_URL_TYPE, FILE_NAMES_TYPE]) {
            read_file_list_from_pasteboard(pasteboard)
        } else {
            None
        };

        let text = if pasteboard_has_any_type(pasteboard, &[STRING_TYPE]) {
            read_text_from_pasteboard(pasteboard)
        } else {
            None
        };

        let image_id = change_count.max(0) as u64;
        let image = if skip_image_id == Some(image_id) {
            None
        } else {
            read_image_from_pasteboard(pasteboard, image_id)
        };

        ClipboardSnapshot { files, text, image }
    })
}

#[cfg(target_os = "macos")]
fn autoreleasepool<T>(f: impl FnOnce() -> T) -> T {
    unsafe {
        let Some(pool_class) = get_class("NSAutoreleasePool") else {
            return f();
        };
        let pool = send_id(send_id(pool_class, "alloc"), "init");
        let value = f();
        if !pool.is_null() {
            send_void(pool, "drain");
        }
        value
    }
}

#[cfg(target_os = "macos")]
unsafe fn general_pasteboard() -> Option<*mut std::ffi::c_void> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let class = get_class("NSPasteboard")?;
        let pasteboard = send_id(class, "generalPasteboard");
        (!pasteboard.is_null()).then_some(pasteboard)
    }
}

#[cfg(target_os = "macos")]
unsafe fn read_text_from_pasteboard(pasteboard: *mut std::ffi::c_void) -> Option<String> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let string_type = nsstring(STRING_TYPE)?;
        let value = send_id_arg(pasteboard, "stringForType:", string_type);
        nsstring_to_string(value)
            .map(|text| text.trim_end_matches('\0').to_string())
            .filter(|text| !text.is_empty())
    }
}

#[cfg(target_os = "macos")]
unsafe fn read_image_from_pasteboard(
    pasteboard: *mut std::ffi::c_void,
    image_id: u64,
) -> Option<ClipboardImage> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        for (uti, format) in IMAGE_TYPES {
            let uti = match nsstring(uti) {
                Some(value) => value,
                None => continue,
            };
            let data = send_id_arg(pasteboard, "dataForType:", uti);
            let Some(bytes) = nsdata_to_vec(data) else {
                continue;
            };
            if bytes.is_empty() {
                continue;
            }
            return Some(ClipboardImage {
                id: image_id,
                format,
                bytes,
            });
        }
        None
    }
}

#[cfg(target_os = "macos")]
unsafe fn read_file_list_from_pasteboard(pasteboard: *mut std::ffi::c_void) -> Option<Vec<String>> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        if let Some(paths) = read_file_urls_from_items(pasteboard) {
            if !paths.is_empty() {
                return Some(paths);
            }
        }

        let file_names_type = nsstring(FILE_NAMES_TYPE)?;
        let values = send_id_arg(pasteboard, "propertyListForType:", file_names_type);
        nsarray_to_strings(values)
    }
}

#[cfg(target_os = "macos")]
unsafe fn read_file_urls_from_items(pasteboard: *mut std::ffi::c_void) -> Option<Vec<String>> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let items = send_id(pasteboard, "pasteboardItems");
        let count = nsarray_count(items)?;
        if count == 0 {
            return None;
        }

        let file_url_type = nsstring(FILE_URL_TYPE)?;
        let url_class = get_class("NSURL")?;
        let mut paths = Vec::new();

        for index in 0..count {
            let item = nsarray_object_at(items, index)?;
            let value = send_id_arg(item, "stringForType:", file_url_type);
            if value.is_null() {
                continue;
            }
            let url = send_id_arg(url_class, "URLWithString:", value);
            if url.is_null() || !send_bool(url, "isFileURL") {
                continue;
            }
            let path = send_id(url, "path");
            if let Some(path) = nsstring_to_string(path).filter(|path| !path.is_empty()) {
                paths.push(path);
            }
        }

        (!paths.is_empty()).then_some(paths)
    }
}

#[cfg(target_os = "macos")]
unsafe fn pasteboard_has_any_type(pasteboard: *mut std::ffi::c_void, wanted: &[&str]) -> bool {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let Some(types) = pasteboard_types(pasteboard) else {
            return false;
        };
        types
            .iter()
            .any(|ty| wanted.iter().any(|wanted| ty == wanted))
    }
}

#[cfg(target_os = "macos")]
unsafe fn pasteboard_types(pasteboard: *mut std::ffi::c_void) -> Option<Vec<String>> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        nsarray_to_strings(send_id(pasteboard, "types"))
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsarray_to_strings(array: *mut std::ffi::c_void) -> Option<Vec<String>> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let count = nsarray_count(array)?;
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            let item = nsarray_object_at(array, index)?;
            if let Some(text) = nsstring_to_string(item) {
                values.push(text);
            }
        }
        (!values.is_empty()).then_some(values)
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsarray_count(array: *mut std::ffi::c_void) -> Option<usize> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        (!array.is_null()).then(|| send_usize(array, "count"))
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsarray_object_at(
    array: *mut std::ffi::c_void,
    index: usize,
) -> Option<*mut std::ffi::c_void> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let object = send_id_usize_arg(array, "objectAtIndex:", index);
        (!object.is_null()).then_some(object)
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsdata_to_vec(data: *mut std::ffi::c_void) -> Option<Vec<u8>> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        if data.is_null() {
            return None;
        }
        let len = send_usize(data, "length");
        if len == 0 {
            return Some(Vec::new());
        }
        let bytes = send_ptr(data, "bytes");
        if bytes.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(bytes.cast::<u8>(), len).to_vec())
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsarray_of_file_urls(paths: &[String]) -> anyhow::Result<*mut std::ffi::c_void> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let array =
            new_mutable_array().ok_or_else(|| anyhow::anyhow!("failed to allocate NSMutableArray"))?;
        let url_class = get_class("NSURL").ok_or_else(|| anyhow::anyhow!("failed to load NSURL"))?;
        for path in paths {
            let ns_path = nsstring(path)
                .ok_or_else(|| anyhow::anyhow!("failed to convert path into NSString"))?;
            let url = send_id_arg(url_class, "fileURLWithPath:", ns_path);
            if url.is_null() {
                anyhow::bail!("failed to create file URL for path: {path}");
            }
            send_void_arg(array, "addObject:", url);
        }
        Ok(array)
    }
}

#[cfg(target_os = "macos")]
unsafe fn new_mutable_array() -> Option<*mut std::ffi::c_void> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let class = get_class("NSMutableArray")?;
        let array = send_id(class, "alloc");
        let array = send_id(array, "init");
        (!array.is_null()).then_some(array)
    }
}

#[cfg(target_os = "macos")]
unsafe fn clear_pasteboard(pasteboard: *mut std::ffi::c_void) -> anyhow::Result<()> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let cleared = send_isize(pasteboard, "clearContents");
        if cleared < 0 {
            anyhow::bail!("failed to clear pasteboard");
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsstring(text: &str) -> Option<*mut std::ffi::c_void> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        let class = get_class("NSString")?;
        let value = send_id(class, "alloc");
        if value.is_null() {
            return None;
        }
        let string = send_id_bytes_len_usize_arg(
            value,
            "initWithBytes:length:encoding:",
            text.as_ptr().cast(),
            text.len(),
            UTF8_ENCODING,
        );
        (!string.is_null()).then_some(string)
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(value: *mut std::ffi::c_void) -> Option<String> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        if value.is_null() {
            return None;
        }
        let ptr = send_cstr(value, "UTF8String");
        (!ptr.is_null()).then(|| std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "macos")]
unsafe fn get_class(name: &str) -> Option<*mut std::ffi::c_void> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        use std::os::raw::c_char;

        unsafe extern "C" {
            fn objc_getClass(name: *const c_char) -> *mut std::ffi::c_void;
        }

        let class = objc_getClass(std::ffi::CString::new(name).ok()?.as_ptr());
        (!class.is_null()).then_some(class)
    }
}

#[cfg(target_os = "macos")]
unsafe fn selector(name: &str) -> Option<*mut std::ffi::c_void> {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        use std::os::raw::c_char;

        unsafe extern "C" {
            fn sel_registerName(name: *const c_char) -> *mut std::ffi::c_void;
        }

        let selector = sel_registerName(std::ffi::CString::new(name).ok()?.as_ptr());
        (!selector.is_null()).then_some(selector)
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_id(object: *mut std::ffi::c_void, selector_name: &str) -> *mut std::ffi::c_void {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend =
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> *mut std::ffi::c_void;

        let Some(sel) = selector(selector_name) else {
            return std::ptr::null_mut();
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel)
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_void(object: *mut std::ffi::c_void, selector_name: &str) {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void);

        let Some(sel) = selector(selector_name) else {
            return;
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel);
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_bool(object: *mut std::ffi::c_void, selector_name: &str) -> bool {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i8;

        let Some(sel) = selector(selector_name) else {
            return false;
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel) != 0
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_usize(object: *mut std::ffi::c_void, selector_name: &str) -> usize {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> usize;

        let Some(sel) = selector(selector_name) else {
            return 0;
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel)
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_ptr(object: *mut std::ffi::c_void, selector_name: &str) -> *const std::ffi::c_void {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
        ) -> *const std::ffi::c_void;

        let Some(sel) = selector(selector_name) else {
            return std::ptr::null();
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel)
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_cstr(
    object: *mut std::ffi::c_void,
    selector_name: &str,
) -> *const std::os::raw::c_char {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        send_ptr(object, selector_name).cast()
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_bool_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: *mut std::ffi::c_void,
) -> bool {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
        ) -> i8;

        let Some(sel) = selector(selector_name) else {
            return false;
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel, arg) != 0
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_id_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;

        let Some(sel) = selector(selector_name) else {
            return std::ptr::null_mut();
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel, arg)
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_void_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: *mut std::ffi::c_void,
) {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend =
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void);

        let Some(sel) = selector(selector_name) else {
            return;
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel, arg);
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_isize(object: *mut std::ffi::c_void, selector_name: &str) -> isize {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> isize;

        let Some(sel) = selector(selector_name) else {
            return -1;
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel)
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_id_usize_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: usize,
) -> *mut std::ffi::c_void {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            usize,
        ) -> *mut std::ffi::c_void;

        let Some(sel) = selector(selector_name) else {
            return std::ptr::null_mut();
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel, arg)
    }
}

#[cfg(target_os = "macos")]
unsafe fn send_id_bytes_len_usize_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    bytes: *const std::ffi::c_void,
    len: usize,
    arg: usize,
) -> *mut std::ffi::c_void {
    // SAFETY: all operations in this function are Objective-C runtime
    // calls or other well-defined unsafe operations.
    unsafe {
        unsafe extern "C" {
            fn objc_msgSend();
        }

        type MsgSend = unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            *const std::ffi::c_void,
            usize,
            usize,
        ) -> *mut std::ffi::c_void;

        let Some(sel) = selector(selector_name) else {
            return std::ptr::null_mut();
        };
        let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        msg_send(object, sel, bytes, len, arg)
    }
}
