use std::sync::{Arc, Mutex};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{
    certificate::CaManager, engine::CaptureEngine, manifest, mock_engine::MockEngine,
    mock_store::MockStore, store::CaptureStore, view::CaptureView,
};
use qingqi_plugin::{
    command::{Command, ContextKind, ContextMatcher},
    database::DatabaseService,
    events::AppEventBus,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

/// HTTP 抓包插件 — 管理代理引擎、Mock 引擎、证书管理器和数据存储。
pub struct HttpCapturePlugin {
    engine: Arc<CaptureEngine>,
    store: Arc<Mutex<CaptureStore>>,
    mock_store: Arc<Mutex<MockStore>>,
    ca_manager: Arc<Mutex<CaManager>>,
    events: AppEventBus,
}

impl HttpCapturePlugin {
    pub fn new(
        database: Arc<DatabaseService>,
        paths: AppPaths,
        events: AppEventBus,
    ) -> anyhow::Result<Self> {
        let store = CaptureStore::open(
            Arc::clone(&database),
            &qingqi_plugin::database::feature_database_key(manifest::PLUGIN_ID, "capture"),
        )?;
        let mock_store = MockStore::open(
            Arc::clone(&database),
            &qingqi_plugin::database::feature_database_key(manifest::PLUGIN_ID, "mock"),
        )?;
        let ca_manager = Arc::new(Mutex::new(CaManager::new(paths)?));

        let store = Arc::new(Mutex::new(store));
        let mock_store = Arc::new(Mutex::new(mock_store));
        let mock_engine = Arc::new(MockEngine::new(Arc::clone(&mock_store)));
        let engine = Arc::new(CaptureEngine::new(
            Arc::clone(&store),
            Arc::clone(&mock_engine),
            Arc::clone(&ca_manager),
            events.clone(),
        ));

        Ok(Self {
            engine,
            store,
            mock_store,
            ca_manager,
            events,
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
        let view = cx.app.new(|cx| {
            CaptureView::new(
                Arc::clone(&self.store),
                Arc::clone(&self.engine),
                Arc::clone(&self.mock_store),
                Arc::clone(&self.ca_manager),
                self.events.clone(),
                cx,
            )
        });
        Ok(PluginView::Window(Box::new(HttpCaptureView { view })))
    }

    /// 懒加载：CA 证书已在 CaManager::new() 中初始化，无需额外后台任务。
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
