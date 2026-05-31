use std::sync::{Arc, Mutex};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, store::CaptureStore, view::CaptureView};
use qingqi_plugin::{
    command::{Command, ContextKind, ContextMatcher},
    database::DatabaseService,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

pub struct HttpCapturePlugin {
    store: Arc<Mutex<CaptureStore>>,
}

impl HttpCapturePlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        let _ = paths;
        let store = CaptureStore::open(
            database,
            &qingqi_plugin::database::feature_database_key(manifest::PLUGIN_ID, "capture"),
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
    fn commands(&self, _query: &str) -> Vec<Command> {
        let manifest = self.manifest();
        vec![
            Command::plugin_open(
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
        let view = cx.app.new(|cx| CaptureView::new(store, cx));
        Ok(PluginView::Window(Box::new(HttpCaptureView { view })))
    }

    fn close_idle(&mut self) {}
}

struct HttpCaptureView {
    view: Entity<CaptureView>,
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
