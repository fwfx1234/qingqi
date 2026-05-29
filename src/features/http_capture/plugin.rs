use std::sync::{Arc, Mutex};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        database::{DatabaseService, DatabaseSpec},
        plugin::{PluginManifest, PluginRuntime, PluginSession},
        storage::AppPaths,
    },
    features::http_capture::{manifest, store::CaptureStore, view::CapturePanel},
};

pub struct HttpCaptureRuntime {
    store: Arc<Mutex<CaptureStore>>,
}

impl HttpCaptureRuntime {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        let _ = paths;
        let store = CaptureStore::open(
            database,
            &crate::core::database::feature_database_key(manifest::PLUGIN_ID, "capture"),
        )?;
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
        })
    }
}

impl PluginRuntime for HttpCaptureRuntime {
    fn manifest(&self) -> PluginManifest {
        manifest::manifest()
    }

    fn database_specs(&self) -> Vec<DatabaseSpec> {
        vec![DatabaseSpec::feature(
            manifest::PLUGIN_ID,
            "capture",
            "capture.db",
        )]
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
            .with_recommend_matchers([ContextMatcher::new(ContextKind::Url, 90)]),
        ]
    }

    fn open_session(
        &mut self,
        _events: AppEventBus,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        let store = Arc::clone(&self.store);
        let view = cx.new(|cx| CapturePanel::new(store, cx));
        Ok(Box::new(HttpCaptureSession { view }))
    }

    fn close_idle(&mut self) {}
}

struct HttpCaptureSession {
    view: Entity<CapturePanel>,
}

impl PluginSession for HttpCaptureSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "HTTP 抓包"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }

    fn on_close(&mut self) {}
}
