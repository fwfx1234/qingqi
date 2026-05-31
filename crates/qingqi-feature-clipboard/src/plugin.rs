use std::sync::{Arc, Mutex};

use anyhow::Result;
use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, service::ClipboardService, view};
use qingqi_plugin::{
    command::{Activation, Command},
    events::AppEventBus,
    host::ShortcutHandleRef,
    plugin::{Plugin, PluginCx, PluginId, PluginView, WindowView},
    shortcut::{ShortcutDescriptor, ShortcutScope, ShortcutTarget},
};

pub struct ClipboardPlugin {
    service: Arc<Mutex<ClipboardService>>,
    shortcut_handle: Option<ShortcutHandleRef>,
}

impl ClipboardPlugin {
    pub fn new(service: ClipboardService) -> Self {
        Self::from_shared(Arc::new(Mutex::new(service)))
    }

    pub fn from_shared(service: Arc<Mutex<ClipboardService>>) -> Self {
        Self {
            service,
            shortcut_handle: None,
        }
    }

    pub fn with_shortcut_handle(mut self, shortcut_handle: Option<ShortcutHandleRef>) -> Self {
        self.shortcut_handle = shortcut_handle;
        self
    }

    pub fn service(&self) -> Arc<Mutex<ClipboardService>> {
        Arc::clone(&self.service)
    }
}

impl Plugin for ClipboardPlugin {
    fn manifest(&self) -> qingqi_plugin::plugin::Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        let manifest = self.manifest();
        vec![Command::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.prefixes.iter().map(|s| s.as_ref()),
            manifest.icon.as_str(),
        )]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> Result<PluginView> {
        if let Ok(service) = self.service.lock() {
            let _ = service.capture_current(cx.app);
        }

        let panel = cx.app.new(|cx| {
            let mut panel = view::ClipboardView::with_shortcut_handle(
                Arc::clone(&self.service),
                self.shortcut_handle.clone(),
            );
            panel.init(cx);
            panel
        });

        panel.update(cx.app, |panel, cx| {
            panel.refresh_async(cx);
        });

        Ok(PluginView::Window(Box::new(ClipboardView { panel })))
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
                manifest.id.as_ref(),
                "剪贴板历史",
                ShortcutScope::Global,
                "Alt+V",
                ShortcutTarget::Command(Activation::Open {
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
        ClipboardService::start_background(Arc::clone(&self.service), cx);
    }

    fn close_idle(&mut self) {}
}

struct ClipboardView {
    panel: Entity<view::ClipboardView>,
}

impl WindowView for ClipboardView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "剪贴板历史".into()
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
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
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
        let path = temp_db("clipboard-plugin.db");
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "clipboard/history",
                path.clone(),
            ))
            .unwrap();
        let service = ClipboardService::new(database, path);
        service
            .set_hotkey(String::from("Ctrl+Alt+V"))
            .expect("legacy hotkey should persist");
        let mut runtime = ClipboardPlugin::new(service);

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
