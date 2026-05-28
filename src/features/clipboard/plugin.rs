use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{CommandItem, CommandTarget, ContextKind, ContextMatcher},
        plugin::{PluginRuntime, PluginSession},
        shortcut::{ShortcutDescriptor, ShortcutScope, ShortcutTarget},
    },
    features::clipboard::{manifest, service::ClipboardService, view},
};

pub struct ClipboardRuntime {
    service: Arc<Mutex<ClipboardService>>,
    watch_started: bool,
}

impl ClipboardRuntime {
    pub fn new(service: ClipboardService) -> Self {
        Self {
            service: Arc::new(Mutex::new(service)),
            watch_started: false,
        }
    }

    pub fn service(&self) -> Arc<Mutex<ClipboardService>> {
        Arc::clone(&self.service)
    }
}

impl PluginRuntime for ClipboardRuntime {
    fn manifest(&self) -> crate::core::plugin::PluginManifest {
        manifest::manifest()
    }

    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        vec![
            CommandItem::plugin_open(
                manifest.id,
                manifest.name,
                manifest.description,
                manifest.keywords.iter().copied(),
                manifest.command_prefixes.iter().copied(),
                manifest.visual.icon,
            )
            .with_recommend_matchers([ContextMatcher::clipboard(ContextKind::Clipboard, 30)]),
        ]
    }

    fn open_session(&mut self, _: AppEventBus, cx: &mut App) -> Result<Box<dyn PluginSession>> {
        if let Ok(service) = self.service.lock() {
            let _ = service.capture_current(cx);
        }

        let panel = cx.new(|cx| {
            let mut panel = view::ClipboardPanel::new(Arc::clone(&self.service));
            panel.init(cx);
            panel
        });

        panel.update(cx, |panel, cx| {
            panel.refresh_async(cx);
        });

        Ok(Box::new(ClipboardSession { panel }))
    }

    fn shortcuts(&self) -> Vec<ShortcutDescriptor> {
        let manifest = self.manifest();
        let configured_hotkey = self
            .service
            .lock()
            .ok()
            .map(|service| service.config().hotkey)
            .unwrap_or_else(|| String::from("Alt+V"));
        let enabled = !configured_hotkey.trim().is_empty();
        let hotkey = if enabled {
            configured_hotkey
        } else {
            String::from("Alt+V")
        };
        vec![
            ShortcutDescriptor::new(
                "clipboard.open-history",
                manifest.id,
                "剪贴板历史",
                ShortcutScope::Global,
                "Alt+V",
                ShortcutTarget::Command(CommandTarget::PluginOpen {
                    plugin_id: manifest.id.to_string(),
                }),
            )
            .with_current_accelerator(hotkey)
            .enabled(enabled),
        ]
    }

    fn set_shortcut(
        &mut self,
        shortcut_id: &str,
        accelerator: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        if shortcut_id != "clipboard.open-history" {
            return Ok(());
        }
        let value = if enabled { accelerator } else { "" };
        self.service
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?
            .set_hotkey(value.to_string())?;
        Ok(())
    }

    fn start_background(&mut self, _: AppEventBus, cx: &mut App) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;

        let service = Arc::clone(&self.service);
        cx.spawn(async move |async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(700))
                    .await;
                let service = Arc::clone(&service);
                let _ = async_cx.update(move |cx| {
                    if let Ok(service) = service.lock() {
                        let _ = service.capture_current(cx);
                    }
                });
            }
        })
        .detach();
    }

    fn close_idle(&mut self) {}
}

struct ClipboardSession {
    panel: Entity<view::ClipboardPanel>,
}

impl PluginSession for ClipboardSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "剪贴板历史"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.panel.clone().into_any_element()
    }

    fn on_reopen(&mut self, _window: &mut Window, cx: &mut App) {
        self.panel.update(cx, |panel, cx| panel.reopen(cx));
    }

    fn on_close(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-clipboard-plugin-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    #[test]
    fn shortcut_uses_legacy_hotkey_config_and_persists_back() {
        let service = ClipboardService::new(temp_db("clipboard-plugin.db"));
        service
            .set_hotkey(String::from("Ctrl+Alt+V"))
            .expect("legacy hotkey should persist");
        let mut runtime = ClipboardRuntime::new(service);

        let shortcut = runtime
            .shortcuts()
            .into_iter()
            .find(|shortcut| shortcut.id == "clipboard.open-history")
            .expect("clipboard shortcut should be declared");
        assert_eq!(shortcut.current_accelerator, "Ctrl+Alt+V");
        assert!(shortcut.enabled);

        runtime
            .set_shortcut("clipboard.open-history", "Shift+Win+V", true)
            .expect("core should save shortcut through plugin");
        let shortcut = runtime.shortcuts().remove(0);
        assert_eq!(shortcut.current_accelerator, "Shift+Win+V");
        assert!(shortcut.enabled);

        runtime
            .set_shortcut("clipboard.open-history", "Shift+Win+V", false)
            .expect("core should disable shortcut through plugin");
        let shortcut = runtime.shortcuts().remove(0);
        assert_eq!(shortcut.current_accelerator, "Alt+V");
        assert!(!shortcut.enabled);
    }
}
