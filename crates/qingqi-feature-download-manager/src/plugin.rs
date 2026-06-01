use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Task, Window};

use qingqi_plugin::{
    command::{Command, ContextKind, ContextMatcher},
    database::DatabaseService,
    events::{AppEventBus, AppEventKind},
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

use super::{manifest, service::DownloadService, store::DownloadStore, view};

pub struct DownloadManagerPlugin {
    database: Arc<DatabaseService>,
    paths: AppPaths,
    service: Option<Arc<Mutex<DownloadService>>>,
    watcher_task: Option<Task<()>>,
}

impl DownloadManagerPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            database,
            paths,
            service: None,
            watcher_task: None,
        })
    }

    fn service(&mut self) -> anyhow::Result<Arc<Mutex<DownloadService>>> {
        if let Some(service) = &self.service {
            return Ok(Arc::clone(service));
        }
        let store = DownloadStore::open(
            Arc::clone(&self.database),
            &qingqi_plugin::database::feature_database_key(manifest::PLUGIN_ID, "tasks"),
        )?;
        let save_dir = self.paths.feature_output_dir(manifest::PLUGIN_ID);
        let service = Arc::new(Mutex::new(DownloadService::new(store, save_dir)));
        self.service = Some(Arc::clone(&service));
        Ok(service)
    }

    fn ensure_watcher(
        &mut self,
        service: Arc<Mutex<DownloadService>>,
        events: AppEventBus,
        cx: &mut App,
    ) {
        if self.watcher_task.is_some() {
            return;
        }

        self.watcher_task = Some(cx.spawn(async move |async_cx| {
            let mut revision = service.lock().unwrap().revision();
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(400))
                    .await;
                let (active_count, next_revision) = {
                    let svc = service.lock().unwrap();
                    (svc.active_count(), svc.revision())
                };
                if active_count > 0 || next_revision != revision {
                    revision = next_revision;
                    events.publish(manifest::PLUGIN_ID, AppEventKind::JobsChanged);
                }
            }
        }));
    }
}

impl Plugin for DownloadManagerPlugin {
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
            .with_recommend_matchers([
                ContextMatcher::new(ContextKind::Url, 90),
                ContextMatcher::new(ContextKind::Url, 60),
            ]),
        ]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let service = self.service()?;
        self.ensure_watcher(Arc::clone(&service), cx.events.clone(), cx.app);

        let panel = cx.app.new(|cx| {
            let mut panel = view::DownloadManagerView::new(service);
            panel.init(cx);
            panel
        });

        Ok(PluginView::Window(Box::new(DownloadManagerView { panel })))
    }

    fn close_idle(&mut self) {
        // Stop the job watcher when the window closes. Keep `service` so the
        // same instance is reused on reopen — its `active` map stays consistent
        // and in-flight downloads (whose threads own their own Arc clones)
        // remain visible. Dropping/recreating the service here would orphan the
        // running downloads behind a fresh, empty service.
        self.watcher_task = None;
    }
}

struct DownloadManagerView {
    panel: Entity<view::DownloadManagerView>,
}

impl WindowView for DownloadManagerView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "下载管理器".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.panel.clone().into_any_element()
    }

    fn on_close(&mut self) {}
}
