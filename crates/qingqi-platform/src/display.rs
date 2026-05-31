use gpui::{App, PlatformDisplay};
use std::rc::Rc;

pub fn active_display(cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    platform_active_display(cx).or_else(|| cx.primary_display())
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

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    type CGDirectDisplayID = u32;
    type CGError = i32;
    type CGEventRef = *mut c_void;
    type CGEventSourceRef = *mut c_void;

    const K_CG_NULL_DISPLAY_ID: CGDirectDisplayID = 0;
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
        fn CGMainDisplayID() -> CGDirectDisplayID;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    pub fn display_id_containing_mouse() -> Option<CGDirectDisplayID> {
        unsafe {
            let event = CGEventCreate(std::ptr::null_mut());
            if event.is_null() {
                return None;
            }

            let location = CGEventGetLocation(event);
            CFRelease(event.cast());

            let mut display = K_CG_NULL_DISPLAY_ID;
            let mut count = 0;
            let error = CGGetDisplaysWithPoint(location, 1, &mut display, &mut count);
            if error == 0 && count > 0 && display != K_CG_NULL_DISPLAY_ID {
                return Some(display);
            }

            let fallback = CGMainDisplayID();
            (fallback != K_CG_NULL_DISPLAY_ID).then_some(fallback)
        }
    }
}
