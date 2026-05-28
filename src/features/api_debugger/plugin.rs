use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use gpui::{AnyElement, App, IntoElement, Window};

use crate::{
    app::events::{AppEventBus, AppEventKind},
    core::{
        command::{CommandItem, ContextKind, ContextMatcher},
        plugin::{PluginRuntime, PluginSession},
        storage::AppPaths,
    },
    features::api_debugger::{manifest, service::ApiService, view},
};

pub struct ApiDebuggerRuntime {
    service: Arc<ApiService>,
    watch_started: bool,
}

impl ApiDebuggerRuntime {
    pub fn new(paths: AppPaths) -> Self {
        Self {
            service: Arc::new(ApiService::new(paths)),
            watch_started: false,
        }
    }

    fn ensure_watcher(&mut self, events: AppEventBus, cx: &mut App) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;

        let service = Arc::clone(&self.service);
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
        Self::new(paths)
    }
}

impl PluginRuntime for ApiDebuggerRuntime {
    fn manifest(&self) -> crate::core::plugin::PluginManifest {
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
        self.ensure_watcher(events, cx);
        Ok(Box::new(ApiDebuggerSession {
            panel: Rc::new(RefCell::new(view::ApiDebuggerPanel::new(
                Arc::clone(&self.service),
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
