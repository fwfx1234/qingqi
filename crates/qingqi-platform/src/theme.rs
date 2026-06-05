//! macOS 系统主题检测，使用 ObjC FFI 替代 `defaults` 命令。
//!
//! - [`read_system_dark`] 通过 `NSUserDefaults` 直接读取 `AppleInterfaceStyle`，
//!   无子进程开销。
//! - [`ThemeChangeListener`] 通过 `CFNotificationCenter` 监听
//!   `AppleInterfaceThemeChangedNotification`，实现事件驱动，无需轮询。

use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, channel},
};

/// 读取 macOS 系统深浅色模式。
///
/// 使用 `NSUserDefaults` 直接查询 `AppleInterfaceStyle` key，替代
/// `Command::new("defaults").args(["read", "-g", "AppleInterfaceStyle"])`。
pub fn read_system_dark() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::read_system_dark()
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// 主题变化监听器。
///
/// 通过 `CFNotificationCenter` 监听 `AppleInterfaceThemeChangedNotification`。
/// 当用户在系统设置中切换深色/浅色模式时，`receiver` 会收到通知。
///
/// ```ignore
/// let listener = qingqi_platform::theme::ThemeChangeListener::new();
/// let rx = listener.receiver();
/// // 在异步循环中 await rx.recv()
/// ```
pub struct ThemeChangeListener {
    receiver: Arc<Mutex<Receiver<()>>>,
    /// macOS 上持有 observer 引用以防止被提前释放。
    #[cfg(target_os = "macos")]
    _observer: macos::ThemeObserver,
    /// 非 macOS 上 receiver 永远不会收到消息。
    #[cfg(not(target_os = "macos"))]
    _marker: (),
}

impl ThemeChangeListener {
    /// 创建监听器并注册系统主题变化通知。
    ///
    /// 在非 macOS 平台上返回一个永远不会收到消息的监听器。
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            let (tx, rx) = channel();
            let observer = macos::ThemeObserver::new(tx);
            Self {
                receiver: Arc::new(Mutex::new(rx)),
                _observer: observer,
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let (_tx, rx) = channel::<()>();
            Self {
                receiver: Arc::new(Mutex::new(rx)),
                _marker: (),
            }
        }
    }

    /// 获取接收端的 Arc 句柄。在 GPUI 后台任务中通过 `rx.lock().recv()` 等待主题变化。
    pub fn receiver(&self) -> Arc<Mutex<Receiver<()>>> {
        Arc::clone(&self.receiver)
    }
}

impl Default for ThemeChangeListener {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// macOS 实现
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        ffi::c_void,
        sync::{Arc, Mutex, mpsc::Sender},
    };

    // ---- CoreFoundation FFI ------------------------------------------------

    type CFStringRef = *const c_void;
    type CFNotificationCenterRef = *const c_void;
    type CFStringEncoding = u32;

    const K_CF_STRING_ENCODING_UTF8: CFStringEncoding = 0x0800_0100;

    /// `CFComparisonResult::kCFCompareEqualTo`
    const K_CF_COMPARE_EQUAL_TO: i32 = 0;

    type CFNotificationCallback = unsafe extern "C" fn(
        center: CFNotificationCenterRef,
        observer: *const c_void,
        name: CFStringRef,
        object: *const c_void,
        user_info: *const c_void, // CFDictionaryRef
    );

    /// `CFNotificationSuspensionBehavior::CFNotificationSuspensionBehaviorDeliverImmediately`
    const SUSPENSION_BEHAVIOR_DELIVER_IMMEDIATELY: isize = 4;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        /// `kCFPreferencesAnyApplication` — 读取全局偏好域。
        static kCFPreferencesAnyApplication: *const c_void;

        fn CFStringCreateWithCString(
            allocator: *const c_void,
            c_str: *const std::ffi::c_char,
            encoding: CFStringEncoding,
        ) -> CFStringRef;

        fn CFRelease(cf: *const c_void);

        fn CFStringCompare(str1: CFStringRef, str2: CFStringRef, options: i32) -> i32;

        fn CFPreferencesCopyAppValue(key: CFStringRef, application_id: CFStringRef) -> CFStringRef;

        fn CFNotificationCenterGetDistributedCenter() -> CFNotificationCenterRef;

        fn CFNotificationCenterAddObserver(
            center: CFNotificationCenterRef,
            observer: *const c_void,
            callback: CFNotificationCallback,
            name: CFStringRef,
            object: CFStringRef,
            suspension_behavior: isize,
        );

        fn CFNotificationCenterRemoveObserver(
            center: CFNotificationCenterRef,
            observer: *const c_void,
            name: CFStringRef,
            object: CFStringRef,
        );
    }

    /// 创建一个临时的 CFStringRef，调用者负责 CFRelease。
    unsafe fn cfstr(s: &str) -> CFStringRef {
        let c_str = std::ffi::CString::new(s).expect("CString::new failed");
        unsafe {
            CFStringCreateWithCString(std::ptr::null(), c_str.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        }
    }

    /// 读取 macOS 系统深色模式偏好。
    ///
    /// 使用 CoreFoundation 的 `CFPreferencesCopyAppValue` 配合
    /// `kCFPreferencesAnyApplication` 查询全局域，等价于
    /// `defaults read -g AppleInterfaceStyle`。
    pub(super) fn read_system_dark() -> bool {
        unsafe {
            let key = cfstr("AppleInterfaceStyle");
            // kCFPreferencesAnyApplication → 从全局偏好域读取
            let value = CFPreferencesCopyAppValue(key, kCFPreferencesAnyApplication);
            CFRelease(key);

            if value.is_null() {
                return false; // key 不存在 → 浅色模式
            }

            let dark = cfstr("Dark");
            let result = CFStringCompare(value as CFStringRef, dark, 0);
            CFRelease(dark);
            CFRelease(value);

            result == K_CF_COMPARE_EQUAL_TO
        }
    }

    // ---- 主题变化观察者 ----------------------------------------------------

    /// 持有 CFNotificationCenter observer 的 RAII 句柄。
    /// Drop 时自动移除观察者。
    pub(super) struct ThemeObserver {
        sender: Arc<Mutex<Option<Sender<()>>>>,
        observer_ptr: *const c_void, // self 的指针，作为 observer token
    }

    // ThemeObserver 被 ThemeChangeListener 持有，后者按值传递是安全的。
    unsafe impl Send for ThemeObserver {}

    impl ThemeObserver {
        pub(super) fn new(tx: Sender<()>) -> Self {
            let sender = Arc::new(Mutex::new(Some(tx)));

            unsafe {
                let center = CFNotificationCenterGetDistributedCenter();
                assert!(
                    !center.is_null(),
                    "CFNotificationCenterGetDistributedCenter returned NULL"
                );

                let name = cfstr("AppleInterfaceThemeChangedNotification");

                // 将 sender Arc 的内部指针作为 observer token 传入，
                // 回调中通过该指针访问 sender。
                // Arc::into_raw 返回 *const T，泄漏 Arc 本身但保留内部数据引用。
                let sender_raw = Arc::into_raw(Arc::clone(&sender));

                CFNotificationCenterAddObserver(
                    center,
                    sender_raw as *const c_void,
                    theme_change_callback,
                    name,
                    std::ptr::null(), // object = nil
                    SUSPENSION_BEHAVIOR_DELIVER_IMMEDIATELY,
                );

                CFRelease(name);

                Self {
                    sender,
                    observer_ptr: sender_raw as *const c_void,
                }
            }
        }
    }

    impl Drop for ThemeObserver {
        fn drop(&mut self) {
            // 先清空 sender，回调将变为空操作
            if let Ok(mut guard) = self.sender.lock() {
                *guard = None;
            }

            unsafe {
                let center = CFNotificationCenterGetDistributedCenter();
                let name = cfstr("AppleInterfaceThemeChangedNotification");
                CFNotificationCenterRemoveObserver(
                    center,
                    self.observer_ptr,
                    name,
                    std::ptr::null(),
                );
                CFRelease(name);

                // 回收 Arc（之前通过 into_raw 泄漏的）
                let _ = Arc::from_raw(self.observer_ptr as *const Mutex<Option<Sender<()>>>);
            }
        }
    }

    /// CFNotificationCenter 回调：系统主题变化时发送通知。
    unsafe extern "C" fn theme_change_callback(
        _center: CFNotificationCenterRef,
        observer: *const c_void,
        _name: CFStringRef,
        _object: *const c_void,
        _user_info: *const c_void,
    ) {
        use std::sync::{Mutex, mpsc::Sender};
        let sender_ptr = observer as *const Mutex<Option<Sender<()>>>;
        unsafe {
            if let Ok(guard) = (*sender_ptr).lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_system_dark_does_not_crash() {
        // 基本健全性：调用不应 panic
        let _ = read_system_dark();
    }

    #[test]
    fn theme_change_listener_creates_and_drops() {
        let listener = ThemeChangeListener::new();
        let rx = listener.receiver();
        // 不应 panic，不应死锁
        drop(rx);
    }
}
