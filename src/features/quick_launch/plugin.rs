use std::{sync::Arc, time::Duration};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{
    app::events::{AppEventBus, AppEventKind},
    core::{
        command::{CommandInvocation, CommandItem, CommandOutcome, CommandTarget},
        plugin::{PluginRuntime, PluginSession},
        storage::AppPaths,
    },
    features::quick_launch::{manifest, service::QuickLaunchService, view::QuickLaunchView},
};

pub struct QuickLaunchRuntime {
    service: Arc<QuickLaunchService>,
    watch_started: bool,
}

impl QuickLaunchRuntime {
    pub fn new(paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            service: Arc::new(QuickLaunchService::new(paths)?),
            watch_started: false,
        })
    }
}

impl PluginRuntime for QuickLaunchRuntime {
    fn manifest(&self) -> crate::core::plugin::PluginManifest {
        manifest::manifest()
    }

    fn open_session(
        &mut self,
        _: AppEventBus,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        Ok(Box::new(QuickLaunchSession {
            view: cx.new(|cx| QuickLaunchView::new(Arc::clone(&self.service), cx)),
        }))
    }

    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        let mut commands = vec![CommandItem::plugin_open(
            manifest.id,
            manifest.name,
            manifest.description,
            manifest.keywords.iter().copied(),
            manifest.command_prefixes.iter().copied(),
            manifest.visual.icon,
        )];
        let actions = self
            .service
            .list_actions("", Some(true))
            .unwrap_or_default();
        commands.extend(actions.into_iter().map(|action| {
            CommandItem::plugin_action(
                manifest.id,
                format!("action-{}", action.id),
                action.name.clone(),
                action.description.clone(),
                action.command_keywords(),
                ["ql", "quick"],
                manifest.visual.icon,
                Some(action.id.to_string()),
            )
        }));
        commands
    }

    fn handle_command(
        &mut self,
        invocation: CommandInvocation,
        _cx: &mut App,
    ) -> anyhow::Result<CommandOutcome> {
        if let CommandTarget::PluginAction { payload, .. } = invocation.target
            && let Some(id) = payload
        {
            let message = match id.parse::<i64>() {
                Ok(action_id) => {
                    let required = self.service.required_parameters(action_id)?;
                    if required.is_empty() {
                        self.service
                            .start_action(action_id)
                            .unwrap_or_else(|error| format!("执行失败: {error}"))
                    } else {
                        let names = required
                            .into_iter()
                            .map(|spec| spec.name)
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("该动作需要参数: {names}。请打开“快速启动”窗口填写后执行")
                    }
                }
                Err(_) => String::from("无效动作标识"),
            };
            return Ok(CommandOutcome {
                message: Some(message),
            });
        }
        Ok(CommandOutcome::default())
    }

    fn start_background(&mut self, events: AppEventBus, cx: &mut App) {
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
                    .timer(Duration::from_millis(250))
                    .await;
                let next_revision = service.revision();
                if next_revision != revision {
                    revision = next_revision;
                    events.publish(manifest::PLUGIN_ID, AppEventKind::CommandsChanged);
                }
            }
        })
        .detach();
    }

    fn close_idle(&mut self) {}
}

struct QuickLaunchSession {
    view: Entity<QuickLaunchView>,
}

impl PluginSession for QuickLaunchSession {
    fn plugin_id(&self) -> &'static str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &'static str {
        "快速启动"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }
}
