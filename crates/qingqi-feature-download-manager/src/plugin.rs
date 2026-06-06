use std::sync::{Arc, Mutex};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use qingqi_plugin::{
    command::{ClipboardPayload, Command, ContextKind, ContextMatcher},
    database::DatabaseService,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

use super::{manifest, service::DownloadService, store::DownloadStore, view};

pub struct DownloadManagerPlugin {
    database: Arc<DatabaseService>,
    paths: AppPaths,
    service: Option<Arc<Mutex<DownloadService>>>,
}

impl DownloadManagerPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            database,
            paths,
            service: None,
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

    fn clipboard_boost(&self, payload: &ClipboardPayload) -> Option<i32> {
        let text_has_url = payload
            .text
            .as_deref()
            .is_some_and(|text| !super::model::extract_urls_from_text(text).is_empty());
        if text_has_url {
            return Some(160);
        }

        let file_list_has_url = payload.file_paths.as_ref().is_some_and(|paths| {
            paths.iter().any(|path| {
                path.starts_with("http://")
                    || path.starts_with("https://")
                    || !super::model::extract_urls_from_text(path).is_empty()
            })
        });
        file_list_has_url.then_some(120)
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let service = self.service()?;

        let panel = cx.app.new(|cx| {
            let mut panel = view::DownloadManagerView::new(service);
            panel.init(cx);
            panel
        });

        Ok(PluginView::Window(Box::new(DownloadManagerView { panel })))
    }

    fn close_idle(&mut self) {
        // Keep `service` so in-flight downloads remain visible on reopen.
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

    fn on_input_changed(&mut self, text: &str, cx: &mut App) {
        self.panel
            .update(cx, |panel, cx| panel.import_launch_input(text, cx));
    }

    fn on_close(&mut self) {}
}
