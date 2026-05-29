use std::{sync::Arc, time::Duration};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{
    app::events::{AppEventBus, AppEventKind},
    core::{
        command::{Action, Activation, CommandInvocation, CommandItem, CommandOutcome},
        database::{DatabaseService, DatabaseSpec},
        plugin::{Plugin, PluginCx, PluginView, WindowView},
        storage::AppPaths,
    },
    features::quick_launch::{manifest, service::QuickLaunchService, view::QuickLaunchView},
};

pub struct QuickLaunchRuntime {
    service: Arc<QuickLaunchService>,
    watch_started: bool,
}

impl QuickLaunchRuntime {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            service: Arc::new(QuickLaunchService::new(database, paths)?),
            watch_started: false,
        })
    }
}

impl Plugin for QuickLaunchRuntime {
    fn manifest(&self) -> crate::core::plugin::PluginManifest {
        manifest::manifest()
    }

    fn database_specs(&self) -> Vec<DatabaseSpec> {
        vec![DatabaseSpec::feature(
            manifest::PLUGIN_ID,
            "actions",
            "actions.db",
        )]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        Ok(PluginView::Window(Box::new(QuickLaunchWindowView {
            view: cx
                .app
                .new(|cx| QuickLaunchView::new(Arc::clone(&self.service), cx)),
        })))
    }

    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        let mut commands = vec![CommandItem::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.command_prefixes.iter().map(|s| s.as_ref()),
            manifest.visual.icon.as_str(),
        )];
        let actions = self
            .service
            .list_actions("", Some(true))
            .unwrap_or_default();
        commands.extend(actions.into_iter().map(|action| {
            CommandItem::plugin_action(
                manifest.id.as_ref(),
                format!("action-{}", action.id),
                action.name.clone(),
                action.description.clone(),
                action.command_keywords(),
                ["ql", "quick"],
                manifest.visual.icon.as_str(),
                Some(action.id.to_string()),
            )
            .with_usage_key(format!("quick-launch:action:{}", action.id))
        }));
        commands
    }

    fn handle_command(
        &mut self,
        invocation: CommandInvocation,
        _cx: &mut App,
    ) -> anyhow::Result<CommandOutcome> {
        if let Activation::Run(Action::PluginAction { payload, .. }) = invocation.activation
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

struct QuickLaunchWindowView {
    view: Entity<QuickLaunchView>,
}

impl WindowView for QuickLaunchWindowView {
    fn plugin_id(&self) -> &str {
        manifest::PLUGIN_ID
    }

    fn title(&self) -> &str {
        "快速启动"
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{database::DatabaseService, storage::AppPaths},
        features::quick_launch::{model::QuickActionDraft, store::QuickLaunchStore},
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_paths(name: &str) -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-quick-launch-plugin-{name}-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir");
        AppPaths::for_test(dir)
    }

    #[test]
    fn action_commands_use_stable_usage_keys() {
        let paths = temp_paths("usage-key");
        let database = Arc::new(DatabaseService::new(paths.clone()));
        database
            .register_database(crate::core::database::DatabaseSpec::feature(
                manifest::PLUGIN_ID,
                "actions",
                "actions.db",
            ))
            .unwrap();
        let store = QuickLaunchStore::open(
            Arc::clone(&database),
            &crate::core::database::feature_database_key(manifest::PLUGIN_ID, "actions"),
        )
        .unwrap();
        let action = store
            .create_action(&QuickActionDraft::script(
                "Build Project",
                "demo action",
                "echo ok",
            ))
            .unwrap();
        let runtime = QuickLaunchRuntime::new(database, paths).unwrap();

        let command = runtime
            .commands()
            .into_iter()
            .find(|command| command.title == "Build Project")
            .expect("quick launch action command should be present");

        assert_eq!(
            command.usage_key,
            format!("quick-launch:action:{}", action.id)
        );
    }
}
