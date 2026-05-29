use std::path::Path;

use gpui::{App, ClipboardEntry, ClipboardItem, Image, ImageFormat};

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

pub fn read_snapshot(cx: &App, skip_image_id: Option<u64>) -> ClipboardSnapshot {
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

/// Returns the system clipboard's monotonically increasing change counter,
/// used to cheaply skip polling work when nothing has changed.
///
/// Returns `None` when the value cannot be obtained or on non-macOS platforms,
/// in which case callers should fall back to reading the clipboard directly.
pub fn change_count() -> Option<i64> {
    #[cfg(target_os = "macos")]
    {
        change_count_macos()
    }
    #[cfg(not(target_os = "macos"))]
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
    #[cfg(target_os = "macos")]
    {
        read_background_snapshot_macos(change_count, skip_image_id)
    }
    #[cfg(not(target_os = "macos"))]
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
#[allow(unsafe_op_in_unsafe_fn)]
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
#[allow(unsafe_op_in_unsafe_fn)]
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
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn general_pasteboard() -> Option<*mut std::ffi::c_void> {
    let class = get_class("NSPasteboard")?;
    let pasteboard = send_id(class, "generalPasteboard");
    (!pasteboard.is_null()).then_some(pasteboard)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_text_from_pasteboard(pasteboard: *mut std::ffi::c_void) -> Option<String> {
    let string_type = nsstring(STRING_TYPE)?;
    let value = send_id_arg(pasteboard, "stringForType:", string_type);
    nsstring_to_string(value)
        .map(|text| text.trim_end_matches('\0').to_string())
        .filter(|text| !text.is_empty())
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_image_from_pasteboard(
    pasteboard: *mut std::ffi::c_void,
    image_id: u64,
) -> Option<ClipboardImage> {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_file_list_from_pasteboard(pasteboard: *mut std::ffi::c_void) -> Option<Vec<String>> {
    if let Some(paths) = read_file_urls_from_items(pasteboard) {
        if !paths.is_empty() {
            return Some(paths);
        }
    }

    let file_names_type = nsstring(FILE_NAMES_TYPE)?;
    let values = send_id_arg(pasteboard, "propertyListForType:", file_names_type);
    nsarray_to_strings(values)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_file_urls_from_items(pasteboard: *mut std::ffi::c_void) -> Option<Vec<String>> {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn pasteboard_has_any_type(pasteboard: *mut std::ffi::c_void, wanted: &[&str]) -> bool {
    let Some(types) = pasteboard_types(pasteboard) else {
        return false;
    };
    types
        .iter()
        .any(|ty| wanted.iter().any(|wanted| ty == wanted))
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn pasteboard_types(pasteboard: *mut std::ffi::c_void) -> Option<Vec<String>> {
    nsarray_to_strings(send_id(pasteboard, "types"))
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn nsarray_to_strings(array: *mut std::ffi::c_void) -> Option<Vec<String>> {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn nsarray_count(array: *mut std::ffi::c_void) -> Option<usize> {
    (!array.is_null()).then(|| send_usize(array, "count"))
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn nsarray_object_at(
    array: *mut std::ffi::c_void,
    index: usize,
) -> Option<*mut std::ffi::c_void> {
    let object = send_id_usize_arg(array, "objectAtIndex:", index);
    (!object.is_null()).then_some(object)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn nsdata_to_vec(data: *mut std::ffi::c_void) -> Option<Vec<u8>> {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn nsarray_of_file_urls(paths: &[String]) -> anyhow::Result<*mut std::ffi::c_void> {
    let array = new_mutable_array()
        .ok_or_else(|| anyhow::anyhow!("failed to allocate NSMutableArray"))?;
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn new_mutable_array() -> Option<*mut std::ffi::c_void> {
    let class = get_class("NSMutableArray")?;
    let array = send_id(class, "alloc");
    let array = send_id(array, "init");
    (!array.is_null()).then_some(array)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn clear_pasteboard(pasteboard: *mut std::ffi::c_void) -> anyhow::Result<()> {
    let cleared = send_isize(pasteboard, "clearContents");
    if cleared < 0 {
        anyhow::bail!("failed to clear pasteboard");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn nsstring(text: &str) -> Option<*mut std::ffi::c_void> {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn nsstring_to_string(value: *mut std::ffi::c_void) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let ptr = send_cstr(value, "UTF8String");
    (!ptr.is_null()).then(|| std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn get_class(name: &str) -> Option<*mut std::ffi::c_void> {
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut std::ffi::c_void;
    }

    let class = objc_getClass(std::ffi::CString::new(name).ok()?.as_ptr());
    (!class.is_null()).then_some(class)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn selector(name: &str) -> Option<*mut std::ffi::c_void> {
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn sel_registerName(name: *const c_char) -> *mut std::ffi::c_void;
    }

    let selector = sel_registerName(std::ffi::CString::new(name).ok()?.as_ptr());
    (!selector.is_null()).then_some(selector)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_id(object: *mut std::ffi::c_void, selector_name: &str) -> *mut std::ffi::c_void {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_void(object: *mut std::ffi::c_void, selector_name: &str) {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_bool(object: *mut std::ffi::c_void, selector_name: &str) -> bool {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_usize(object: *mut std::ffi::c_void, selector_name: &str) -> usize {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_ptr(object: *mut std::ffi::c_void, selector_name: &str) -> *const std::ffi::c_void {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_cstr(
    object: *mut std::ffi::c_void,
    selector_name: &str,
) -> *const std::os::raw::c_char {
    send_ptr(object, selector_name).cast()
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_bool_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: *mut std::ffi::c_void,
) -> bool {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_id_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_void_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: *mut std::ffi::c_void,
) {
    unsafe extern "C" {
        fn objc_msgSend();
    }

    type MsgSend = unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
    );

    let Some(sel) = selector(selector_name) else {
        return;
    };
    let msg_send: MsgSend = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    msg_send(object, sel, arg);
}

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_isize(object: *mut std::ffi::c_void, selector_name: &str) -> isize {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_id_usize_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    arg: usize,
) -> *mut std::ffi::c_void {
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

#[cfg(target_os = "macos")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn send_id_bytes_len_usize_arg(
    object: *mut std::ffi::c_void,
    selector_name: &str,
    bytes: *const std::ffi::c_void,
    len: usize,
    arg: usize,
) -> *mut std::ffi::c_void {
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
