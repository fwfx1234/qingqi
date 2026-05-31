use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyEventReceiver, GlobalHotKeyManager, hotkey::HotKey,
};

static REGISTERED: OnceLock<Mutex<Vec<HotKey>>> = OnceLock::new();

thread_local! {
    static MANAGER: RefCell<Option<GlobalHotKeyManager>> = const { RefCell::new(None) };
}

pub struct HotkeyRegistrationResult {
    pub registered: HashMap<String, u32>,
    pub errors: HashMap<String, String>,
    /// Shortcuts that failed `RegisterHotKey` but can be retried via
    /// `WH_KEYBOARD_LL` (e.g. `Alt+Space` on Windows, which the OS
    /// reserves for the system menu).
    #[cfg(target_os = "windows")]
    pub low_level_fallbacks: Vec<(String, crate::low_level_hook::LowLevelEntry)>,
}

pub fn register_global_hotkeys(registrations: &[(String, HotKey)]) -> HotkeyRegistrationResult {
    let mut result = HotkeyRegistrationResult {
        registered: HashMap::new(),
        errors: HashMap::new(),
        #[cfg(target_os = "windows")]
        low_level_fallbacks: Vec::new(),
    };
    let manager_error = MANAGER
        .with(|manager| {
            let mut manager = manager.borrow_mut();
            if manager.is_none() {
                *manager = Some(GlobalHotKeyManager::new().map_err(|error| error.to_string())?);
            }
            let manager = manager
                .as_ref()
                .expect("global hotkey manager should be initialized");

            if let Ok(mut registered) = registered_hotkeys().lock() {
                if !registered.is_empty()
                    && let Err(error) = manager
                        .unregister_all(&registered)
                        .map_err(|error| error.to_string())
                {
                    tracing::warn!(error, "global hotkey unregister failed");
                }
                registered.clear();

                for (shortcut_id, hotkey) in registrations {
                    // Some combos (e.g. Alt+Space) are reserved by the OS for the
                    // window system menu.  `RegisterHotKey` may report success but the
                    // system menu still swallows the keystroke, so route them through
                    // the low-level keyboard hook unconditionally rather than waiting
                    // for a registration error that never comes.
                    #[cfg(target_os = "windows")]
                    if needs_low_level_hook(hotkey)
                        && let Some(entry) = try_as_low_level_entry(shortcut_id, hotkey)
                    {
                        tracing::debug!(
                            shortcut_id,
                            "routing system-reserved hotkey through low-level hook"
                        );
                        result
                            .low_level_fallbacks
                            .push((shortcut_id.clone(), entry));
                        continue;
                    }

                    match manager.register(*hotkey).map_err(|error| error.to_string()) {
                        Ok(()) => {
                            registered.push(*hotkey);
                            result.registered.insert(shortcut_id.clone(), hotkey.id());
                        }
                        Err(error) => {
                            #[cfg(target_os = "windows")]
                            {
                                // On Windows, Alt+Space is reserved by the OS — try
                                // to parse it as a low-level hook fallback instead
                                // of reporting a hard error.
                                if let Some(entry) = try_as_low_level_entry(shortcut_id, hotkey) {
                                    tracing::debug!(
                                        shortcut_id,
                                        error = %error,
                                        "global hotkey failed, routing through low-level hook"
                                    );
                                    result
                                        .low_level_fallbacks
                                        .push((shortcut_id.clone(), entry));
                                    continue;
                                }
                            }
                            let _ = shortcut_id;
                            let _ = hotkey;
                            result.errors.insert(shortcut_id.clone(), error);
                        }
                    }
                }
            } else {
                for (shortcut_id, _) in registrations {
                    result.errors.insert(
                        shortcut_id.clone(),
                        String::from("global hotkey registry lock poisoned"),
                    );
                }
            }

            Ok::<(), String>(())
        })
        .err();
    if let Some(error) = manager_error {
        for (shortcut_id, _) in registrations {
            result.errors.insert(shortcut_id.clone(), error.clone());
        }
    }

    result
}

#[cfg(target_os = "windows")]
fn needs_low_level_hook(hotkey: &HotKey) -> bool {
    use global_hotkey::hotkey::Code;
    // Alt+Space opens the window system menu — `RegisterHotKey` can't reliably
    // claim it, so it must go through `WH_KEYBOARD_LL`.
    let mods = hotkey.mods;
    hotkey.key == Code::Space && mods.alt() && !mods.ctrl() && !mods.shift() && !mods.meta()
}

#[cfg(target_os = "windows")]
fn try_as_low_level_entry(
    shortcut_id: &str,
    hotkey: &HotKey,
) -> Option<crate::low_level_hook::LowLevelEntry> {
    use crate::low_level_hook::parse_low_level_entry;

    // Convert the `global-hotkey` crate's HotKey back into a string the
    // low-level hook parser understands.  We only support the subset of
    // combinations that need this fallback (Alt+Space and similar).
    let mut parts = Vec::new();
    let mods = hotkey.mods;
    if mods.ctrl() {
        parts.push("Ctrl");
    }
    if mods.alt() {
        parts.push("Alt");
    }
    if mods.shift() {
        parts.push("Shift");
    }
    if mods.meta() {
        parts.push("Win");
    }
    // Map the VK code back to a key name.
    let vk = code_to_vk(&hotkey.key);
    let key_name = vk_to_pretty_key(vk)?;
    parts.push(&key_name);
    let accelerator = parts.join("+");

    parse_low_level_entry(&accelerator, 0) // id assigned later by the service
        .map(|mut entry| {
            // Use a deterministic id derived from the shortcut_id hash.
            entry.id = shortcut_id_to_hook_id(shortcut_id);
            entry
        })
}

#[cfg(target_os = "windows")]
fn code_to_vk(code: &global_hotkey::hotkey::Code) -> u32 {
    use global_hotkey::hotkey::Code;
    match *code {
        Code::Space => 0x20,
        Code::Enter => 0x0D,
        Code::Escape => 0x1B,
        Code::Tab => 0x09,
        Code::Backspace => 0x08,
        Code::Delete => 0x2E,
        Code::Insert => 0x2D,
        Code::Home => 0x24,
        Code::End => 0x23,
        Code::PageUp => 0x21,
        Code::PageDown => 0x22,
        Code::ArrowLeft => 0x25,
        Code::ArrowRight => 0x27,
        Code::ArrowUp => 0x26,
        Code::ArrowDown => 0x28,
        Code::F1 => 0x70,
        Code::F2 => 0x71,
        Code::F3 => 0x72,
        Code::F4 => 0x73,
        Code::F5 => 0x74,
        Code::F6 => 0x75,
        Code::F7 => 0x76,
        Code::F8 => 0x77,
        Code::F9 => 0x78,
        Code::F10 => 0x79,
        Code::F11 => 0x7A,
        Code::F12 => 0x7B,
        Code::Digit0 => 0x30,
        Code::Digit1 => 0x31,
        Code::Digit2 => 0x32,
        Code::Digit3 => 0x33,
        Code::Digit4 => 0x34,
        Code::Digit5 => 0x35,
        Code::Digit6 => 0x36,
        Code::Digit7 => 0x37,
        Code::Digit8 => 0x38,
        Code::Digit9 => 0x39,
        Code::KeyA => 0x41,
        Code::KeyB => 0x42,
        Code::KeyC => 0x43,
        Code::KeyD => 0x44,
        Code::KeyE => 0x45,
        Code::KeyF => 0x46,
        Code::KeyG => 0x47,
        Code::KeyH => 0x48,
        Code::KeyI => 0x49,
        Code::KeyJ => 0x4A,
        Code::KeyK => 0x4B,
        Code::KeyL => 0x4C,
        Code::KeyM => 0x4D,
        Code::KeyN => 0x4E,
        Code::KeyO => 0x4F,
        Code::KeyP => 0x50,
        Code::KeyQ => 0x51,
        Code::KeyR => 0x52,
        Code::KeyS => 0x53,
        Code::KeyT => 0x54,
        Code::KeyU => 0x55,
        Code::KeyV => 0x56,
        Code::KeyW => 0x57,
        Code::KeyX => 0x58,
        Code::KeyY => 0x59,
        Code::KeyZ => 0x5A,
        _ => return 0,
    }
}

#[cfg(target_os = "windows")]
fn shortcut_id_to_hook_id(shortcut_id: &str) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    shortcut_id.hash(&mut hasher);
    // Map to 1..u32::MAX so 0 is never a valid id.
    (hasher.finish() as u32).max(1)
}

#[cfg(target_os = "windows")]
fn vk_to_pretty_key(vk: u32) -> Option<String> {
    Some(match vk {
        0x20 => "Space".into(),
        0x0D => "Enter".into(),
        0x1B => "Escape".into(),
        0x09 => "Tab".into(),
        0x08 => "Backspace".into(),
        0x2E => "Delete".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x25 => "Left".into(),
        0x27 => "Right".into(),
        0x26 => "Up".into(),
        0x28 => "Down".into(),
        0x70..=0x7B => format!("F{}", vk - 0x70 + 1),
        c if c >= 0x30 && c <= 0x39 => {
            // Digit
            char::from_u32(c)?.to_string()
        }
        c if c >= 0x41 && c <= 0x5A => {
            // Letter
            char::from_u32(c)?.to_string()
        }
        _ => return None,
    })
}

pub fn event_receiver() -> GlobalHotKeyEventReceiver {
    GlobalHotKeyEvent::receiver().clone()
}

fn registered_hotkeys() -> &'static Mutex<Vec<HotKey>> {
    REGISTERED.get_or_init(|| Mutex::new(Vec::new()))
}
