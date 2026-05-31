use serde::{Deserialize, Serialize};

use crate::command::Activation;

pub const CORE_PLUGIN_ID: &str = "core";
pub const OPEN_LAUNCHER_SHORTCUT_ID: &str = "core.open-launcher";

#[derive(Clone, Debug)]
pub struct ShortcutView {
    pub descriptor: ShortcutDescriptor,
    pub normalized_accelerator: Option<String>,
    pub active: bool,
    pub overridden_by: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ShortcutTarget {
    Command(Activation),
    CoreAction(CoreShortcutAction),
    PluginAction {
        plugin_id: String,
        action_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CoreShortcutAction {
    ToggleLauncher,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

pub fn normalize_accelerator(text: &str) -> Option<String> {
    let normalized = text
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "cmd" | "command" | "win" | "super" => "Win".to_string(),
            "esc" | "escape" => "Escape".to_string(),
            "space" => "Space".to_string(),
            "enter" | "return" => "Enter".to_string(),
            "left" => "Left".to_string(),
            "right" => "Right".to_string(),
            "up" => "Up".to_string(),
            "down" => "Down".to_string(),
            value if value.len() == 1 => value.to_ascii_uppercase(),
            value => {
                let mut chars = value.chars();
                match chars.next() {
                    Some(first) => {
                        let mut result = first.to_ascii_uppercase().to_string();
                        result.push_str(chars.as_str());
                        result
                    }
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>();

    if normalized.len() < 2 {
        return None;
    }

    let mut modifiers = Vec::new();
    let mut key = None;
    for part in normalized {
        match part.as_str() {
            "Ctrl" | "Alt" | "Shift" | "Win" => {
                if !modifiers.iter().any(|existing| existing == &part) {
                    modifiers.push(part);
                }
            }
            _ => {
                if key.is_some() {
                    return None;
                }
                key = Some(part);
            }
        }
    }

    let key = key?;
    modifiers.sort_by_key(|modifier| match modifier.as_str() {
        "Ctrl" => 0,
        "Alt" => 1,
        "Shift" => 2,
        "Win" => 3,
        _ => 9,
    });
    modifiers.push(key);
    Some(modifiers.join("+"))
}
