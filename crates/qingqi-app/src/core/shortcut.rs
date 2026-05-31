use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow};
use global_hotkey::hotkey::HotKey;
use gpui::{Action, App, Global, KeyBinding, SharedString};
use qingqi_plugin::command::{Action as PluginAction, Activation};
pub use qingqi_plugin::shortcut::{
    CORE_PLUGIN_ID, CoreShortcutAction, OPEN_LAUNCHER_SHORTCUT_ID, ShortcutDescriptor,
    ShortcutScope, ShortcutTarget, ShortcutView, normalize_accelerator,
};

use crate::app::window_controller::{WindowController, WindowControllerHandle};
use qingqi_core::plugin::PluginManager;

#[derive(Clone, Debug)]
struct ResolvedShortcut {
    descriptor: ShortcutDescriptor,
    normalized_accelerator: String,
    active: bool,
    overridden_by: Option<String>,
    error: Option<String>,
    hotkey: Option<HotKey>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct ShortcutCollisionKey {
    scope: ShortcutScopeKey,
    context: Option<String>,
    accelerator: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum ShortcutScopeKey {
    Global,
    App,
}

impl From<ShortcutScope> for ShortcutScopeKey {
    fn from(value: ShortcutScope) -> Self {
        match value {
            ShortcutScope::Global => Self::Global,
            ShortcutScope::App => Self::App,
        }
    }
}

#[cfg(target_os = "windows")]
use qingqi_platform::low_level_hook::LowLevelEntry;

#[derive(Default)]
pub struct ShortcutService {
    plugins: Option<Arc<Mutex<PluginManager>>>,
    shortcuts: Vec<ShortcutDescriptor>,
    resolved: Vec<ResolvedShortcut>,
    hotkey_ids: HashMap<u32, String>,
    registration_errors: HashMap<String, String>,
    /// Shortcut ids that are handled via `WH_KEYBOARD_LL` instead of
    /// `RegisterHotKey` (Windows-only — empty on other platforms).
    #[cfg(target_os = "windows")]
    low_level_ids: HashMap<u32, String>,
    /// Low-level hook entries that need to be installed by background.
    #[cfg(target_os = "windows")]
    low_level_entries: Vec<LowLevelEntry>,
}

#[derive(Clone)]
pub struct ShortcutGlobal {
    shared: Arc<Mutex<ShortcutService>>,
}

impl Global for ShortcutGlobal {}

impl ShortcutGlobal {
    pub fn new(shared: Arc<Mutex<ShortcutService>>) -> Self {
        Self { shared }
    }

    pub fn dispatch_global(&self, hotkey_id: u32) -> Option<ShortcutTarget> {
        self.shared
            .lock()
            .ok()
            .and_then(|service| service.dispatch_global(hotkey_id))
    }

    #[cfg(target_os = "windows")]
    pub fn dispatch_low_level(&self, hook_id: u32) -> Option<ShortcutTarget> {
        self.shared
            .lock()
            .ok()
            .and_then(|service| service.dispatch_low_level(hook_id))
    }

    pub fn dispatch_app_action(&self, action: &ShortcutAction) -> Option<ShortcutTarget> {
        self.shared
            .lock()
            .ok()
            .and_then(|service| service.dispatch_app_action(action))
    }
}

impl ShortcutService {
    pub fn new(plugins: Arc<Mutex<PluginManager>>) -> Self {
        Self {
            plugins: Some(plugins),
            ..Default::default()
        }
    }

    pub fn reload_from_plugins(&mut self, cx: &mut App) -> Result<()> {
        let mut shortcuts = vec![core_open_launcher_shortcut()];
        if let Some(plugins) = self.plugins.as_ref() {
            // Respect each plugin's declared shortcut scope. Plugins that
            // explicitly declare Global (e.g. clipboard Alt+V) will be
            // registered as system-wide hotkeys; App-scoped shortcuts are
            // bound as in-window key bindings only.
            shortcuts.extend(
                qingqi_core::lock_or_recover(&plugins, "plugin-manager")
                    .shortcuts()
                    .into_iter(),
            );
        }
        self.replace_shortcuts(shortcuts, cx)
    }

    pub fn replace_shortcuts(
        &mut self,
        shortcuts: Vec<ShortcutDescriptor>,
        cx: &mut App,
    ) -> Result<()> {
        self.shortcuts = shortcuts;
        self.rebuild(cx)
    }

    pub fn refresh(&mut self, cx: &mut App) -> Result<()> {
        self.reload_from_plugins(cx)
    }

    pub fn views(&self) -> Vec<ShortcutView> {
        self.resolved
            .iter()
            .map(|resolved| ShortcutView {
                descriptor: resolved.descriptor.clone(),
                normalized_accelerator: (!resolved.normalized_accelerator.is_empty())
                    .then(|| resolved.normalized_accelerator.clone()),
                active: resolved.active,
                overridden_by: resolved.overridden_by.clone(),
                error: resolved.error.clone(),
            })
            .collect()
    }

    pub fn set_shortcut(
        &mut self,
        id: &str,
        accelerator: &str,
        enabled: bool,
        cx: &mut App,
    ) -> Result<()> {
        self.set_shortcut_internal(id, accelerator, enabled)?;
        self.refresh(cx)
    }

    pub fn set_shortcut_detached(
        &mut self,
        id: &str,
        accelerator: &str,
        enabled: bool,
    ) -> Result<()> {
        self.set_shortcut_internal(id, accelerator, enabled)
    }

    fn set_shortcut_internal(&mut self, id: &str, accelerator: &str, enabled: bool) -> Result<()> {
        let shortcut = self
            .shortcuts
            .iter()
            .find(|shortcut| shortcut.id == id)
            .cloned()
            .ok_or_else(|| anyhow!("快捷键不存在: {id}"))?;
        if !shortcut.editable {
            return Err(anyhow!("快捷键不可编辑: {id}"));
        }
        let owner = shortcut.owner_plugin_id.clone();
        let normalized = if enabled {
            normalize_accelerator(accelerator)
                .ok_or_else(|| anyhow!("快捷键格式无效: {accelerator}"))?
        } else {
            String::new()
        };
        if owner == CORE_PLUGIN_ID {
            if let Some(shortcut) = self.shortcuts.iter_mut().find(|shortcut| shortcut.id == id) {
                shortcut.current_accelerator = normalized;
                shortcut.enabled = enabled;
            }
            return Ok(());
        }

        let plugins = self
            .plugins
            .as_ref()
            .ok_or_else(|| anyhow!("plugin manager unavailable"))?;
        qingqi_core::lock_or_recover(&plugins, "plugin-manager")
            .set_shortcut(&owner, id, &normalized, enabled)
            .with_context(|| format!("保存快捷键失败: {id}"))?;
        Ok(())
    }

    pub fn restore_shortcut(&mut self, id: &str, cx: &mut App) -> Result<()> {
        self.restore_shortcut_detached(id)?;
        self.refresh(cx)
    }

    pub fn restore_shortcut_detached(&mut self, id: &str) -> Result<()> {
        let shortcut = self
            .shortcuts
            .iter()
            .find(|shortcut| shortcut.id == id)
            .cloned()
            .ok_or_else(|| anyhow!("快捷键不存在: {id}"))?;
        self.set_shortcut_internal(id, &shortcut.default_accelerator, true)
    }

    pub fn dispatch_global(&self, hotkey_id: u32) -> Option<ShortcutTarget> {
        let Some(shortcut_id) = self.hotkey_ids.get(&hotkey_id) else {
            return None;
        };
        let Some(resolved) = self
            .resolved
            .iter()
            .find(|shortcut| shortcut.descriptor.id == *shortcut_id && shortcut.active)
        else {
            return None;
        };
        Some(resolved.descriptor.target.clone())
    }

    /// Dispatch a low-level hook event (Windows WH_KEYBOARD_LL) to a shortcut target.
    #[cfg(target_os = "windows")]
    pub fn dispatch_low_level(&self, hook_id: u32) -> Option<ShortcutTarget> {
        let shortcut_id = self.low_level_ids.get(&hook_id)?;
        let Some(resolved) = self
            .resolved
            .iter()
            .find(|shortcut| shortcut.descriptor.id == *shortcut_id && shortcut.active)
        else {
            return None;
        };
        Some(resolved.descriptor.target.clone())
    }

    /// Return the low-level hook entries that background should install.
    #[cfg(target_os = "windows")]
    pub fn low_level_entries(&self) -> &[LowLevelEntry] {
        &self.low_level_entries
    }

    pub fn dispatch_app_action(&self, action: &ShortcutAction) -> Option<ShortcutTarget> {
        let Some(resolved) = self.resolved.iter().find(|shortcut| {
            shortcut.descriptor.id == action.id
                && shortcut.normalized_accelerator == action.accelerator
                && shortcut.active
        }) else {
            return None;
        };
        Some(resolved.descriptor.target.clone())
    }

    fn rebuild(&mut self, cx: &mut App) -> Result<()> {
        self.registration_errors.clear();
        let resolved = resolve_shortcuts(&self.shortcuts);
        let app_bindings = resolved
            .iter()
            .filter(|shortcut| {
                shortcut.active
                    && shortcut.error.is_none()
                    && shortcut.descriptor.scope == ShortcutScope::App
            })
            .filter_map(|shortcut| {
                let keystroke = accelerator_to_gpui_keystroke(&shortcut.normalized_accelerator)?;
                Some(KeyBinding::new(
                    &keystroke,
                    ShortcutAction {
                        id: shortcut.descriptor.id.clone(),
                        accelerator: shortcut.normalized_accelerator.clone(),
                    },
                    shortcut.descriptor.context.as_deref(),
                ))
            })
            .collect::<Vec<_>>();

        if !app_bindings.is_empty() {
            cx.bind_keys(app_bindings);
        }

        let hotkeys = resolved
            .iter()
            .filter(|shortcut| {
                shortcut.active
                    && shortcut.error.is_none()
                    && shortcut.descriptor.scope == ShortcutScope::Global
            })
            .filter_map(|shortcut| {
                shortcut
                    .hotkey
                    .map(|hotkey| (shortcut.descriptor.id.clone(), hotkey))
            })
            .collect::<Vec<_>>();

        let registration_result = qingqi_platform::hotkey::register_global_hotkeys(&hotkeys);
        self.hotkey_ids = registration_result
            .registered
            .into_iter()
            .map(|(shortcut_id, hotkey_id)| (hotkey_id, shortcut_id))
            .collect();
        self.registration_errors = registration_result.errors;

        // On Windows, shortcuts that conflict with system-reserved combos
        // (e.g. Alt+Space) are routed through WH_KEYBOARD_LL instead of
        // being reported as errors.
        #[cfg(target_os = "windows")]
        {
            self.low_level_ids.clear();
            self.low_level_entries.clear();
            for (shortcut_id, entry) in registration_result.low_level_fallbacks {
                self.registration_errors.remove(&shortcut_id);
                self.low_level_ids.insert(entry.id, shortcut_id.clone());
                self.low_level_entries.push(entry);
            }
        }

        self.resolved = resolved
            .into_iter()
            .map(|mut shortcut| {
                if let Some(error) = self.registration_errors.get(&shortcut.descriptor.id) {
                    shortcut.error = Some(error.clone());
                    shortcut.active = false;
                }
                shortcut
            })
            .collect();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ShortcutAction {
    pub id: String,
    pub accelerator: String,
}

impl Action for ShortcutAction {
    fn boxed_clone(&self) -> Box<dyn Action> {
        Box::new(self.clone())
    }

    fn partial_eq(&self, action: &dyn Action) -> bool {
        action
            .as_any()
            .downcast_ref::<ShortcutAction>()
            .is_some_and(|other| other.id == self.id && other.accelerator == self.accelerator)
    }

    fn name(&self) -> &'static str {
        "qingqi::ShortcutAction"
    }

    fn name_for_type() -> &'static str
    where
        Self: Sized,
    {
        "qingqi::ShortcutAction"
    }

    fn build(value: serde_json::Value) -> gpui::Result<Box<dyn Action>>
    where
        Self: Sized,
    {
        let id = value
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let accelerator = value
            .get("accelerator")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        Ok(Box::new(Self { id, accelerator }))
    }
}

pub fn core_open_launcher_shortcut() -> ShortcutDescriptor {
    ShortcutDescriptor::new(
        OPEN_LAUNCHER_SHORTCUT_ID,
        CORE_PLUGIN_ID,
        "打开启动器",
        ShortcutScope::Global,
        "Alt+Space",
        ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher),
    )
    .editable(false)
}

pub fn parse_hotkey(accelerator: &str) -> Result<HotKey> {
    let normalized = normalize_for_global_hotkey(accelerator)?;
    HotKey::from_str(&normalized).map_err(|error| anyhow!(error.to_string()))
}

fn normalize_for_global_hotkey(accelerator: &str) -> Result<String> {
    let normalized = accelerator
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| match part {
            "Ctrl" => "Control".to_string(),
            "Alt" => "Alt".to_string(),
            "Shift" => "Shift".to_string(),
            "Win" => "Super".to_string(),
            "Esc" => "Escape".to_string(),
            "Space" => "Space".to_string(),
            "Enter" => "Enter".to_string(),
            "Left" => "ArrowLeft".to_string(),
            "Right" => "ArrowRight".to_string(),
            "Up" => "ArrowUp".to_string(),
            "Down" => "ArrowDown".to_string(),
            key if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() => {
                format!("Key{}", key.to_ascii_uppercase())
            }
            key if key.len() == 1 && key.as_bytes()[0].is_ascii_digit() => {
                format!("Digit{key}")
            }
            key => key.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+");
    if normalized.is_empty() {
        return Err(anyhow!("empty accelerator"));
    }
    Ok(normalized)
}

pub fn accelerator_to_gpui_keystroke(accelerator: &str) -> Option<String> {
    let normalized = normalize_accelerator(accelerator)?;
    Some(
        normalized
            .split('+')
            .map(|part| match part {
                "Ctrl" => "ctrl".to_string(),
                "Alt" => "alt".to_string(),
                "Shift" => "shift".to_string(),
                "Win" => "cmd".to_string(),
                "Esc" => "escape".to_string(),
                "Space" => "space".to_string(),
                "Enter" => "enter".to_string(),
                "Left" => "left".to_string(),
                "Right" => "right".to_string(),
                "Up" => "up".to_string(),
                "Down" => "down".to_string(),
                key => key.to_ascii_lowercase(),
            })
            .collect::<Vec<_>>()
            .join("-"),
    )
}

fn resolve_shortcuts(shortcuts: &[ShortcutDescriptor]) -> Vec<ResolvedShortcut> {
    let mut resolved = shortcuts
        .iter()
        .cloned()
        .map(|descriptor| {
            let normalized = descriptor
                .enabled
                .then(|| normalize_accelerator(&descriptor.current_accelerator))
                .flatten();
            let hotkey = normalized
                .as_deref()
                .filter(|_| descriptor.scope == ShortcutScope::Global)
                .and_then(|accelerator| parse_hotkey(accelerator).ok());
            ResolvedShortcut {
                descriptor,
                normalized_accelerator: normalized.clone().unwrap_or_default(),
                active: normalized.is_some(),
                overridden_by: None,
                error: None,
                hotkey,
            }
        })
        .collect::<Vec<_>>();

    for shortcut in &mut resolved {
        if !shortcut.descriptor.enabled {
            shortcut.active = false;
            continue;
        }
        if shortcut.normalized_accelerator.is_empty() {
            shortcut.error = Some(String::from("快捷键格式无效"));
            shortcut.active = false;
        }
    }

    let mut winners = HashMap::<ShortcutCollisionKey, usize>::new();
    for (index, shortcut) in resolved.iter().enumerate() {
        if !shortcut.descriptor.enabled || shortcut.error.is_some() {
            continue;
        }
        winners.insert(
            ShortcutCollisionKey {
                scope: shortcut.descriptor.scope.into(),
                context: shortcut.descriptor.context.clone(),
                accelerator: shortcut.normalized_accelerator.clone(),
            },
            index,
        );
    }

    for (index, shortcut) in resolved.clone().iter().enumerate() {
        if !shortcut.descriptor.enabled || shortcut.error.is_some() {
            resolved[index].active = false;
            continue;
        }
        let key = ShortcutCollisionKey {
            scope: shortcut.descriptor.scope.into(),
            context: shortcut.descriptor.context.clone(),
            accelerator: shortcut.normalized_accelerator.clone(),
        };
        if let Some(winner) = winners.get(&key).copied()
            && winner != index
        {
            resolved[index].active = false;
            resolved[index].overridden_by = Some(resolved[winner].descriptor.title.clone());
        }
    }

    resolved
}

pub fn dispatch_target(
    target: &ShortcutTarget,
    window_controller: WindowControllerHandle,
    cx: &mut App,
) {
    match target {
        ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher) => {
            WindowController::toggle_launcher(window_controller, cx);
        }
        ShortcutTarget::Command(target) => {
            WindowController::run_command(window_controller, target.clone(), cx);
        }
        ShortcutTarget::PluginAction {
            plugin_id,
            action_id,
        } => {
            let activation = Activation::Run(PluginAction::PluginAction {
                plugin_id: plugin_id.clone(),
                action_id: action_id.clone(),
                payload: None,
            });
            WindowController::run_command(window_controller, activation, cx);
        }
    }
}

pub fn shortcut_action_json(id: &str) -> Option<SharedString> {
    Some(SharedString::from(format!(r#"{{"id":"{id}"}}"#)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accelerator_orders_modifiers() {
        assert_eq!(
            normalize_accelerator("cmd + shift + v"),
            Some(String::from("Shift+Win+V"))
        );
        assert_eq!(
            normalize_accelerator("ctrl+alt+space"),
            Some(String::from("Ctrl+Alt+Space"))
        );
        assert_eq!(normalize_accelerator("ctrl+alt"), None);
        assert_eq!(
            normalize_accelerator("Alt+V+Ctrl"),
            Some(String::from("Ctrl+Alt+V"))
        );
        assert_eq!(normalize_accelerator("Alt+V+X"), None);
        assert_eq!(normalize_accelerator("Ctrl++"), None);
    }

    #[test]
    fn later_shortcut_overrides_earlier_conflict() {
        let one = ShortcutDescriptor::new(
            "one",
            "a",
            "One",
            ShortcutScope::Global,
            "Alt+V",
            ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher),
        );
        let two = ShortcutDescriptor::new(
            "two",
            "b",
            "Two",
            ShortcutScope::Global,
            "Alt+V",
            ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher),
        );
        let resolved = resolve_shortcuts(&[one, two]);
        assert!(!resolved[0].active);
        assert_eq!(resolved[0].overridden_by.as_deref(), Some("Two"));
        assert!(resolved[1].active);
    }

    #[test]
    fn conflicts_are_scoped_by_context_and_scope() {
        let global = ShortcutDescriptor::new(
            "global",
            "a",
            "Global",
            ShortcutScope::Global,
            "Alt+V",
            ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher),
        );
        let app = ShortcutDescriptor::new(
            "app",
            "b",
            "App",
            ShortcutScope::App,
            "Alt+V",
            ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher),
        );
        let editor = ShortcutDescriptor::new(
            "editor",
            "c",
            "Editor",
            ShortcutScope::App,
            "Alt+V",
            ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher),
        )
        .with_context("Editor");

        let resolved = resolve_shortcuts(&[global, app, editor]);
        assert!(resolved.iter().all(|shortcut| shortcut.active));
    }

    #[test]
    fn disabled_shortcut_is_not_invalid_or_active() {
        let disabled = ShortcutDescriptor::new(
            "disabled",
            "a",
            "Disabled",
            ShortcutScope::Global,
            "Alt+V",
            ShortcutTarget::CoreAction(CoreShortcutAction::ToggleLauncher),
        )
        .with_current_accelerator("")
        .enabled(false);

        let resolved = resolve_shortcuts(&[disabled]);
        assert!(!resolved[0].active);
        assert!(resolved[0].error.is_none());
    }
}
