use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use image::{DynamicImage, ImageBuffer, Rgba};
use windows::{
    Win32::{
        Foundation::{HWND, SIZE},
        Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap,
            CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetObjectW,
            HBITMAP, HDC, HGDIOBJ, SelectObject,
        },
        Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, IPersistFile, STGM_READ,
        },
        UI::{
            Shell::{
                IShellItemImageFactory, IShellLinkW, SHCreateItemFromParsingName, SHFILEINFOW,
                SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
                SLGP_RAWPATH, ShellExecuteW, ShellLink,
            },
            WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx, HICON, SW_SHOWNORMAL},
        },
    },
    core::{Interface, PCWSTR},
};

use super::{InstalledApp, app_aliases, bundle_id_suffix, icon_cache_path, save_icon_image};

const START_MENU_SUFFIX: &[&str] = &["Microsoft", "Windows", "Start Menu", "Programs"];
const MAX_SHORTCUT_SCAN_DEPTH: usize = 8;
const MAX_PATH_CHARS: usize = 32768;
const ICON_SIZE: i32 = 64;

#[derive(Clone, Debug)]
struct ShortcutMetadata {
    shortcut_path: PathBuf,
    name: String,
    target_path: Option<PathBuf>,
    icon_path: Option<PathBuf>,
    icon_index: i32,
}

pub(super) fn scan_application_metadata() -> Vec<InstalledApp> {
    let _com = ComApartment::init();
    let mut apps = Vec::new();
    let mut seen = HashSet::new();

    for shortcut in collect_shortcuts() {
        let metadata = shortcut_metadata(&shortcut);
        if should_skip_shortcut(&metadata) {
            tracing::debug!(path = %shortcut.display(), "skip start menu shortcut");
            continue;
        }

        let dedupe_key = metadata
            .target_path
            .as_ref()
            .unwrap_or(&metadata.shortcut_path)
            .to_string_lossy()
            .to_lowercase();
        if dedupe_key.is_empty() || !seen.insert(dedupe_key.clone()) {
            continue;
        }

        let target_stem = metadata
            .target_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|stem| stem.to_str());
        let target_path = metadata.target_path.as_ref().and_then(|path| path.to_str());
        let aliases = app_aliases(
            [
                Some(metadata.name.as_str()),
                target_stem,
                target_path,
                Some(dedupe_key.as_str()),
                target_path.and_then(bundle_id_suffix),
            ],
            &metadata.name,
        );

        apps.push(InstalledApp {
            icon_letter: metadata
                .name
                .chars()
                .next()
                .map(|ch| ch.to_string())
                .unwrap_or_else(|| String::from("A")),
            aliases,
            bundle_id: metadata
                .target_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            icon_path: None,
            name: metadata.name,
            path: metadata.shortcut_path.to_string_lossy().to_string(),
        });
    }

    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

pub(super) fn scan_application_paths() -> Vec<String> {
    let _com = ComApartment::init();
    let mut paths = collect_shortcuts()
        .into_iter()
        .filter_map(|shortcut| {
            let metadata = shortcut_metadata(&shortcut);
            if should_skip_shortcut(&metadata) {
                None
            } else {
                metadata.shortcut_path.to_str().map(ToOwned::to_owned)
            }
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub(super) fn populate_application_icons(apps: &mut [InstalledApp]) {
    let _com = ComApartment::init();
    let mut metadata_cache = HashMap::<PathBuf, ShortcutMetadata>::new();

    for app in apps {
        if app.icon_path.is_some() {
            continue;
        }
        let shortcut_path = PathBuf::from(&app.path);
        let metadata = metadata_cache
            .entry(shortcut_path.clone())
            .or_insert_with(|| shortcut_metadata(&shortcut_path));
        app.icon_path = extract_icon(metadata);
    }
}

pub(super) fn open_application(path: &str) -> Result<(), String> {
    let path_wide = wide_null(path);
    let open = wide_null("open");
    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            PCWSTR(open.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            PCWSTR(std::ptr::null()),
            PCWSTR(std::ptr::null()),
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("启动失败: ShellExecuteW 返回 {code}"))
    }
}

fn collect_shortcuts() -> Vec<PathBuf> {
    let mut shortcuts = Vec::new();
    let roots = start_menu_roots();
    for root in roots {
        collect_lnk_files(&root, 0, &mut shortcuts);
    }
    shortcuts.sort();
    shortcuts.dedup();
    shortcuts
}

fn start_menu_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(program_data) = std::env::var("ProgramData") {
        roots.push(join_segments(
            PathBuf::from(program_data),
            START_MENU_SUFFIX,
        ));
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        roots.push(join_segments(PathBuf::from(app_data), START_MENU_SUFFIX));
    }
    roots
}

fn join_segments(mut base: PathBuf, segments: &[&str]) -> PathBuf {
    for segment in segments {
        base.push(segment);
    }
    base
}

fn collect_lnk_files(directory: &Path, depth: usize, shortcuts: &mut Vec<PathBuf>) {
    if depth > MAX_SHORTCUT_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_lnk_files(&path, depth + 1, shortcuts);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
        {
            shortcuts.push(path);
        }
    }
}

fn shortcut_metadata(path: &Path) -> ShortcutMetadata {
    resolve_shell_link(path).unwrap_or_else(|| ShortcutMetadata {
        shortcut_path: path.to_path_buf(),
        name: shortcut_display_name(path),
        target_path: None,
        icon_path: None,
        icon_index: 0,
    })
}

fn resolve_shell_link(path: &Path) -> Option<ShortcutMetadata> {
    let link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()? };
    let persist: IPersistFile = link.cast().ok()?;
    let shortcut_wide = wide_null(path.as_os_str());
    unsafe {
        persist
            .Load(PCWSTR(shortcut_wide.as_ptr()), STGM_READ)
            .ok()?
    };

    let mut target_buffer = vec![0u16; MAX_PATH_CHARS];
    let _ = unsafe {
        link.GetPath(
            &mut target_buffer,
            std::ptr::null_mut(),
            SLGP_RAWPATH.0 as u32,
        )
    };
    let target_path =
        path_from_wide_buffer(&target_buffer).filter(|target| !target.as_os_str().is_empty());

    let mut icon_buffer = vec![0u16; MAX_PATH_CHARS];
    let mut icon_index = 0;
    let _ = unsafe { link.GetIconLocation(&mut icon_buffer, &mut icon_index) };
    let icon_path = path_from_wide_buffer(&icon_buffer).filter(|icon| icon.is_file());

    Some(ShortcutMetadata {
        shortcut_path: path.to_path_buf(),
        name: shortcut_display_name(path),
        target_path,
        icon_path,
        icon_index,
    })
}

fn shortcut_display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| String::from("应用"))
}

fn should_skip_shortcut(metadata: &ShortcutMetadata) -> bool {
    let name = metadata.name.to_lowercase();
    if ["uninstall", "readme", "help", "卸载", "帮助"]
        .iter()
        .any(|needle| name.contains(needle))
    {
        return true;
    }

    metadata
        .target_path
        .as_ref()
        .is_some_and(|path| path.to_string_lossy().trim().is_empty())
}

fn extract_icon(metadata: &ShortcutMetadata) -> Option<String> {
    let cache_path = icon_cache_path(&metadata.shortcut_path);
    if let Ok(path) = extract_icon_from_shell_item(&metadata.shortcut_path, &cache_path) {
        return Some(path);
    }

    if let Some(icon_path) = metadata.icon_path.as_ref()
        && let Ok(path) = extract_icon_from_shell_item(icon_path, &cache_path)
    {
        return Some(path);
    }

    if let Some(target_path) = metadata.target_path.as_ref()
        && let Ok(path) = extract_icon_from_shell_item(target_path, &cache_path)
    {
        return Some(path);
    }

    let fallback_path = metadata
        .icon_path
        .as_ref()
        .or(metadata.target_path.as_ref())
        .unwrap_or(&metadata.shortcut_path);
    extract_icon_with_shgetfileinfo(fallback_path, &cache_path).ok()
}

fn extract_icon_from_shell_item(path: &Path, cache_path: &Path) -> Result<String, String> {
    let wide = wide_null(path.as_os_str());
    let factory: IShellItemImageFactory = unsafe {
        SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)
            .map_err(|error| error.to_string())?
    };
    let bitmap = unsafe {
        factory
            .GetImage(
                SIZE {
                    cx: ICON_SIZE,
                    cy: ICON_SIZE,
                },
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            )
            .map_err(|error| error.to_string())?
    };
    let image = unsafe { image_from_hbitmap(bitmap) };
    unsafe {
        let _ = DeleteObject(HGDIOBJ::from(bitmap));
    }
    save_icon_image(image?, cache_path)
}

fn extract_icon_with_shgetfileinfo(path: &Path, cache_path: &Path) -> Result<String, String> {
    let wide = wide_null(path.as_os_str());
    let mut info = SHFILEINFOW::default();
    let ok = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if ok == 0 || info.hIcon.is_invalid() {
        return Err(String::from("SHGetFileInfoW did not return an icon"));
    }

    let image = unsafe { image_from_hicon(info.hIcon) };
    unsafe {
        let _ = DestroyIcon(info.hIcon);
    }
    save_icon_image(image?, cache_path)
}

unsafe fn image_from_hicon(icon: HICON) -> Result<DynamicImage, String> {
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return Err(String::from("CreateCompatibleDC failed"));
    }
    let bitmap = unsafe { CreateCompatibleBitmap(dc, ICON_SIZE, ICON_SIZE) };
    if bitmap.is_invalid() {
        unsafe {
            let _ = DeleteDC(dc);
        }
        return Err(String::from("CreateCompatibleBitmap failed"));
    }

    let previous = unsafe { SelectObject(dc, HGDIOBJ::from(bitmap)) };
    let draw_result =
        unsafe { DrawIconEx(dc, 0, 0, icon, ICON_SIZE, ICON_SIZE, 0, None, DI_NORMAL) };
    unsafe {
        SelectObject(dc, previous);
    }
    if draw_result.is_err() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ::from(bitmap));
            let _ = DeleteDC(dc);
        }
        return Err(String::from("DrawIconEx failed"));
    }

    let image = unsafe { image_from_hbitmap_with_dc(bitmap, dc) };
    unsafe {
        let _ = DeleteObject(HGDIOBJ::from(bitmap));
        let _ = DeleteDC(dc);
    }
    image
}

unsafe fn image_from_hbitmap(bitmap: HBITMAP) -> Result<DynamicImage, String> {
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return Err(String::from("CreateCompatibleDC failed"));
    }
    let image = unsafe { image_from_hbitmap_with_dc(bitmap, dc) };
    unsafe {
        let _ = DeleteDC(dc);
    }
    image
}

unsafe fn image_from_hbitmap_with_dc(bitmap: HBITMAP, dc: HDC) -> Result<DynamicImage, String> {
    let mut bitmap_info = BITMAP::default();
    let object_size = std::mem::size_of::<BITMAP>() as i32;
    let got = unsafe {
        GetObjectW(
            HGDIOBJ::from(bitmap),
            object_size,
            Some((&mut bitmap_info as *mut BITMAP).cast()),
        )
    };
    if got == 0 || bitmap_info.bmWidth <= 0 || bitmap_info.bmHeight <= 0 {
        return Err(String::from("GetObjectW failed"));
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
    if lines == 0 {
        return Err(String::from("GetDIBits failed"));
    }

    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| String::from("invalid icon bitmap buffer"))?;
    Ok(DynamicImage::ImageRgba8(image))
}

fn path_from_wide_buffer(buffer: &[u16]) -> Option<PathBuf> {
    let len = buffer
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(buffer.len());
    if len == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
}

fn wide_null(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    fn init() -> Self {
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self {
            initialized: result.is_ok(),
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}
