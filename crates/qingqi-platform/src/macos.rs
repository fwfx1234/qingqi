use std::fmt;

/// Accessibility permission status for the current process on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionStatus {
    /// The process has accessibility authorization.
    Authorized,
    /// The process does not have accessibility authorization.
    NotAuthorized,
    /// The platform cannot determine the status (e.g. non-macos).
    Unknown,
}

impl PermissionStatus {
    pub fn is_authorized(self) -> bool {
        matches!(self, PermissionStatus::Authorized)
    }
}

impl fmt::Display for PermissionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionStatus::Authorized => write!(f, "已授权"),
            PermissionStatus::NotAuthorized => write!(f, "未授权"),
            PermissionStatus::Unknown => write!(f, "未知"),
        }
    }
}

/// Check macOS accessibility authorization without prompting the user.
#[cfg(target_os = "macos")]
pub fn check_accessibility() -> PermissionStatus {
    use std::os::raw::c_int;

    unsafe extern "C" {
        fn AXIsProcessTrusted() -> c_int;
    }

    if unsafe { AXIsProcessTrusted() } != 0 {
        PermissionStatus::Authorized
    } else {
        PermissionStatus::NotAuthorized
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility() -> PermissionStatus {
    PermissionStatus::Unknown
}

/// Open macOS System Settings to the Accessibility privacy pane.
/// Optionally triggers the authorization prompt first (like suishou's behavior).
#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() -> bool {
    // Trigger the authorization prompt (same as suishou's _request_accessibility_prompt)
    prompt_accessibility();

    // Open both possible URLs for different macOS versions
    let urls = [
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
    ];
    let mut any_ok = false;
    for url in &urls {
        let ok = std::process::Command::new("open")
            .arg(url)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        any_ok = any_ok || ok;
    }
    any_ok
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() -> bool {
    false
}

/// Trigger the macOS accessibility authorization prompt (does not block).
/// Note: requires core-foundation crate for full CFDictionary construction.
/// Currently a no-op; can be enhanced when core-foundation is added as a dependency.
#[cfg(target_os = "macos")]
fn prompt_accessibility() {
    // TODO: add core-foundation dependency and call AXIsProcessTrustedWithOptions
    // with { "AXTrustedCheckOptionPrompt": true } to trigger the system prompt.
    // The open_accessibility_settings() call already takes the user to the right pane.
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn prompt_accessibility() {}

/// Hide the app from the macOS Dock while keeping menu bar/status item behavior available.
#[cfg(target_os = "macos")]
pub fn hide_dock_icon() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("cannot hide Dock icon outside the macOS main thread");
        return;
    };

    let app = NSApplication::sharedApplication(mtm);
    if !app.setActivationPolicy(NSApplicationActivationPolicy::Accessory) {
        tracing::warn!("failed to set macOS activation policy to accessory");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_dock_icon() {}

#[cfg(target_os = "macos")]
pub fn prepare_dock_agent_name(name: &str) {
    use objc2_foundation::{NSProcessInfo, NSString};

    NSProcessInfo::processInfo().setProcessName(&NSString::from_str(name));
}

#[cfg(not(target_os = "macos"))]
pub fn prepare_dock_agent_name(_name: &str) {}

#[cfg(target_os = "macos")]
pub fn configure_dock_agent(name: &str, icon_png: &[u8]) -> Result<(), String> {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{NSData, NSProcessInfo, NSString};

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| String::from("Dock agent must be configured on the macOS main thread"))?;
    let data = NSData::from_vec(icon_png.to_vec());
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| String::from("cannot decode Dock agent icon PNG"))?;
    let app = NSApplication::sharedApplication(mtm);
    unsafe { app.setApplicationIconImage(Some(&image)) };
    NSProcessInfo::processInfo().setProcessName(&NSString::from_str(name));
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn configure_dock_agent(_name: &str, _icon_png: &[u8]) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn restore_window(window: &gpui::Window) {
    use objc2_app_kit::NSView;
    use raw_window_handle::RawWindowHandle;

    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let Some(native_window) = view.window() else {
        return;
    };
    native_window.deminiaturize(None);
    native_window.makeKeyAndOrderFront(None);
}

#[cfg(not(target_os = "macos"))]
pub fn restore_window(_window: &gpui::Window) {}

/// Bring this accessory app to the foreground without changing Dock policy.
#[cfg(target_os = "macos")]
pub fn activate_frontmost() {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    let app = NSRunningApplication::currentApplication();
    #[allow(deprecated)]
    let options = NSApplicationActivationOptions::ActivateAllWindows
        | NSApplicationActivationOptions::ActivateIgnoringOtherApps;
    if !app.activateWithOptions(options) {
        tracing::warn!("failed to activate macOS running application");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn activate_frontmost() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_status_display() {
        assert_eq!(PermissionStatus::Authorized.to_string(), "已授权");
        assert_eq!(PermissionStatus::NotAuthorized.to_string(), "未授权");
        assert_eq!(PermissionStatus::Unknown.to_string(), "未知");
    }

    #[test]
    fn permission_status_is_authorized() {
        assert!(PermissionStatus::Authorized.is_authorized());
        assert!(!PermissionStatus::NotAuthorized.is_authorized());
        assert!(!PermissionStatus::Unknown.is_authorized());
    }

    #[test]
    fn check_accessibility_returns_valid_variant() {
        let status = check_accessibility();
        // On macOS, should be Authorized or NotAuthorized.
        // On non-macos, should be Unknown.
        #[cfg(not(target_os = "macos"))]
        assert_eq!(status, PermissionStatus::Unknown);

        #[cfg(target_os = "macos")]
        assert!(
            status == PermissionStatus::Authorized || status == PermissionStatus::NotAuthorized
        );
    }
}
