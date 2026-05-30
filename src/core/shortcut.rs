use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    str::FromStr,
};

use anyhow::{Context, Result, anyhow};
use global_hotkey::hotkey::HotKey;
use gpui::{Action, App, Global, KeyBinding, SharedString};

use crate::{
    app::window_controller::{WindowController, WindowControllerHandle},
    core::{command::Activation, plugin::PluginManager},
};

pub const CORE_PLUGIN_ID: &str = "core";
pub const OPEN_LAUNCHER_SHORTCUT_ID: &str = "core.open-launcher";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutScope {
    Global,
    App,
}

impl ShortcutScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Global => "全局",
            Self::App => "应用内",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShortcutTarget {
    Command(Activation),
    CoreAction(CoreShortcutAction),
    PluginAction {
        plugin_id: String,
        action_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreShortcutAction {
    ToggleLauncher,
}

#[derive(Clone, Debug)]
pub struct ShortcutDescriptor {
    pub id: String,
    pub owner_plugin_id: String,
    pub title: String,
    pub scope: ShortcutScope,
    pub default_accelerator: String,
    pub current_accelerator: String,
    pub context: Option<String>,
    pub target: ShortcutTarget,
    pub editable: bool,
    pub enabled: bool,
}

impl ShortcutDescriptor {
    pub fn new(
        id: impl Into<String>,
        owner_plugin_id: impl Into<String>,
        title: impl Into<String>,
        scope: ShortcutScope,
        accelerator: impl Into<String>,
        target: ShortcutTarget,
    ) -> Self {
        let accelerator = accelerator.into();
        Self {
            id: id.into(),
            owner_plugin_id: owner_plugin_id.into(),
            title: title.into(),
            scope,
            default_accelerator: accelerator.clone(),
            current_accelerator: accelerator,
            context: None,
            target,
            editable: true,
            enabled: true,
        }
    }

    pub fn with_current_accelerator(mut self, accelerator: impl Into<String>) -> Self {
        self.current_accelerator = accelerator.into();
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Clone, Debug)]
pub struct ShortcutView {
    pub descriptor: ShortcutDescriptor,
    pub normalized_accelerator: Option<String>,
    pub active: bool,
    pub overridden_by: Option<String>,
    pub error: Option<String>,
}

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

#[derive(Default)]
pub struct ShortcutService {
    plugins: Option<Rc<RefCell<PluginManager>>>,
    shortcuts: Vec<ShortcutDescriptor>,
    resolved: Vec<ResolvedShortcut>,
    hotkey_ids: HashMap<u32, String>,
    registration_errors: HashMap<String, String>,
}

impl Global for ShortcutService {}

impl ShortcutService {
    pub fn new(plugins: Rc<RefCell<PluginManager>>) -> Self {
        Self {
            plugins: Some(plugins),
            ..Default::default()
        }
    }

    pub fn reload_from_plugins(&mut self, cx: &mut App) -> Result<()> {
        let mut shortcuts = vec![core_open_launcher_shortcut()];
        if let Some(plugins) = self.plugins.as_ref() {
            // Force all plugin shortcuts to App scope — only core shortcuts
            // (e.g. Alt+Space to toggle launcher) may be global.  This
            // prevents plugins from registering system-wide hotkeys that
            // conflict with other applications.
            shortcuts.extend(plugins.borrow_mut().shortcuts().into_iter().map(|mut s| {
                if s.owner_plugin_id != CORE_PLUGIN_ID {
                    s.scope = ShortcutScope::App;
                }
                s
            }));
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
            return self.rebuild(cx);
        }

        let plugins = self
            .plugins
            .as_ref()
            .ok_or_else(|| anyhow!("plugin manager unavailable"))?;
        plugins
            .borrow_mut()
            .set_shortcut(&owner, id, &normalized, enabled)
            .with_context(|| format!("保存快捷键失败: {id}"))?;
        self.refresh(cx)
    }

    pub fn restore_shortcut(&mut self, id: &str, cx: &mut App) -> Result<()> {
        let shortcut = self
            .shortcuts
            .iter()
            .find(|shortcut| shortcut.id == id)
            .cloned()
            .ok_or_else(|| anyhow!("快捷键不存在: {id}"))?;
        self.set_shortcut(id, &shortcut.default_accelerator, true, cx)
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

        let registration_result = crate::platform::hotkey::register_global_hotkeys(&hotkeys);
        self.hotkey_ids = registration_result
            .registered
            .into_iter()
            .map(|(shortcut_id, hotkey_id)| (hotkey_id, shortcut_id))
            .collect();
        self.registration_errors = registration_result.errors;

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

pub fn normalize_accelerator(text: &str) -> Option<String> {
    let parts = text
        .replace('＋', "+")
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(normalize_part)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    let key = parts.last()?.clone();
    if is_modifier(&key) {
        return None;
    }

    let mut seen = HashSet::new();
    let modifiers = ["Ctrl", "Alt", "Shift", "Win"]
        .into_iter()
        .filter(|modifier| {
            parts[..parts.len().saturating_sub(1)]
                .iter()
                .any(|part| part == modifier)
                && seen.insert(*modifier)
        })
        .map(String::from)
        .collect::<Vec<_>>();
    let normalized = modifiers
        .into_iter()
        .chain(std::iter::once(key))
        .collect::<Vec<_>>()
        .join("+");
    parse_hotkey(&normalized).ok()?;
    Some(normalized)
}

fn normalize_part(part: &str) -> String {
    match part.to_ascii_lowercase().as_str() {
        "control" | "ctrl" => String::from("Ctrl"),
        "alt" | "option" | "opt" => String::from("Alt"),
        "shift" => String::from("Shift"),
        "win" | "meta" | "cmd" | "command" | "super" => String::from("Win"),
        "esc" | "escape" => String::from("Esc"),
        "return" => String::from("Enter"),
        "space" => String::from("Space"),
        "left" => String::from("Left"),
        "right" => String::from("Right"),
        "up" => String::from("Up"),
        "down" => String::from("Down"),
        value if value.len() == 1 => value.to_ascii_uppercase(),
        value => {
            let mut chars = value.chars();
            let head = chars
                .next()
                .map(|ch| ch.to_uppercase().collect::<String>())
                .unwrap_or_default();
            let tail = chars.as_str().to_lowercase();
            format!("{head}{tail}")
        }
    }
}

fn is_modifier(value: &str) -> bool {
    matches!(value, "Ctrl" | "Alt" | "Shift" | "Win")
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
            let activation = Activation::Run(crate::core::command::Action::PluginAction {
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
        assert_eq!(normalize_accelerator("Alt+V+Ctrl"), None);
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
