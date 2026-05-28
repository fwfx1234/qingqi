use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::{AppEventBus, AppEventKind},
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{PluginManifest, PluginRuntime, PluginSession},
        storage::AppPaths,
    },
};

use super::{manifest, service::DownloadService, store::DownloadStore, view};

pub struct DownloadManagerRuntime {
    service: Rc<RefCell<DownloadService>>,
    watch_started: bool,
}

impl DownloadManagerRuntime {
    pub fn new(paths: AppPaths) -> anyhow::Result<Self> {
        let db_path = paths.feature_state(manifest::PLUGIN_ID, "tasks.db");
        let store = DownloadStore::open(&db_path)?;
        let save_dir = paths.feature_output_dir(manifest::PLUGIN_ID);
        let service = DownloadService::new(store, save_dir);
        Ok(Self {
            service: Rc::new(RefCell::new(service)),
            watch_started: false,
        })
    }

    fn ensure_watcher(&mut self, events: AppEventBus, cx: &mut App) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;

        let service = Rc::clone(&self.service);
        cx.spawn(async move |async_cx| {
            let mut revision = service.borrow().revision();
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(400))
                    .await;
                let active_count = service.borrow().active_count();
                let next_revision = service.borrow().revision();
                if active_count > 0 || next_revision != revision {
                    revision = next_revision;
                    events.publish(manifest::PLUGIN_ID, AppEventKind::JobsChanged);
                }
            }
        })
        .detach();
    }
}

impl PluginRuntime for DownloadManagerRuntime {
    fn manifest(&self) -> PluginManifest {
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
            .with_recommend_matchers([
                ContextMatcher::new(ContextKind::Url, 90),
                ContextMatcher::clipboard(ContextKind::Url, 60),
            ]),
        ]
    }

    fn open_session(
        &mut self,
        events: AppEventBus,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        self.ensure_watcher(events, cx);
        let panel = Rc::new(RefCell::new(view::DownloadManagerPanel::new(Rc::clone(
            &self.service,
        ))));
        panel.borrow_mut().init(cx);
        Ok(Box::new(DownloadManagerSession { panel }))
    }

    fn close_idle(&mut self) {}
}

struct DownloadManagerSession {
    panel: Rc<RefCell<view::DownloadManagerPanel>>,
}

impl PluginSession for DownloadManagerSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "下载管理器"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        view::DownloadManagerElement {
            panel: Rc::clone(&self.panel),
        }
        .into_any_element()
    }

    fn on_close(&mut self) {
        self.panel.borrow_mut().cleanup();
    }
}
