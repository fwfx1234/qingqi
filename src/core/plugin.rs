use std::{
    any::Any,
    collections::{HashMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use gpui::{App, Window};
use serde::{Deserialize, Serialize};

use crate::{
    app::events::AppEventBus,
    core::{
        command::{
            CommandInvocation, CommandItem, CommandOutcome, ContextKind, ContextMatcher,
            build_launcher_context_with_clipboard_kinds,
        },
        command_usage::{CommandUsage, CommandUsageStore},
        database::DatabaseSpec,
        plugin_spec::{PluginStats, PluginVisualSpec},
        shortcut::ShortcutDescriptor,
    },
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub usage_key: String,
    pub enabled: bool,
}

pub type PluginListItem = ListItem;

impl ListItem {
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

pub enum PluginView {
    Inline(Box<dyn InlineView>),
    List(Box<dyn ListView>),
    Window(Box<dyn WindowView>),
}

impl PluginView {
    pub fn into_inline(self) -> anyhow::Result<Box<dyn InlineView>> {
        match self {
            Self::Inline(view) => Ok(view),
            _ => anyhow::bail!("plugin returned a non-inline view"),
        }
    }

    pub fn into_list(self) -> anyhow::Result<Box<dyn ListView>> {
        match self {
            Self::List(view) => Ok(view),
            _ => anyhow::bail!("plugin returned a non-list view"),
        }
    }

    pub fn into_window(self) -> anyhow::Result<Box<dyn WindowView>> {
        match self {
            Self::Window(view) => Ok(view),
            _ => anyhow::bail!("plugin returned a non-window view"),
        }
    }
}

pub trait WindowView {
    fn plugin_id(&self) -> &str;
    fn title(&self) -> &str;
    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement;
    fn on_reopen(&mut self, _window: &mut Window, _cx: &mut App) {}
    fn on_close(&mut self) {}
}

pub trait InlineView {
    fn plugin_id(&self) -> &str;
    fn title(&self) -> &str;
    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement;
    fn on_input_changed(&mut self, _text: &str, _cx: &mut App) {}
    fn on_enter(&mut self, _cx: &mut App) -> bool {
        false
    }
    fn on_close(&mut self) {}
}

pub trait ListView {
    fn plugin_id(&self) -> &str;
    fn title(&self) -> &str;
    fn items(&mut self, _cx: &mut App) -> Vec<ListItem>;
    fn on_input_changed(&mut self, text: &str, cx: &mut App) -> Vec<ListItem> {
        let _ = text;
        self.items(cx)
    }
    fn on_enter(&mut self, _cx: &mut App) -> bool {
        false
    }
    fn on_list_item_selected(&mut self, _item_id: &str, _cx: &mut App) {}
    fn on_close(&mut self) {}
}

pub struct PluginCx<'a> {
    pub events: AppEventBus,
    pub app: &'a mut App,
}

impl<'a> PluginCx<'a> {
    pub fn new(events: AppEventBus, app: &'a mut App) -> Self {
        Self { events, app }
    }

    pub fn notify_commands_changed(&self, plugin: &PluginId) {
        self.events.publish(
            plugin.as_ref(),
            crate::app::events::AppEventKind::CommandsChanged,
        );
    }
}

pub trait Plugin {
    fn manifest(&self) -> PluginManifest;
    fn database_specs(&self) -> Vec<DatabaseSpec> {
        Vec::new()
    }
    fn commands(&self) -> Vec<CommandItem> {
        let manifest = self.manifest();
        vec![CommandItem::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.command_prefixes.iter().map(|s| s.as_ref()),
            manifest.visual.icon.as_str(),
        )]
    }
    fn commands_for_query(&self, query: &str, limit: usize) -> Vec<CommandItem> {
        let _ = limit;
        self.commands()
            .into_iter()
            .filter(|command| command.score(query).is_some())
            .collect()
    }
    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView>;
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

pub type PluginId = Arc<str>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: Arc<str>,
    pub description: Arc<str>,
    pub keywords: Vec<Arc<str>>,
    pub background: bool,
    pub dynamic_commands: bool,
    pub visual: PluginVisualSpec,
    pub stats: PluginStats,
    pub command_hint: Arc<str>,
    pub command_prefixes: Vec<Arc<str>>,
}

pub type Manifest = PluginManifest;

pub struct ConfiguredPluginRuntime<S> {
    manifest: fn() -> PluginManifest,
    commands: fn(PluginManifest) -> Vec<CommandItem>,
    open_view: fn(&mut S, &mut PluginCx<'_>) -> anyhow::Result<PluginView>,
    state: S,
}

impl ConfiguredPluginRuntime<()> {
    pub fn new(manifest: fn() -> PluginManifest) -> Self {
        Self::with_state(manifest, ())
    }
}

impl<S> ConfiguredPluginRuntime<S> {
    pub fn with_state(manifest: fn() -> PluginManifest, state: S) -> Self {
        Self {
            manifest,
            commands: default_plugin_commands,
            open_view: |_, _| anyhow::bail!("plugin view factory is not configured"),
            state,
        }
    }

    pub fn with_commands(mut self, commands: fn(PluginManifest) -> Vec<CommandItem>) -> Self {
        self.commands = commands;
        self
    }

    pub fn with_view(
        mut self,
        open_view: fn(&mut S, &mut PluginCx<'_>) -> anyhow::Result<PluginView>,
    ) -> Self {
        self.open_view = open_view;
        self
    }
}

impl<S> Plugin for ConfiguredPluginRuntime<S> {
    fn manifest(&self) -> PluginManifest {
        (self.manifest)()
    }

    fn commands(&self) -> Vec<CommandItem> {
        (self.commands)(self.manifest())
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        (self.open_view)(&mut self.state, cx)
    }

    fn close_idle(&mut self) {}
}

pub struct PanelPluginView<P> {
    plugin_id: &'static str,
    title: &'static str,
    panel: P,
    render: fn(&mut P, &mut Window, &mut App) -> gpui::AnyElement,
    on_input_changed: fn(&mut P, &str, &mut App),
    on_close: fn(&mut P),
}

impl<P> PanelPluginView<P> {
    pub fn new(
        plugin_id: &'static str,
        title: &'static str,
        panel: P,
        render: fn(&mut P, &mut Window, &mut App) -> gpui::AnyElement,
    ) -> Self {
        Self {
            plugin_id,
            title,
            panel,
            render,
            on_input_changed: |_, _, _| {},
            on_close: |_| {},
        }
    }

    pub fn with_input_changed(mut self, on_input_changed: fn(&mut P, &str, &mut App)) -> Self {
        self.on_input_changed = on_input_changed;
        self
    }

    pub fn with_close(mut self, on_close: fn(&mut P)) -> Self {
        self.on_close = on_close;
        self
    }
}

impl<P> WindowView for PanelPluginView<P> {
    fn plugin_id(&self) -> &str {
        self.plugin_id
    }

    fn title(&self) -> &str {
        self.title
    }

    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        (self.render)(&mut self.panel, window, cx)
    }

    fn on_close(&mut self) {
        (self.on_close)(&mut self.panel);
    }
}

impl<P> InlineView for PanelPluginView<P> {
    fn plugin_id(&self) -> &str {
        self.plugin_id
    }

    fn title(&self) -> &str {
        self.title
    }

    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        (self.render)(&mut self.panel, window, cx)
    }

    fn on_input_changed(&mut self, text: &str, cx: &mut App) {
        (self.on_input_changed)(&mut self.panel, text, cx);
    }

    fn on_close(&mut self) {
        (self.on_close)(&mut self.panel);
    }
}

pub fn default_plugin_commands(manifest: PluginManifest) -> Vec<CommandItem> {
    vec![CommandItem::plugin_open(
        manifest.id.as_ref(),
        manifest.name.as_ref(),
        manifest.description.as_ref(),
        manifest.keywords.iter().map(|s| s.as_ref()),
        manifest.command_prefixes.iter().map(|s| s.as_ref()),
        manifest.visual.icon.as_str(),
    )]
}

pub fn recommended_plugin_command(
    manifest: PluginManifest,
    matchers: impl IntoIterator<Item = ContextMatcher>,
) -> Vec<CommandItem> {
    vec![
        CommandItem::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.command_prefixes.iter().map(|s| s.as_ref()),
            manifest.visual.icon.as_str(),
        )
        .with_recommend_matchers(matchers),
    ]
}

pub struct PluginManager {
    runtimes: HashMap<Arc<str>, Box<dyn Plugin>>,
    runtime_order: Vec<Arc<str>>,
    dynamic_runtime_ids: HashSet<Arc<str>>,
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
            dynamic_runtime_ids: HashSet::new(),
            command_cache: Vec::new(),
            command_cache_revision: 0,
            command_cache_valid: false,
            usage_store,
            events,
        }
    }

    pub fn register(&mut self, runtime: Box<dyn Plugin>) {
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
        let dynamic_commands = manifest.dynamic_commands;
        let id = manifest.id;
        if !self.runtimes.contains_key(&id) {
            self.runtime_order.push(id.clone());
        }
        if dynamic_commands {
            self.dynamic_runtime_ids.insert(id.clone());
        } else {
            self.dynamic_runtime_ids.remove(&id);
        }
        self.runtimes.insert(id, runtime);
        self.invalidate_commands();
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
                    .map(|runtime| (plugin_id.clone(), runtime))
            })
            .flat_map(|(plugin_id, runtime)| {
                catch_unwind(AssertUnwindSafe(|| runtime.shortcuts())).unwrap_or_else(|error| {
                    tracing::error!(
                        plugin_id = plugin_id.as_ref(),
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
                        plugin_id = plugin_id.as_ref(),
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

    pub fn command_cache_revision(&mut self) -> u64 {
        self.command_cache_revision
    }

    pub fn invalidate_commands(&mut self) {
        self.command_cache_valid = false;
        self.command_cache_revision = self.command_cache_revision.wrapping_add(1);
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
            if !self.dynamic_runtime_ids.contains(plugin_id) {
                continue;
            }
            let dynamic_commands = catch_unwind(AssertUnwindSafe(|| {
                runtime.commands_for_query(runtime_query, 0)
            }))
            .unwrap_or_else(|error| {
                tracing::error!(
                    plugin_id = plugin_id.as_ref(),
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
        if self.command_cache_valid {
            return;
        }
        self.command_cache = self.build_commands();
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
                            plugin_id = plugin_id.as_ref(),
                            error = %panic_message(error),
                            "plugin panicked while reading manifest"
                        );
                    })
                    .ok()
            })
            .collect::<Vec<_>>();
        manifests.sort_by(|a, b| a.name.cmp(&b.name));
        manifests
    }

    pub fn open(&mut self, plugin_id: &str, cx: &mut App) -> anyhow::Result<PluginView> {
        let runtime = self
            .runtimes
            .get_mut(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("plugin not registered: {plugin_id}"))?;
        let events = self.events.clone();
        let mut plugin_cx = PluginCx::new(events, cx);
        catch_unwind(AssertUnwindSafe(|| runtime.open(&mut plugin_cx))).unwrap_or_else(|error| {
            Err(anyhow::anyhow!(
                "plugin {plugin_id} panicked while opening: {}",
                panic_message(error)
            ))
        })
    }

    pub fn open_window_view(
        &mut self,
        plugin_id: &str,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn WindowView>> {
        self.open(plugin_id, cx).and_then(PluginView::into_window)
    }

    pub fn open_inline_view(
        &mut self,
        plugin_id: &str,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn InlineView>> {
        self.open(plugin_id, cx).and_then(PluginView::into_inline)
    }

    pub fn open_list_view(
        &mut self,
        plugin_id: &str,
        cx: &mut App,
    ) -> anyhow::Result<Box<dyn ListView>> {
        self.open(plugin_id, cx).and_then(PluginView::into_list)
    }

    pub fn handle_command(
        &mut self,
        invocation: CommandInvocation,
        cx: &mut App,
    ) -> anyhow::Result<CommandOutcome> {
        let plugin_id = invocation.activation.plugin_id().to_string();
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
                        plugin_id = plugin_id.as_ref(),
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
                            plugin_id = plugin_id.as_ref(),
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
                    plugin_id = plugin_id.as_ref(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{database::DatabaseService, storage::AppPaths};
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-plugin-manager-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    fn usage_store(name: &str) -> CommandUsageStore {
        let path = temp_db(name);
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(crate::core::database::DatabaseSpec::path(
                "command-usage",
                path,
            ))
            .unwrap();
        CommandUsageStore::new(database, "command-usage")
    }

    #[test]
    fn empty_query_sort_treats_plugin_and_action_as_peers() {
        let events = AppEventBus::default();
        let usage_store = usage_store("sort-peer.db");
        usage_store
            .record_launch("app:/Applications/Fixture.app")
            .unwrap();
        let manager = PluginManager::new(events, usage_store);
        let plugin = CommandItem::plugin_open(
            "quick-launch",
            "快速启动",
            "启动项",
            ["quick"],
            ["ql"],
            "icons/rocket.svg",
        );
        let app = CommandItem::plugin_action(
            "app-launcher",
            "open-fixture",
            "Fixture App",
            "dev.fixture.app",
            ["fixture"],
            ["app"],
            "",
            Some(String::from("/Applications/Fixture.app")),
        );

        let sorted = manager.sorted_commands("", vec![plugin, app], false);

        assert_eq!(sorted[0].title, "Fixture App");
    }

    #[test]
    fn empty_query_sort_treats_apps_and_quick_launch_actions_as_peers() {
        let events = AppEventBus::default();
        let usage_store = usage_store("sort-apps-quick-launch.db");
        usage_store.record_launch("quick-launch:action:42").unwrap();
        usage_store.record_launch("quick-launch:action:42").unwrap();
        usage_store
            .record_launch("app:/Applications/Fixture.app")
            .unwrap();
        let manager = PluginManager::new(events, usage_store);
        let app = CommandItem::plugin_action(
            "app-launcher",
            "open-fixture",
            "Fixture App",
            "dev.fixture.app",
            ["fixture"],
            ["app"],
            "",
            Some(String::from("/Applications/Fixture.app")),
        );
        let quick_launch = CommandItem::plugin_action(
            "quick-launch",
            "action-42",
            "Build Project",
            "Run local build",
            ["build"],
            ["ql", "quick"],
            "qta/fa5s.bolt.png",
            Some(String::from("42")),
        )
        .with_usage_key("quick-launch:action:42");

        let sorted = manager.sorted_commands("", vec![app, quick_launch], false);

        assert_eq!(sorted[0].title, "Build Project");
    }
}
