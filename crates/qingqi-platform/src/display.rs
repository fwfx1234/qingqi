use gpui::{App, PlatformDisplay};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayFrame {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn active_display(cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    platform_active_display(cx).or_else(|| cx.primary_display())
}

pub fn mouse_display_frame() -> Option<(DisplayPoint, DisplayFrame)> {
    #[cfg(target_os = "macos")]
    {
        macos::mouse_display_frame()
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn centered_on_active_display(
    cx: &App,
    size: gpui::Size<gpui::Pixels>,
) -> (Option<Rc<dyn PlatformDisplay>>, gpui::Bounds<gpui::Pixels>) {
    let display = active_display(cx);
    let bounds = display
        .as_ref()
        .map(|display| gpui::Bounds::centered_at(display.bounds().center(), size))
        .unwrap_or_else(|| gpui::Bounds::centered(None, size, cx));

    (display, bounds)
}

#[cfg(target_os = "macos")]
fn platform_active_display(cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    let display_id = macos::display_id_containing_mouse()?;
    cx.displays()
        .into_iter()
        .find(|display| u32::from(display.id()) == display_id)
}

#[cfg(not(target_os = "macos"))]
fn platform_active_display(_cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    None
}

// ── Display management types ─────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DisplayDescriptor {
    pub id: u32,
    pub name: String,
    pub vendor_id: u32,
    pub product_id: u32,
    pub serial_number: u32,
    pub is_builtin: bool,
    pub native_width: u32,
    pub native_height: u32,
    pub current_mode: Option<DisplayMode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayMode {
    pub key: DisplayModeKey,
    pub is_hidpi: bool,
}

impl DisplayMode {
    pub fn is_hidpi(&self) -> bool {
        self.is_hidpi
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayModeKey {
    pub width: u32,
    pub height: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub refresh_millihz: u32,
}

pub fn online_displays() -> anyhow::Result<Vec<DisplayDescriptor>> {
    #[cfg(target_os = "macos")]
    {
        macos::online_displays()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

pub fn display_modes(_display_id: u32) -> anyhow::Result<Vec<DisplayMode>> {
    #[cfg(target_os = "macos")]
    {
        macos::display_modes(display_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

pub fn display_descriptor(_display_id: u32) -> anyhow::Result<DisplayDescriptor> {
    #[cfg(target_os = "macos")]
    {
        macos::display_descriptor(display_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("display_descriptor is only supported on macOS")
    }
}

pub fn set_display_mode(_display_id: u32, _mode: DisplayModeKey) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos::set_display_mode(display_id, mode)
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("set_display_mode is only supported on macOS")
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    type CGDirectDisplayID = u32;
    type CGError = i32;
    type CGEventRef = *mut c_void;
    type CGEventSourceRef = *mut c_void;

    type CGDisplayModeRef = *const c_void;
    type CFStringRef = *const c_void;
    type io_object_t = u32;
    type io_service_t = u32;
    type CFTypeRef = *const c_void;

    const K_CG_NULL_DISPLAY_ID: CGDirectDisplayID = 0;
    const kCFStringEncodingUTF8: u32 = 0x08000100;
    const kCFAllocatorDefault: *const c_void = std::ptr::null();

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
        fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
        fn CGGetDisplaysWithPoint(
            point: CGPoint,
            max_displays: u32,
            displays: *mut CGDirectDisplayID,
            matching_display_count: *mut u32,
        ) -> CGError;
        fn CGGetActiveDisplayList(
            max_displays: u32,
            displays: *mut CGDirectDisplayID,
            matching_display_count: *mut u32,
        ) -> CGError;
        fn CGMainDisplayID() -> CGDirectDisplayID;
        fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
        fn CGDisplayIsBuiltin(display: CGDirectDisplayID) -> i32;
        fn CGDisplayPixelsWide(display: CGDirectDisplayID) -> isize;
        fn CGDisplayPixelsHigh(display: CGDirectDisplayID) -> isize;
        fn CGDisplayCopyAllDisplayModes(
            display: CGDirectDisplayID,
            options: *const c_void,
        ) -> *const c_void;
        fn CGDisplayModeGetPixelWidth(mode: CGDisplayModeRef) -> isize;
        fn CGDisplayModeGetPixelHeight(mode: CGDisplayModeRef) -> isize;
        fn CGDisplayModeGetWidth(mode: CGDisplayModeRef) -> isize;
        fn CGDisplayModeGetHeight(mode: CGDisplayModeRef) -> isize;
        fn CGDisplayModeGetRefreshRate(mode: CGDisplayModeRef) -> f64;
        fn CGDisplayIOServicePort(display: CGDirectDisplayID) -> io_service_t;
        fn CGDisplayVendorNumber(display: CGDirectDisplayID) -> u32;
        fn CGDisplayModelNumber(display: CGDirectDisplayID) -> u32;
        fn CGDisplaySerialNumber(display: CGDirectDisplayID) -> u32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFArrayGetCount(theArray: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(theArray: *const c_void, idx: isize) -> *const c_void;
        fn CFStringGetLength(theString: CFStringRef) -> isize;
        fn CFStringGetCString(
            theString: CFStringRef,
            buffer: *mut i8,
            bufferSize: isize,
            encoding: u32,
        ) -> bool;
        fn CFSTR(theString: *const i8) -> CFStringRef;
        fn IORegistryEntryCreateCFProperty(
            entry: io_service_t,
            key: CFStringRef,
            allocator: *const c_void,
            options: u32,
        ) -> CFTypeRef;
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOObjectRelease(object: io_object_t) -> i32;
    }

    pub fn display_id_containing_mouse() -> Option<CGDirectDisplayID> {
        let location = mouse_location()?;
        display_id_containing_point(location)
    }

    pub fn mouse_display_frame() -> Option<(super::DisplayPoint, super::DisplayFrame)> {
        let location = mouse_location()?;
        let display_id = display_id_containing_point(location)?;
        let bounds = unsafe { CGDisplayBounds(display_id) };
        Some((
            super::DisplayPoint {
                x: location.x,
                y: location.y,
            },
            super::DisplayFrame {
                id: display_id,
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
            },
        ))
    }

    fn mouse_location() -> Option<CGPoint> {
        unsafe {
            let event = CGEventCreate(std::ptr::null_mut());
            if event.is_null() {
                return None;
            }

            let location = CGEventGetLocation(event);
            CFRelease(event.cast());
            Some(location)
        }
    }

    fn display_id_containing_point(location: CGPoint) -> Option<CGDirectDisplayID> {
        unsafe {
            let mut display = K_CG_NULL_DISPLAY_ID;
            let mut count = 0;
            let error = CGGetDisplaysWithPoint(location, 1, &mut display, &mut count);
            if error == 0 && count > 0 && display != K_CG_NULL_DISPLAY_ID {
                Some(display)
            } else {
                let fallback = CGMainDisplayID();
                (fallback != K_CG_NULL_DISPLAY_ID).then_some(fallback)
            }
        }
    }

    fn online_display_ids() -> Vec<CGDirectDisplayID> {
        unsafe {
            let max_displays = 16u32;
            let mut displays = vec![0u32; max_displays as usize];
            let mut count = 0u32;
            let error = CGGetActiveDisplayList(max_displays, displays.as_mut_ptr(), &mut count);
            if error == 0 {
                displays.truncate(count as usize);
                displays
            } else {
                let main = CGMainDisplayID();
                if main != K_CG_NULL_DISPLAY_ID {
                    vec![main]
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn is_builtin_display(id: CGDirectDisplayID) -> bool {
        unsafe { CGDisplayIsBuiltin(id) != 0 }
    }

    fn display_native_size(id: CGDirectDisplayID) -> (u32, u32) {
        unsafe {
            let width = CGDisplayPixelsWide(id);
            let height = CGDisplayPixelsHigh(id);
            (width as u32, height as u32)
        }
    }

    fn display_vendor_id(id: CGDirectDisplayID) -> u32 {
        unsafe { CGDisplayVendorNumber(id) }
    }

    fn display_product_id(id: CGDirectDisplayID) -> u32 {
        unsafe { CGDisplayModelNumber(id) }
    }

    fn display_serial_number(id: CGDirectDisplayID) -> u32 {
        unsafe { CGDisplaySerialNumber(id) }
    }

    fn display_name(id: CGDirectDisplayID) -> String {
        let mut name = format!("Display {}", id);
        unsafe {
            let key = c"DisplayProductName";
            let service = CGDisplayIOServicePort(id);
            if service != 0 {
                let cf_name = IORegistryEntryCreateCFProperty(
                    service,
                    CFSTR(key.as_ptr()),
                    kCFAllocatorDefault,
                    0,
                );
                if !cf_name.is_null() {
                    let len = CFStringGetLength(cf_name as CFStringRef);
                    if len > 0 {
                        let mut buf = vec![0u8; (len * 4) as usize + 1];
                        CFStringGetCString(
                            cf_name as CFStringRef,
                            buf.as_mut_ptr() as *mut i8,
                            buf.len() as isize,
                            kCFStringEncodingUTF8,
                        );
                        if let Ok(s) = std::str::from_utf8(&buf) {
                            name = s.trim_end_matches('\0').to_string();
                        }
                    }
                    CFRelease(cf_name.cast());
                }
                IOObjectRelease(service);
            }
        }
        name
    }

    pub fn online_displays() -> anyhow::Result<Vec<super::DisplayDescriptor>> {
        let ids = online_display_ids();
        let mut displays = Vec::with_capacity(ids.len());
        for id in ids {
            let (native_width, native_height) = display_native_size(id);
            let vendor = display_vendor_id(id);
            let product = display_product_id(id);
            let serial = display_serial_number(id);
            displays.push(super::DisplayDescriptor {
                id,
                name: display_name(id),
                vendor_id: vendor,
                product_id: product,
                serial_number: serial,
                is_builtin: is_builtin_display(id),
                native_width,
                native_height,
                current_mode: None,
            });
        }
        Ok(displays)
    }

    pub fn display_modes(display_id: u32) -> anyhow::Result<Vec<super::DisplayMode>> {
        unsafe {
            let modes_ref = CGDisplayCopyAllDisplayModes(display_id, std::ptr::null());
            if modes_ref.is_null() {
                return Ok(Vec::new());
            }
            let count = CFArrayGetCount(modes_ref);
            let mut modes = Vec::new();
            for i in 0..count {
                let mode_ref = CFArrayGetValueAtIndex(modes_ref, i) as CGDisplayModeRef;
                if mode_ref.is_null() {
                    continue;
                }
                let pixel_width = CGDisplayModeGetPixelWidth(mode_ref) as u32;
                let pixel_height = CGDisplayModeGetPixelHeight(mode_ref) as u32;
                let width = CGDisplayModeGetWidth(mode_ref) as u32;
                let height = CGDisplayModeGetHeight(mode_ref) as u32;
                let refresh = CGDisplayModeGetRefreshRate(mode_ref) as u32;
                let is_hidpi = pixel_width == width * 2 && pixel_height == height * 2;
                modes.push(super::DisplayMode {
                    key: super::DisplayModeKey {
                        width,
                        height,
                        pixel_width,
                        pixel_height,
                        refresh_millihz: refresh * 1000,
                    },
                    is_hidpi,
                });
            }
            CFRelease(modes_ref.cast());
            Ok(modes)
        }
    }

    pub fn display_descriptor(display_id: u32) -> anyhow::Result<super::DisplayDescriptor> {
        let displays = online_displays()?;
        displays
            .into_iter()
            .find(|d| d.id == display_id)
            .ok_or_else(|| anyhow::anyhow!("display {} not found", display_id))
    }

    pub fn set_display_mode(_display_id: u32, _mode: super::DisplayModeKey) -> anyhow::Result<()> {
        anyhow::bail!("set_display_mode is not yet implemented")
    }
}
