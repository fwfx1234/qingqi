use std::sync::{Arc, Mutex};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, service::QuickLaunchService, view::QuickLaunchView};
use qingqi_plugin::{
    command::{Action, Activation, Command, CommandInvocation, CommandOutcome},
    database::DatabaseService,
    events::{AppEventBus, AppEventKind},
    plugin::{Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

pub struct QuickLaunchPlugin {
    service: Arc<QuickLaunchService>,
    watch_started: bool,
}

impl QuickLaunchPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Self> {
        Ok(Self {
            service: Arc::new(QuickLaunchService::new(database, paths)?),
            watch_started: false,
        })
    }
}

impl Plugin for QuickLaunchPlugin {
    fn manifest(&self) -> qingqi_plugin::plugin::Manifest {
        manifest::manifest()
    }
    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        Ok(PluginView::Window(Box::new(QuickLaunchWindowView {
            view: cx
                .app
                .new(|cx| QuickLaunchView::new(Arc::clone(&self.service), cx)),
        })))
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        let manifest = self.manifest();
        let mut commands = vec![Command::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.command_prefixes.iter().map(|s| s.as_ref()),
            manifest.icon.as_str(),
        )];
        let actions = self
            .service
            .list_actions("", Some(true))
            .unwrap_or_default();
        commands.extend(actions.into_iter().map(|action| {
            Command::plugin_action(
                manifest.id.as_ref(),
                format!("action-{}", action.id),
                action.name.clone(),
                action.description.clone(),
                action.command_keywords(),
                ["ql", "quick"],
                manifest.icon.as_str(),
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
        // 事件驱动：在 revision 变化时通过 channel 唤醒，代替定时轮询
        let rx = Arc::new(Mutex::new(
            service
                .take_notify_receiver()
                .expect("quick launch notify receiver already taken"),
        ));
        cx.spawn(async move |async_cx| {
            loop {
                let rx = Arc::clone(&rx);
                let Ok(()) = async_cx
                    .background_executor()
                    .spawn(async move { rx.lock().unwrap().recv() })
                    .await
                else {
                    break;
                };
                events.publish(manifest::PLUGIN_ID, AppEventKind::CommandsChanged);
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
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "快速启动".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::QuickActionDraft, store::QuickLaunchStore};
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
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
            .register_database(qingqi_plugin::database::DatabaseSpec::feature(
                manifest::PLUGIN_ID,
                "actions",
                "actions.db",
            ))
            .unwrap();
        let store = QuickLaunchStore::open(
            Arc::clone(&database),
            &qingqi_plugin::database::feature_database_key(manifest::PLUGIN_ID, "actions"),
        )
        .unwrap();
        let action = store
            .create_action(&QuickActionDraft::script(
                "Build Project",
                "demo action",
                "echo ok",
            ))
            .unwrap();
        let runtime = QuickLaunchPlugin::new(database, paths).unwrap();

        let command = runtime
            .commands("")
            .into_iter()
            .find(|command| command.title == "Build Project")
            .expect("quick launch action command should be present");

        assert_eq!(
            command.usage_key,
            format!("quick-launch:action:{}", action.id)
        );
    }
}
