use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::{AppEventBus, AppEventKind},
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        database::{DatabaseService, DatabaseSpec},
        plugin::{PluginRuntime, PluginSession},
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

impl PluginRuntime for ApiDebuggerRuntime {
    fn manifest(&self) -> crate::core::plugin::PluginManifest {
        manifest::manifest()
    }

    fn database_specs(&self) -> Vec<DatabaseSpec> {
        vec![DatabaseSpec::app("api_debugger/main", "api_debugger.db")]
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
            .with_recommend_matchers([
                ContextMatcher::new(ContextKind::Url, 120),
                ContextMatcher::clipboard(ContextKind::Url, 70),
            ]),
        ]
    }

    fn open_session(
        &mut self,
        events: AppEventBus,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        let service = self.service();
        self.ensure_watcher(Arc::clone(&service), events, cx);
        Ok(Box::new(ApiDebuggerSession {
            panel: Rc::new(RefCell::new(view::ApiDebuggerPanel::new(
                service,
                cx,
            ))),
        }))
    }
}

struct ApiDebuggerSession {
    panel: Rc<RefCell<view::ApiDebuggerPanel>>,
}

impl PluginSession for ApiDebuggerSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "API 调试器"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        view::ApiDebuggerElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }
}
