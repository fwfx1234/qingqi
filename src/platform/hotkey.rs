use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyEventReceiver, GlobalHotKeyManager, hotkey::HotKey,
};

static REGISTERED: OnceLock<Mutex<Vec<HotKey>>> = OnceLock::new();

pub struct HotkeyRegistrationResult {
    pub registered: HashMap<String, u32>,
    pub errors: HashMap<String, String>,
}

pub fn register_global_hotkeys(registrations: &[(String, HotKey)]) -> HotkeyRegistrationResult {
    let mut result = HotkeyRegistrationResult {
        registered: HashMap::new(),
        errors: HashMap::new(),
    };
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            for (shortcut_id, _) in registrations {
                result.errors.insert(shortcut_id.clone(), error.to_string());
            }
            return result;
        }
    };

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
            match manager.register(*hotkey).map_err(|error| error.to_string()) {
                Ok(()) => {
                    registered.push(*hotkey);
                    result.registered.insert(shortcut_id.clone(), hotkey.id());
                }
                Err(error) => {
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

    result
}

pub fn event_receiver() -> GlobalHotKeyEventReceiver {
    GlobalHotKeyEvent::receiver().clone()
}

fn registered_hotkeys() -> &'static Mutex<Vec<HotKey>> {
    REGISTERED.get_or_init(|| Mutex::new(Vec::new()))
}
