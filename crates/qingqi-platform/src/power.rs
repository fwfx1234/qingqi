//! Power management: prevent system sleep via IOPMAssertion (macOS).

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreventSleepMode {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "always_on")]
    AlwaysOn,
    #[serde(rename = "when_plugged_in")]
    WhenPluggedIn,
}

impl PreventSleepMode {
    pub fn label(self) -> &'static str {
        match self {
            PreventSleepMode::Disabled => "不开启",
            PreventSleepMode::AlwaysOn => "始终开启",
            PreventSleepMode::WhenPluggedIn => "仅接入电源开启",
        }
    }
}

/// Manages system sleep prevention via IOPMAssertion on macOS.
pub struct PowerManager {
    mode: PreventSleepMode,
    assertion_id: u32,
    config_path: PathBuf,
}

impl PowerManager {
    pub fn load(config_path: PathBuf) -> Self {
        let mode = fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<PreventSleepMode>(&s).ok())
            .unwrap_or(PreventSleepMode::Disabled);

        let mut mgr = Self {
            mode,
            assertion_id: 0,
            config_path,
        };
        mgr.sync_assertion();
        mgr
    }

    pub fn mode(&self) -> PreventSleepMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: PreventSleepMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.sync_assertion();
        self.save();
    }

    /// Re-evaluate assertion state (called on timer for WhenPluggedIn mode).
    pub fn update(&mut self) {
        if self.mode == PreventSleepMode::WhenPluggedIn {
            self.sync_assertion();
        }
    }

    fn sync_assertion(&mut self) {
        let should_assert = match self.mode {
            PreventSleepMode::Disabled => false,
            PreventSleepMode::AlwaysOn => true,
            PreventSleepMode::WhenPluggedIn => is_on_ac_power(),
        };

        if should_assert && self.assertion_id == 0 {
            self.assertion_id = create_assertion();
        } else if !should_assert && self.assertion_id != 0 {
            release_assertion(self.assertion_id);
            self.assertion_id = 0;
        }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.mode) {
            if let Some(parent) = self.config_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    tracing::warn!(error = %e, "创建电源配置目录失败");
                }
            }
            if let Err(e) = fs::write(&self.config_path, json) {
                tracing::warn!(error = %e, "保存电源配置失败");
            }
        }
    }
}

impl Drop for PowerManager {
    fn drop(&mut self) {
        if self.assertion_id != 0 {
            release_assertion(self.assertion_id);
        }
    }
}

// ── macOS IOKit FFI ──

#[cfg(target_os = "macos")]
fn create_assertion() -> u32 {
    use std::ffi::CString;

    let type_str = CString::new("PreventUserIdleSystemSleep").unwrap();
    let name_str = CString::new("Qingqi Prevent Sleep").unwrap();

    unsafe {
        let type_cf = CFStringCreateWithCString(
            std::ptr::null(),
            type_str.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );
        let name_cf = CFStringCreateWithCString(
            std::ptr::null(),
            name_str.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );

        let mut id: u32 = 0;
        let ret = IOPMAssertionCreateWithName(type_cf, K_IOPM_ASSERTION_LEVEL_ON, name_cf, &mut id);

        CFRelease(type_cf);
        CFRelease(name_cf);

        if ret == 0 { id } else { 0 }
    }
}

#[cfg(target_os = "macos")]
fn release_assertion(id: u32) {
    if id == 0 {
        return;
    }
    unsafe {
        IOPMAssertionRelease(id);
    }
}

#[cfg(target_os = "macos")]
fn is_on_ac_power() -> bool {
    use std::ffi::CString;

    unsafe {
        let blob = IOPSCopyPowerSourcesInfo();
        if blob.is_null() {
            return true; // can't determine — assume AC
        }

        let list = IOPSCopyPowerSourcesList(blob);
        if list.is_null() {
            CFRelease(blob);
            return true;
        }

        let state_key_c = CString::new("Power Source State").unwrap();
        let ac_value_c = CString::new("AC Power").unwrap();
        let state_key = CFStringCreateWithCString(
            std::ptr::null(),
            state_key_c.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );
        let ac_value = CFStringCreateWithCString(
            std::ptr::null(),
            ac_value_c.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );

        let count = CFArrayGetCount(list);
        let mut on_ac = false;

        for i in 0..count {
            let ps = CFArrayGetValueAtIndex(list, i);
            if ps.is_null() {
                continue;
            }
            let desc = IOPSGetPowerSourceDescription(blob, ps);
            if desc.is_null() {
                continue;
            }
            let state = CFDictionaryGetValue(desc, state_key);
            if !state.is_null() && CFEqual(state, ac_value) {
                on_ac = true;
                break;
            }
        }

        CFRelease(state_key);
        CFRelease(ac_value);
        CFRelease(list);
        CFRelease(blob);
        on_ac
    }
}

#[cfg(not(target_os = "macos"))]
fn create_assertion() -> u32 {
    0
}

#[cfg(not(target_os = "macos"))]
fn release_assertion(_id: u32) {}

#[cfg(not(target_os = "macos"))]
fn is_on_ac_power() -> bool {
    true
}

// ── FFI declarations ──

#[cfg(target_os = "macos")]
const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;

#[cfg(target_os = "macos")]
const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

#[cfg(target_os = "macos")]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        assertion_type: *const std::ffi::c_void,
        assertion_level: u32,
        assertion_name: *const std::ffi::c_void,
        assertion_id: *mut u32,
    ) -> i32;

    fn IOPMAssertionRelease(assertion_id: u32) -> i32;

    fn IOPSCopyPowerSourcesInfo() -> *const std::ffi::c_void;

    fn IOPSCopyPowerSourcesList(blob: *const std::ffi::c_void) -> *const std::ffi::c_void;

    fn IOPSGetPowerSourceDescription(
        blob: *const std::ffi::c_void,
        ps: *const std::ffi::c_void,
    ) -> *const std::ffi::c_void;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        alloc: *const std::ffi::c_void,
        c_str: *const i8,
        encoding: u32,
    ) -> *const std::ffi::c_void;

    fn CFRelease(cf: *const std::ffi::c_void);

    fn CFArrayGetCount(array: *const std::ffi::c_void) -> i64;

    fn CFArrayGetValueAtIndex(
        array: *const std::ffi::c_void,
        index: i64,
    ) -> *const std::ffi::c_void;

    fn CFDictionaryGetValue(
        dict: *const std::ffi::c_void,
        key: *const std::ffi::c_void,
    ) -> *const std::ffi::c_void;

    fn CFEqual(cf1: *const std::ffi::c_void, cf2: *const std::ffi::c_void) -> bool;
}
