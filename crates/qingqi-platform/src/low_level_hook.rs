//! Low-level keyboard hook (WH_KEYBOARD_LL) for Windows shortcuts that
//! cannot be registered via `RegisterHotKey` — most notably `Alt+Space`,
//! which the OS reserves for the window system menu.
//!
//! The hook runs on a dedicated thread with its own message pump.  When a
//! registered key combination is detected, the hook sends a u32 event id
//! through a channel so the main loop can dispatch it without blocking the
//! hook callback.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};

use windows::{
    Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
        VK_RWIN, VK_SPACE,
    },
    Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN, MSG,
        PostQuitMessage, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
        WM_SYSKEYDOWN,
    },
};

/// Opaque id assigned via `HotkeyRegistrationResult::low_level_ids`.
pub type LowLevelHotkeyId = u32;

/// A single key-combo description for the low-level hook.
#[derive(Clone, Debug)]
pub struct LowLevelEntry {
    pub id: LowLevelHotkeyId,
    /// The non-modifier virtual-key code (e.g. VK_SPACE).
    pub vk: u32,
    pub require_alt: bool,
    pub require_ctrl: bool,
    pub require_shift: bool,
    pub require_win: bool,
}

impl LowLevelEntry {
    fn matches(&self, vk: u32, alt: bool, ctrl: bool, shift: bool, win: bool) -> bool {
        self.vk == vk
            && self.require_alt == alt
            && self.require_ctrl == ctrl
            && self.require_shift == shift
            && self.require_win == win
    }
}

/// Installed low-level hook state.  Dropping this uninstalls the hook and
/// stops the message-pump thread.
pub struct LowLevelHook {
    thread_quit: Arc<AtomicBool>,
    _thread: std::thread::JoinHandle<()>,
}

impl LowLevelHook {
    /// Install a `WH_KEYBOARD_LL` hook that watches for the given entries
    /// and sends their id through `sender` on a match.  Spawns a dedicated
    /// message-pump thread.
    pub fn install(
        entries: Vec<LowLevelEntry>,
    ) -> Result<(Self, Receiver<LowLevelHotkeyId>), String> {
        let (tx, rx) = mpsc::channel::<LowLevelHotkeyId>();
        let thread_quit = Arc::new(AtomicBool::new(false));
        let quit_flag = Arc::clone(&thread_quit);

        let ready = Arc::new(AtomicBool::new(false));
        let ready_flag = Arc::clone(&ready);
        let error: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
        let error_flag = Arc::clone(&error);

        let thread_quit_for_thread = Arc::clone(&thread_quit);
        let thread = std::thread::Builder::new()
            .name("qingqi-ll-hook".into())
            .spawn(move || {
                let hhook = install_hook(tx, entries);

                let hhook = match hhook {
                    Ok(h) => h,
                    Err(e) => {
                        *error_flag.lock().expect("error_flag mutex should not be poisoned") = Some(e);
                        ready_flag.store(true, Ordering::SeqCst);
                        return;
                    }
                };

                ready_flag.store(true, Ordering::SeqCst);

                // Windows message pump.
                let mut msg = MSG::default();
                loop {
                    if thread_quit_for_thread.load(Ordering::SeqCst) {
                        break;
                    }
                    let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                    if ret.0 == 0 || ret.0 == -1 {
                        break;
                    }
                    unsafe {
                        let _ = DispatchMessageW(&msg);
                    }
                }

                unsafe {
                    let _ = UnhookWindowsHookEx(hhook);
                }
            })
            .map_err(|e| format!("failed to spawn low-level hook thread: {e}"))?;

        // Wait until the hook is installed (or we got an error).
        while !ready.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }

        if let Ok(mut err) = error.lock() {
            if let Some(e) = err.take() {
                return Err(e);
            }
        }

        Ok((
            Self {
                thread_quit: quit_flag,
                _thread: thread,
            },
            rx,
        ))
    }
}

impl Drop for LowLevelHook {
    fn drop(&mut self) {
        self.thread_quit.store(true, Ordering::SeqCst);
        unsafe {
            PostQuitMessage(0);
        }
    }
}

/// Build parsing helpers for `Alt+Space`-style accelerator strings.
pub fn parse_low_level_entry(accelerator: &str, id: LowLevelHotkeyId) -> Option<LowLevelEntry> {
    let parts: Vec<&str> = accelerator
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    let (key, parts) = parts.split_last()?;
    let vk = key_to_vk(key)?;
    if is_modifier_vk(vk) {
        return None;
    }

    let mut alt = false;
    let mut ctrl = false;
    let mut shift = false;
    let mut win = false;
    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "alt" | "option" | "opt" => alt = true,
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "win" | "cmd" | "meta" | "super" => win = true,
            _ => return None,
        }
    }

    Some(LowLevelEntry {
        id,
        vk,
        require_alt: alt,
        require_ctrl: ctrl,
        require_shift: shift,
        require_win: win,
    })
}

fn key_to_vk(key: &str) -> Option<u32> {
    if key.len() == 1 {
        let ch = key.chars().next()?;
        if ch.is_ascii_alphabetic() {
            return Some(ch.to_ascii_uppercase() as u32);
        }
        if ch.is_ascii_digit() {
            return Some(ch as u32);
        }
        return None;
    }

    Some(match key.to_ascii_lowercase().as_str() {
        "space" => VK_SPACE.0 as u32,
        "enter" | "return" => 0x0D,
        "escape" | "esc" => 0x1B,
        "tab" => 0x09,
        "backspace" | "back" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" | "ins" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdn" => 0x22,
        "left" => 0x25,
        "right" => 0x27,
        "up" => 0x26,
        "down" => 0x28,
        "f1" => 0x70,
        "f2" => 0x71,
        "f3" => 0x72,
        "f4" => 0x73,
        "f5" => 0x74,
        "f6" => 0x75,
        "f7" => 0x76,
        "f8" => 0x77,
        "f9" => 0x78,
        "f10" => 0x79,
        "f11" => 0x7A,
        "f12" => 0x7B,
        _ => return None,
    })
}

fn is_modifier_vk(vk: u32) -> bool {
    let vk = vk as u16;
    vk == VK_LSHIFT.0
        || vk == VK_RSHIFT.0
        || vk == VK_LCONTROL.0
        || vk == VK_RCONTROL.0
        || vk == VK_LMENU.0
        || vk == VK_RMENU.0
        || vk == VK_LWIN.0
        || vk == VK_RWIN.0
}

// ── Hook internals ──

struct HookContext {
    entries: Vec<LowLevelEntry>,
    sender: Sender<LowLevelHotkeyId>,
}

static HOOK_CTX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn install_hook(
    tx: Sender<LowLevelHotkeyId>,
    entries: Vec<LowLevelEntry>,
) -> Result<HHOOK, String> {
    let ctx = Box::new(HookContext {
        entries,
        sender: tx,
    });
    let ctx_ptr = Box::into_raw(ctx);

    let hhook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            None,
            // Must be 0: a low-level keyboard hook is global only when not
            // associated with a specific thread.  Passing a thread id would
            // scope it to that thread's windows — and our hook thread has
            // none, so it would capture nothing.
            0,
        )
    }
    .map_err(|e| format!("SetWindowsHookExW(WH_KEYBOARD_LL) failed: {e}"))?;

    if hhook.is_invalid() {
        unsafe {
            let _ = Box::from_raw(ctx_ptr);
        }
        return Err(format!(
            "SetWindowsHookExW returned invalid handle: {}",
            std::io::Error::last_os_error()
        ));
    }

    HOOK_CTX.store(ctx_ptr as usize, Ordering::SeqCst);
    Ok(hhook)
}

unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: windows::Win32::Foundation::WPARAM,
    l_param: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if n_code < 0 {
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }

    let ctx_ptr = HOOK_CTX.load(Ordering::SeqCst) as *const HookContext;
    if ctx_ptr.is_null() {
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }
    // SAFETY: ctx_ptr is valid for the lifetime of the hook.
    let ctx = unsafe { &*ctx_ptr };

    let msg = w_param.0 as u32;
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }

    // SAFETY: l_param is a valid KBDLLHOOKSTRUCT pointer provided by the OS.
    let kb = unsafe { &*(l_param.0 as *const KBDLLHOOKSTRUCT) };
    let vk = kb.vkCode;

    let alt_down = (kb.flags.0 & LLKHF_ALTDOWN.0) != 0;
    let ctrl_down = ctrl_pressed();
    let shift_down = shift_pressed();
    let win_down = win_pressed();

    for entry in &ctx.entries {
        if entry.matches(vk, alt_down, ctrl_down, shift_down, win_down) {
            let _ = ctx.sender.send(entry.id);
            // Return 1 to prevent the OS from processing this keystroke
            // (blocks the system menu for Alt+Space).
            return windows::Win32::Foundation::LRESULT(1);
        }
    }

    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

fn ctrl_pressed() -> bool {
    unsafe {
        (GetKeyState(VK_LCONTROL.0 as i32) as u32 & 0x8000) != 0
            || (GetKeyState(VK_RCONTROL.0 as i32) as u32 & 0x8000) != 0
    }
}

fn shift_pressed() -> bool {
    unsafe {
        (GetKeyState(VK_LSHIFT.0 as i32) as u32 & 0x8000) != 0
            || (GetKeyState(VK_RSHIFT.0 as i32) as u32 & 0x8000) != 0
    }
}

fn win_pressed() -> bool {
    unsafe {
        (GetKeyState(VK_LWIN.0 as i32) as u32 & 0x8000) != 0
            || (GetKeyState(VK_RWIN.0 as i32) as u32 & 0x8000) != 0
    }
}
