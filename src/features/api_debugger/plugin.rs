use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::{AppEventBus, AppEventKind},
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        database::{DatabaseService, DatabaseSpec},
        plugin::{Plugin, PluginCx, PluginView, WindowView},
        storage::AppPaths,
    },
    features::api_debugger::{manifest, service::ApiService, view},
};

pub struct ApiDebuggerRuntime {
    database: Arc<DatabaseService>,
    paths: AppPaths,
    service: Option<Arc<ApiService>>,
    watch_started: bool,
}

impl ApiDebuggerRuntime {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            service: None,
            watch_started: false,
        }
    }

    fn service(&mut self) -> Arc<ApiService> {
        if let Some(service) = &self.service {
            return Arc::clone(service);
        }
        let service = Arc::new(ApiService::new(
            Arc::clone(&self.database),
            self.paths.clone(),
        ));
        self.service = Some(Arc::clone(&service));
        service
    }

    fn ensure_watcher(&mut self, service: Arc<ApiService>, events: AppEventBus, cx: &mut App) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;

        cx.spawn(async move |async_cx| {
            let mut revision = service.revision();
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(180))
                    .await;
                let next_revision = service.revision();
                if next_revision != revision {
                    revision = next_revision;
                    events.publish(manifest::PLUGIN_ID, AppEventKind::FeatureChanged);
                }
            }
        })
        .detach();
    }
}

impl Default for ApiDebuggerRuntime {
    fn default() -> Self {
        let paths = AppPaths::resolve().expect("failed to resolve qingqi data path");
        let database = Arc::new(DatabaseService::new(paths.clone()));
        Self::new(database, paths)
    }
}

impl Plugin for ApiDebuggerRuntime {
    fn manifest(&self) -> crate::core::plugin::PluginManifest {
        manifest::manifest()
    }

    fn database_specs(&self) -> Vec<DatabaseSpec> {
        vec![DatabaseSpec::app("api_debugger/main", "api_debugger.db")]
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
                manifest.visual.icon.as_str(),
            )
            .with_recommend_matchers([
                ContextMatcher::new(ContextKind::Url, 120),
                ContextMatcher::clipboard(ContextKind::Url, 70),
            ]),
        ]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let service = self.service();
        self.ensure_watcher(Arc::clone(&service), cx.events.clone(), cx.app);
        Ok(PluginView::Window(Box::new(ApiDebuggerView {
            panel: Rc::new(RefCell::new(view::ApiDebuggerPanel::new(service, cx.app))),
        })))
    }
}

struct ApiDebuggerView {
    panel: Rc<RefCell<view::ApiDebuggerPanel>>,
}

impl WindowView for ApiDebuggerView {
    fn plugin_id(&self) -> &str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &str {
        "API 调试器"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        view::ApiDebuggerElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }
}
