use std::sync::{Arc, Mutex};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        database::{DatabaseService, DatabaseSpec},
        plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
        storage::AppPaths,
    },
    features::http_capture::{manifest, store::CaptureStore, view::CapturePanel},
};

pub struct HttpCapturePlugin {
    store: Arc<Mutex<CaptureStore>>,
}

impl HttpCapturePlugin {
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

impl Plugin for HttpCapturePlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn database_specs(&self) -> Vec<DatabaseSpec> {
        vec![DatabaseSpec::feature(
            manifest::PLUGIN_ID,
            "capture",
            "capture.db",
        )]
    }

    fn commands(&self, _query: &str) -> Vec<CommandItem> {
        let manifest = self.manifest();
        vec![
            CommandItem::plugin_open(
                manifest.id.as_ref(),
                manifest.name.as_ref(),
                manifest.description.as_ref(),
                manifest.keywords.iter().map(|s| s.as_ref()),
                manifest.command_prefixes.iter().map(|s| s.as_ref()),
                manifest.icon.as_str(),
            )
            .with_recommend_matchers([ContextMatcher::new(ContextKind::Url, 90)]),
        ]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let store = Arc::clone(&self.store);
        let view = cx.app.new(|cx| CapturePanel::new(store, cx));
        Ok(PluginView::Window(Box::new(HttpCaptureView { view })))
    }

    fn close_idle(&mut self) {}
}

struct HttpCaptureView {
    view: Entity<CapturePanel>,
}

impl WindowView for HttpCaptureView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "HTTP 抓包".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }

    fn on_close(&mut self) {}
}
