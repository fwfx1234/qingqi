use std::{
    any::Any,
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

use gpui::{App, Window};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{
            CommandInvocation, CommandItem, CommandKind, CommandOutcome, ContextKind,
            build_launcher_context_with_clipboard_kinds,
        },
        command_usage::{CommandUsage, CommandUsageStore},
        plugin_spec::{PluginStats, PluginVisualSpec},
        shortcut::ShortcutDescriptor,
    },
};

#[derive(Clone, Debug)]
pub struct PluginListItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub usage_key: String,
    pub enabled: bool,
}

impl PluginListItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        icon: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: subtitle.into(),
            icon: icon.into(),
            usage_key: String::new(),
            enabled: true,
        }
    }

    pub fn with_usage_key(mut self, usage_key: impl Into<String>) -> Self {
        self.usage_key = usage_key.into();
        self
    }
}

pub trait PluginSession {
    fn plugin_id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement;
    fn list_items(&mut self, _cx: &mut App) -> Vec<PluginListItem> {
        Vec::new()
    }
    fn on_input_changed(&mut self, _text: &str, cx: &mut App) -> Vec<PluginListItem> {
        self.list_items(cx)
    }
    fn on_enter(&mut self, _cx: &mut App) -> bool {
        false
    }
    fn on_list_item_selected(&mut self, _item_id: &str, _cx: &mut App) {}
    fn on_reopen(&mut self, _window: &mut Window, _cx: &mut App) {}
    fn on_close(&mut self) {}
}

pub trait PluginRuntime {
    fn manifest(&self) -> PluginManifest;
    fn commands_revision(&self) -> u64 {
        0
    }
    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        vec![CommandItem::plugin_open(
            manifest.id,
            manifest.name,
            manifest.description,
            manifest.keywords.iter().copied(),
            manifest.command_prefixes.iter().copied(),
            manifest.visual.icon,
        )]
    }
    fn commands_for_query(&self, query: &str, limit: usize) -> Vec<CommandItem> {
        let _ = limit;
        self.commands()
            .into_iter()
            .filter(|command| command.score(query).is_some())
            .collect()
    }
    fn open_session(
        &mut self,
        events: AppEventBus,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>>;
    fn handle_command(
        &mut self,
        invocation: CommandInvocation,
        cx: &mut App,
    ) -> anyhow::Result<CommandOutcome> {
        let _ = invocation;
        let _ = cx;
        Ok(CommandOutcome::default())
    }
    fn shortcuts(&self) -> Vec<ShortcutDescriptor> {
        Vec::new()
    }
    fn set_shortcut(
        &mut self,
        shortcut_id: &str,
        accelerator: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let _ = shortcut_id;
        let _ = accelerator;
        let _ = enabled;
        Ok(())
    }
    fn start_background(&mut self, _events: AppEventBus, _cx: &mut App) {}
    fn shutdown(&mut self) {}
    fn close_idle(&mut self) {}
}

#[derive(Clone, Copy, Debug)]
pub struct PluginManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub background: bool,
    pub visual: PluginVisualSpec,
    pub stats: PluginStats,
    pub command_hint: &'static str,
    pub command_prefixes: &'static [&'static str],
}

pub struct PluginManager {
    runtimes: HashMap<&'static str, Box<dyn PluginRuntime>>,
    runtime_order: Vec<&'static str>,
    command_cache: Vec<CommandItem>,
    command_cache_revision: u64,
    command_cache_valid: bool,
    usage_store: CommandUsageStore,
    events: AppEventBus,
}

impl PluginManager {
    pub fn new(events: AppEventBus, usage_store: CommandUsageStore) -> Self {
        Self {
            runtimes: HashMap::new(),
            runtime_order: Vec::new(),
            command_cache: Vec::new(),
            command_cache_revision: 0,
            command_cache_valid: false,
            usage_store,
            events,
        }
    }

    pub fn register(&mut self, runtime: Box<dyn PluginRuntime>) {
        let manifest = match catch_unwind(AssertUnwindSafe(|| runtime.manifest())) {
            Ok(manifest) => manifest,
            Err(error) => {
                tracing::error!(
                    error = %panic_message(error),
                    "plugin panicked while registering"
                );
                return;
            }
        };
        let id = manifest.id;
        if !self.runtimes.contains_key(id) {
            self.runtime_order.push(id);
        }
        self.runtimes.insert(id, runtime);
        self.command_cache_valid = false;
    }

    pub fn commands(&mut self) -> Vec<CommandItem> {
        self.refresh_command_cache();
        self.sorted_commands("", self.command_cache.clone(), false)
    }

    pub fn shortcuts(&mut self) -> Vec<ShortcutDescriptor> {
        self.runtime_order
            .iter()
            .filter_map(|plugin_id| {
                self.runtimes
                    .get(plugin_id)
                    .map(|runtime| (*plugin_id, runtime))
            })
            .flat_map(|(plugin_id, runtime)| {
                catch_unwind(AssertUnwindSafe(|| runtime.shortcuts())).unwrap_or_else(|error| {
                    tracing::error!(
                        plugin_id,
                        error = %panic_message(error),
                        "plugin panicked while building shortcuts"
                    );
                    Vec::new()
                })
            })
            .collect()
    }

    pub fn set_shortcut(
        &mut self,
        plugin_id: &str,
        shortcut_id: &str,
        accelerator: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let runtime = self
            .runtimes
            .get_mut(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("plugin not registered: {plugin_id}"))?;
        catch_unwind(AssertUnwindSafe(|| {
            runtime.set_shortcut(shortcut_id, accelerator, enabled)
        }))
        .unwrap_or_else(|error| {
            Err(anyhow::anyhow!(
                "plugin {plugin_id} panicked while setting shortcut: {}",
                panic_message(error)
            ))
        })
    }

    pub fn commands_with_clipboard(
        &mut self,
        clipboard_kinds: Vec<ContextKind>,
    ) -> Vec<CommandItem> {
        self.refresh_command_cache();
        self.sorted_commands_with_clipboard("", self.command_cache.clone(), false, clipboard_kinds)
    }

    fn build_commands(&self) -> Vec<CommandItem> {
        let mut commands = self
            .runtimes
            .iter()
            .flat_map(|(plugin_id, runtime)| {
                catch_unwind(AssertUnwindSafe(|| runtime.commands())).unwrap_or_else(|error| {
                    tracing::error!(
                        plugin_id = *plugin_id,
                        error = %panic_message(error),
                        "plugin panicked while building commands"
                    );
                    Vec::new()
                })
            })
            .collect::<Vec<_>>();
        commands.sort_by(|a, b| a.title.cmp(&b.title));
        commands
    }

    pub fn commands_revision(&mut self) -> u64 {
        self.refresh_command_cache();
        self.command_cache_revision
    }

    fn current_runtime_commands_revision(&self) -> u64 {
        self.runtimes
            .iter()
            .map(|(plugin_id, runtime)| {
                catch_unwind(AssertUnwindSafe(|| runtime.commands_revision())).unwrap_or_else(
                    |error| {
                        tracing::error!(
                            plugin_id = *plugin_id,
                            error = %panic_message(error),
                            "plugin panicked while reading commands revision"
                        );
                        0
                    },
                )
            })
            .fold(self.runtimes.len() as u64, |acc, revision| {
                acc.wrapping_mul(31).wrapping_add(revision)
            })
    }

    pub fn commands_for_query(&mut self, query: &str, limit: usize) -> Vec<CommandItem> {
        self.commands_for_query_with_clipboard(query, limit, Vec::new())
    }

    pub fn commands_for_query_with_clipboard(
        &mut self,
        query: &str,
        limit: usize,
        clipboard_kinds: Vec<ContextKind>,
    ) -> Vec<CommandItem> {
        self.refresh_command_cache();
        let limit = if limit == 0 { usize::MAX } else { limit };
        let mut scored = self.scored_cached_commands(query, clipboard_kinds);
        self.sort_scored_commands(&mut scored, !query.trim().is_empty());
        scored.truncate(limit);
        scored.into_iter().map(|(_, command)| command).collect()
    }

    fn scored_cached_commands(
        &mut self,
        query: &str,
        clipboard_kinds: Vec<ContextKind>,
    ) -> Vec<(i32, CommandItem)> {
        let known_prefixes = self.known_prefixes();
        let context =
            build_launcher_context_with_clipboard_kinds(query, &known_prefixes, clipboard_kinds);
        let cached_scored = self.command_cache.iter().cloned().filter_map(|command| {
            command
                .score_with_context(&context)
                .map(|matched| (matched.score, command))
        });

        let runtime_query = context.input_body.trim();
        if runtime_query.is_empty() {
            return cached_scored.collect();
        }

        let mut seen = std::collections::HashSet::new();
        let mut scored = cached_scored
            .inspect(|(_, command)| {
                seen.insert(command.id.clone());
            })
            .collect::<Vec<_>>();

        for (plugin_id, runtime) in self.runtimes.iter() {
            let dynamic_commands = catch_unwind(AssertUnwindSafe(|| {
                runtime.commands_for_query(runtime_query, 0)
            }))
            .unwrap_or_else(|error| {
                tracing::error!(
                    plugin_id = *plugin_id,
                    error = %panic_message(error),
                    "plugin panicked while querying dynamic commands"
                );
                Vec::new()
            });
            for command in dynamic_commands {
                if !seen.insert(command.id.clone()) {
                    continue;
                }
                if let Some(matched) = command.score_with_context(&context) {
                    scored.push((matched.score, command));
                }
            }
        }

        scored
    }

    fn sorted_commands(
        &self,
        query: &str,
        commands: Vec<CommandItem>,
        require_positive_score: bool,
    ) -> Vec<CommandItem> {
        self.sorted_commands_with_clipboard(query, commands, require_positive_score, Vec::new())
    }

    fn sorted_commands_with_clipboard(
        &self,
        query: &str,
        commands: Vec<CommandItem>,
        require_positive_score: bool,
        clipboard_kinds: Vec<ContextKind>,
    ) -> Vec<CommandItem> {
        let known_prefixes = self.known_prefixes();
        let context =
            build_launcher_context_with_clipboard_kinds(query, &known_prefixes, clipboard_kinds);
        let mut scored = commands
            .into_iter()
            .filter_map(|command| {
                command
                    .score_with_context(&context)
                    .map(|matched| (matched.score, command))
            })
            .filter(|(score, _)| !require_positive_score || *score > 0)
            .collect::<Vec<_>>();
        self.sort_scored_commands(&mut scored, require_positive_score);
        scored.into_iter().map(|(_, command)| command).collect()
    }

    fn sort_scored_commands(&self, scored: &mut [(i32, CommandItem)], has_query: bool) {
        let usage = self.usage_map();
        if has_query {
            scored.sort_by(|(left_score, left), (right_score, right)| {
                let left_usage = usage.get(&left.usage_key).cloned().unwrap_or_default();
                let right_usage = usage.get(&right.usage_key).cloned().unwrap_or_default();
                right_score
                    .cmp(left_score)
                    .then_with(|| right_usage.use_count.cmp(&left_usage.use_count))
                    .then_with(|| right_usage.last_used_at.cmp(&left_usage.last_used_at))
                    .then_with(|| command_kind_rank(left).cmp(&command_kind_rank(right)))
                    .then_with(|| left.title.cmp(&right.title))
            });
            return;
        }

        scored.sort_by(|(left_score, left), (right_score, right)| {
            let left_usage = usage.get(&left.usage_key).cloned().unwrap_or_default();
            let right_usage = usage.get(&right.usage_key).cloned().unwrap_or_default();
            right_usage
                .use_count
                .cmp(&left_usage.use_count)
                .then_with(|| right_usage.last_used_at.cmp(&left_usage.last_used_at))
                .then_with(|| right_score.cmp(left_score))
                .then_with(|| command_kind_rank(left).cmp(&command_kind_rank(right)))
                .then_with(|| left.title.cmp(&right.title))
        });
    }

    fn usage_map(&self) -> HashMap<String, CommandUsage> {
        self.usage_store.usage_map().unwrap_or_else(|error| {
            tracing::warn!(error = %error, "command usage read failed");
            HashMap::new()
        })
    }

    fn known_prefixes(&self) -> Vec<String> {
        self.command_cache
            .iter()
            .flat_map(|command| command.prefixes.iter().cloned())
            .collect()
    }

    pub fn record_command_launch(&self, command: &CommandItem) {
        self.record_usage_key(&command.usage_key);
    }

    pub fn record_command_launch_background(&self, command: &CommandItem, cx: &mut App) {
        self.record_usage_key_background(command.usage_key.clone(), cx);
    }

    pub fn record_usage_key(&self, usage_key: &str) {
        if let Err(error) = self.usage_store.record_launch(usage_key) {
            tracing::warn!(usage_key, error = %error, "command usage record failed");
        }
    }

    pub fn record_usage_key_background(&self, usage_key: impl Into<String>, cx: &mut App) {
        let usage_key = usage_key.into();
        if usage_key.trim().is_empty() {
            return;
        }

        let usage_store = self.usage_store.clone();
        cx.spawn(async move |async_cx| {
            async_cx
                .background_executor()
                .spawn(async move {
                    if let Err(error) = usage_store.record_launch(&usage_key) {
                        tracing::warn!(usage_key, error = %error, "command usage record failed");
                    }
                })
                .await;
        })
        .detach();
    }

    fn refresh_command_cache(&mut self) {
        let revision = self.current_runtime_commands_revision();
        if self.command_cache_valid && self.command_cache_revision == revision {
            return;
        }
        self.command_cache = self.build_commands();
        self.command_cache_revision = revision;
        self.command_cache_valid = true;
    }

    pub fn manifests(&self) -> Vec<PluginManifest> {
        let mut manifests = self
            .runtimes
            .iter()
            .filter_map(|(plugin_id, runtime)| {
                catch_unwind(AssertUnwindSafe(|| runtime.manifest()))
                    .map_err(|error| {
                        tracing::error!(
                            plugin_id = *plugin_id,
                            error = %panic_message(error),
                            "plugin panicked while reading manifest"
                        );
                    })
                    .ok()
            })
            .collect::<Vec<_>>();
        manifests.sort_by(|a, b| a.name.cmp(b.name));
        manifests
    }

    pub fn open_session(
        &mut self,
        plugin_id: &str,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        let runtime = self
            .runtimes
            .get_mut(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("plugin not registered: {plugin_id}"))?;
        let events = self.events.clone();
        catch_unwind(AssertUnwindSafe(|| runtime.open_session(events, cx))).unwrap_or_else(
            |error| {
                Err(anyhow::anyhow!(
                    "plugin {plugin_id} panicked while opening: {}",
                    panic_message(error)
                ))
            },
        )
    }

    pub fn handle_command(
        &mut self,
        invocation: CommandInvocation,
        cx: &mut App,
    ) -> anyhow::Result<CommandOutcome> {
        let plugin_id = invocation.target.plugin_id().to_string();
        let runtime = self
            .runtimes
            .get_mut(plugin_id.as_str())
            .ok_or_else(|| anyhow::anyhow!("plugin not registered: {plugin_id}"))?;
        catch_unwind(AssertUnwindSafe(|| runtime.handle_command(invocation, cx))).unwrap_or_else(
            |error| {
                Err(anyhow::anyhow!(
                    "plugin {plugin_id} panicked while handling command: {}",
                    panic_message(error)
                ))
            },
        )
    }

    pub fn start_background(&mut self, cx: &mut App) {
        for (plugin_id, runtime) in self.runtimes.iter_mut() {
            let background = catch_unwind(AssertUnwindSafe(|| runtime.manifest().background))
                .unwrap_or_else(|error| {
                    tracing::error!(
                        plugin_id = *plugin_id,
                        error = %panic_message(error),
                        "plugin panicked while checking background mode"
                    );
                    false
                });
            if background {
                let events = self.events.clone();
                let _ = catch_unwind(AssertUnwindSafe(|| runtime.start_background(events, cx)))
                    .map_err(|error| {
                        tracing::error!(
                            plugin_id = *plugin_id,
                            error = %panic_message(error),
                            "plugin panicked while starting background work"
                        );
                    });
            }
        }
    }

    pub fn shutdown(&mut self) {
        for (plugin_id, runtime) in self.runtimes.iter_mut() {
            let _ = catch_unwind(AssertUnwindSafe(|| runtime.shutdown())).map_err(|error| {
                tracing::error!(
                    plugin_id = *plugin_id,
                    error = %panic_message(error),
                    "plugin panicked while shutting down"
                );
            });
        }
    }

    pub fn close_idle(&mut self, plugin_id: &str) {
        if let Some(runtime) = self.runtimes.get_mut(plugin_id) {
            let background = catch_unwind(AssertUnwindSafe(|| runtime.manifest().background))
                .unwrap_or_else(|error| {
                    tracing::error!(
                        plugin_id,
                        error = %panic_message(error),
                        "plugin panicked while checking idle close mode"
                    );
                    true
                });
            if !background {
                let _ = catch_unwind(AssertUnwindSafe(|| runtime.close_idle())).map_err(|error| {
                    tracing::error!(
                        plugin_id,
                        error = %panic_message(error),
                        "plugin panicked while closing idle runtime"
                    );
                });
            }
        }
    }
}

pub fn panic_message(error: Box<dyn Any + Send>) -> String {
    if let Some(message) = error.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = error.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else {
        String::from("unknown panic payload")
    }
}

fn command_kind_rank(command: &CommandItem) -> usize {
    match command.kind {
        CommandKind::Plugin => 0,
        CommandKind::DynamicAction => 1,
    }
}
