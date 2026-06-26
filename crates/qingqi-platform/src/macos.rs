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
