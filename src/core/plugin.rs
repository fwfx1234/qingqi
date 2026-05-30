use std::{
    any::Any,
    collections::{HashMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use anyhow::Context;

use gpui::{App, Window};
use serde::{Deserialize, Serialize};

use crate::core::{
    command::{
        ClipboardPayload, Command, CommandInvocation, CommandKind, CommandOutcome, ContextMatcher,
        build_launcher_context,
    },
    command_usage::{CommandUsage, CommandUsageStore},
    events::{AppEventBus, AppEventKind},
    icon::IconRef,
    plugin_spec::{PluginCategory, PluginStatus, ViewMode, WindowSize, WindowSpec},
    shortcut::ShortcutDescriptor,
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
    pub fn mode(&self) -> ViewMode {
        match self {
            Self::Inline(_) => ViewMode::Inline,
            Self::List(_) => ViewMode::List,
            Self::Window(_) => ViewMode::Window,
        }
    }

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
    fn plugin_id(&self) -> PluginId;
    fn title(&self) -> Arc<str>;
    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement;
    fn on_reopen(&mut self, _window: &mut Window, _cx: &mut App) {}
    fn on_close(&mut self) {}
}

pub trait InlineView {
    fn plugin_id(&self) -> PluginId;
    fn title(&self) -> Arc<str>;
    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement;
    fn on_input_changed(&mut self, _text: &str, _cx: &mut App) {}
    fn on_enter(&mut self, _cx: &mut App) -> bool {
        false
    }
    fn on_close(&mut self) {}
}

pub trait ListView {
    fn plugin_id(&self) -> PluginId;
    fn title(&self) -> Arc<str>;
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
        self.events
            .publish(plugin.as_ref(), AppEventKind::CommandsChanged);
    }
}

pub trait Plugin {
    fn manifest(&self) -> Manifest;
    fn commands(&self, _query: &str) -> Vec<Command> {
        let manifest = self.manifest();
        vec![Command::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.prefixes.iter().map(|s| s.as_ref()),
            manifest.icon.as_str(),
        )]
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
    fn database_specs(&self) -> Vec<crate::core::database::DatabaseSpec> {
        Vec::new()
    }
    fn start_background(&mut self, _events: AppEventBus, _cx: &mut App) {}
    /// Called when the launcher opens without user input and clipboard
    /// content is available.  Return `Some(boost)` to signal that this
    /// plugin can handle the current clipboard content — the higher the
    /// boost the closer to the top it appears.
    ///
    /// Return `None` (the default) if this plugin is not interested in
    /// clipboard content.
    fn clipboard_boost(&self, _payload: &ClipboardPayload) -> Option<i32> {
        None
    }
    fn shutdown(&mut self) {}
    fn close_idle(&mut self) {}
}

pub type PluginId = Arc<str>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub id: PluginId,
    pub name: Arc<str>,
    pub description: Arc<str>,
    pub icon: IconRef,
    pub keywords: Vec<Arc<str>>,
    pub prefixes: Vec<Arc<str>>,
    pub mode: ViewMode,
    pub window: WindowSpec,
    pub category: PluginCategory,
    pub status: PluginStatus,
    pub background: bool,
    pub dynamic_commands: bool,
    /// Migration fields — added for compatibility with ongoing refactoring.
    #[serde(skip)]
    pub visual: Option<crate::core::plugin_spec::PluginVisualSpec>,
    #[serde(skip)]
    pub stats: Option<crate::core::plugin_spec::PluginStats>,
    #[serde(skip)]
    pub command_hint: Option<Arc<str>>,
    #[serde(skip)]
    pub command_prefixes: Vec<Arc<str>>,
}

impl Manifest {
    /// Minimal inline plugin — only the essentials, everything else defaults.
    ///
    /// The window uses [`WindowSize::Auto`] so the launcher panel flexes to
    /// content height instead of requiring a hardcoded size.
    pub fn inline(
        id: impl Into<Arc<str>>,
        name: impl Into<Arc<str>>,
        description: impl Into<Arc<str>>,
        icon: IconRef,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            icon,
            keywords: vec![],
            prefixes: vec![],
            mode: ViewMode::Inline,
            window: WindowSpec::auto(),
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            background: false,
            dynamic_commands: false,
            visual: None,
            stats: None,
            command_hint: None,
            command_prefixes: vec![],
        }
    }

    /// Window plugin with explicit size.
    pub fn windowed(
        id: impl Into<Arc<str>>,
        name: impl Into<Arc<str>>,
        description: impl Into<Arc<str>>,
        icon: IconRef,
        size: WindowSize,
    ) -> Self {
        Self {
            mode: ViewMode::Window,
            window: WindowSpec::from_size(size),
            ..Self::inline(id, name, description, icon)
        }
    }

    /// Builder-style: attach keywords.
    pub fn with_keywords(
        mut self,
        keywords: impl IntoIterator<Item = impl Into<Arc<str>>>,
    ) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Builder-style: attach command prefixes.
    pub fn with_prefixes(
        mut self,
        prefixes: impl IntoIterator<Item = impl Into<Arc<str>>>,
    ) -> Self {
        self.prefixes = prefixes.into_iter().map(Into::into).collect();
        self
    }

    /// Builder-style: set `dynamic_commands = true`.
    pub fn with_dynamic_commands(mut self) -> Self {
        self.dynamic_commands = true;
        self
    }

    /// Builder-style: run in background.
    pub fn with_background(mut self) -> Self {
        self.background = true;
        self.status = PluginStatus::Background;
        self
    }

    /// Builder-style: attach a visual spec for the plugin overview UI.
    pub fn with_visual(mut self, visual: crate::core::plugin_spec::PluginVisualSpec) -> Self {
        self.visual = Some(visual);
        self
    }

    /// Builder-style: attach stats for the plugin overview UI.
    pub fn with_stats(mut self, stats: crate::core::plugin_spec::PluginStats) -> Self {
        self.stats = Some(stats);
        self
    }

    /// Builder-style: attach a command hint shown in the launcher.
    pub fn with_command_hint(mut self, hint: impl Into<Arc<str>>) -> Self {
        self.command_hint = Some(hint.into());
        self
    }

    /// Builder-style: attach command prefixes (same keys as `prefixes`, kept
    /// separately during the manifest migration).
    pub fn with_command_prefixes(
        mut self,
        prefixes: impl IntoIterator<Item = impl Into<Arc<str>>>,
    ) -> Self {
        self.command_prefixes = prefixes.into_iter().map(Into::into).collect();
        self
    }
}

pub fn recommended_plugin_command(
    manifest: Manifest,
    matchers: impl IntoIterator<Item = ContextMatcher>,
) -> Vec<Command> {
    vec![
        Command::plugin_open(
            manifest.id.as_ref(),
            manifest.name.as_ref(),
            manifest.description.as_ref(),
            manifest.keywords.iter().map(|s| s.as_ref()),
            manifest.prefixes.iter().map(|s| s.as_ref()),
            manifest.icon.as_str(),
        )
        .with_recommend_matchers(matchers),
    ]
}

fn default_plugin_command(manifest: Manifest) -> Vec<Command> {
    vec![Command::plugin_open(
        manifest.id.as_ref(),
        manifest.name.as_ref(),
        manifest.description.as_ref(),
        manifest.keywords.iter().map(|s| s.as_ref()),
        manifest.prefixes.iter().map(|s| s.as_ref()),
        manifest.icon.as_str(),
    )]
}

pub struct PluginManager {
    plugins: HashMap<Arc<str>, Box<dyn Plugin>>,
    plugin_order: Vec<Arc<str>>,
    dynamic_plugin_ids: HashSet<Arc<str>>,
    command_cache: Vec<Command>,
    command_cache_valid: bool,
    usage_store: CommandUsageStore,
    events: AppEventBus,
}

impl PluginManager {
    pub fn new(events: AppEventBus, usage_store: CommandUsageStore) -> Self {
        Self {
            plugins: HashMap::new(),
            plugin_order: Vec::new(),
            dynamic_plugin_ids: HashSet::new(),
            command_cache: Vec::new(),
            command_cache_valid: false,
            usage_store,
            events,
        }
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let manifest = plugin.manifest();
        let dynamic_commands = manifest.dynamic_commands;
        let id = manifest.id;
        if !self.plugins.contains_key(&id) {
            self.plugin_order.push(id.clone());
        }
        if dynamic_commands {
            self.dynamic_plugin_ids.insert(id.clone());
        } else {
            self.dynamic_plugin_ids.remove(&id);
        }
        self.plugins.insert(id, plugin);
        self.invalidate_commands();
    }

    pub fn commands(&mut self) -> Vec<Command> {
        self.refresh_command_cache();
        self.sorted_commands("", self.command_cache.clone(), false)
    }

    pub fn shortcuts(&mut self) -> Vec<ShortcutDescriptor> {
        self.plugin_order
            .iter()
            .filter_map(|plugin_id| {
                self.plugins
                    .get(plugin_id)
                    .map(|plugin| (plugin_id.clone(), plugin))
            })
            .flat_map(|(id, plugin)| {
                let plugin_id = id.clone();
                match catch_unwind(AssertUnwindSafe(|| plugin.shortcuts())) {
                    Ok(shortcuts) => shortcuts,
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %panic_message(error),
                            "plugin panicked in shortcuts()"
                        );
                        Vec::new()
                    }
                }
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
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("plugin not registered: {plugin_id}"))?;
        let pid = plugin_id.to_string();
        let sid = shortcut_id.to_string();
        let acc = accelerator.to_string();
        catch_unwind(AssertUnwindSafe(|| {
            plugin.set_shortcut(&sid, &acc, enabled)
        }))
        .unwrap_or_else(|error| {
            Err(anyhow::anyhow!(
                "plugin {pid} panicked in set_shortcut: {}",
                panic_message(error)
            ))
        })
    }

    pub fn commands_with_clipboard(&mut self, boost_map: &HashMap<String, i32>) -> Vec<Command> {
        self.refresh_command_cache();
        self.sorted_commands_with_clipboard("", self.command_cache.clone(), false, boost_map)
    }

    fn build_commands(&self) -> Vec<Command> {
        let mut commands: Vec<Command> = Vec::new();
        for (plugin_id, plugin) in self.plugins.iter() {
            if self.dynamic_plugin_ids.contains(plugin_id) {
                commands.extend(default_plugin_command(plugin.manifest()));
            } else {
                let id = plugin_id.clone();
                match catch_unwind(AssertUnwindSafe(|| plugin.commands(""))) {
                    Ok(plugin_commands) => commands.extend(plugin_commands),
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %id,
                            error = %panic_message(error),
                            "plugin panicked in commands()"
                        );
                    }
                }
            }
        }
        commands.sort_by(|a, b| a.title.cmp(&b.title));
        commands
    }

    pub fn invalidate_commands(&mut self) {
        self.command_cache_valid = false;
    }

    pub fn query_commands(&mut self, query: &str, limit: usize) -> Vec<Command> {
        self.query_commands_with_clipboard(query, limit, &HashMap::new())
    }

    pub fn query_commands_with_clipboard(
        &mut self,
        query: &str,
        limit: usize,
        boost_map: &HashMap<String, i32>,
    ) -> Vec<Command> {
        self.refresh_command_cache();
        let limit = if limit == 0 { usize::MAX } else { limit };
        let has_query = !query.trim().is_empty();
        let mut scored = self.scored_cached_commands(query);
        self.sort_scored_commands(&mut scored, has_query, boost_map);
        scored.truncate(limit);
        scored.into_iter().map(|(_, command)| command).collect()
    }

    fn scored_cached_commands(&mut self, query: &str) -> Vec<(i32, Command)> {
        let known_prefixes = self.known_prefixes();
        let context = build_launcher_context(query, &known_prefixes);
        let cached_scored = self.command_cache.iter().cloned().filter_map(|command| {
            command
                .score_with_context(&context)
                .map(|matched| (matched.score, command))
        });

        let plugin_query = context.input_body.trim();
        if plugin_query.is_empty() {
            return cached_scored.collect();
        }

        let mut seen = std::collections::HashSet::new();
        let mut scored = cached_scored
            .inspect(|(_, command)| {
                seen.insert(command.id.clone());
            })
            .collect::<Vec<_>>();

        for (plugin_id, plugin) in self.plugins.iter() {
            if !self.dynamic_plugin_ids.contains(plugin_id) {
                continue;
            }
            let dynamic_commands = plugin.commands(plugin_query);
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
        commands: Vec<Command>,
        require_positive_score: bool,
    ) -> Vec<Command> {
        self.sorted_commands_with_clipboard(
            query,
            commands,
            require_positive_score,
            &HashMap::new(),
        )
    }

    fn sorted_commands_with_clipboard(
        &self,
        query: &str,
        commands: Vec<Command>,
        require_positive_score: bool,
        boost_map: &HashMap<String, i32>,
    ) -> Vec<Command> {
        let known_prefixes = self.known_prefixes();
        let context = build_launcher_context(query, &known_prefixes);
        let mut scored = commands
            .into_iter()
            .filter_map(|command| {
                command
                    .score_with_context(&context)
                    .map(|matched| (matched.score, command))
            })
            .filter(|(score, _)| !require_positive_score || *score > 0)
            .collect::<Vec<_>>();
        self.sort_scored_commands(&mut scored, require_positive_score, boost_map);
        scored.into_iter().map(|(_, command)| command).collect()
    }

    /// Build a map of `plugin_id → boost` by asking every plugin to
    /// inspect the clipboard payload.  Only plugins that return
    /// `Some(boost)` appear in the result.
    pub fn build_clipboard_boost_map(&self, payload: &ClipboardPayload) -> HashMap<String, i32> {
        self.plugins
            .iter()
            .filter_map(|(id, plugin)| {
                let plugin_id = id.clone();
                match catch_unwind(AssertUnwindSafe(|| plugin.clipboard_boost(payload))) {
                    Ok(Some(boost)) => Some((plugin_id.to_string(), boost)),
                    Ok(None) => None,
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %panic_message(error),
                            "plugin panicked in clipboard_boost()"
                        );
                        None
                    }
                }
            })
            .collect()
    }

    fn sort_scored_commands(
        &self,
        scored: &mut [(i32, Command)],
        has_query: bool,
        boost_map: &HashMap<String, i32>,
    ) {
        let usage = self.usage_map();
        if has_query {
            scored.sort_by(|(left_score, left), (right_score, right)| {
                let left_usage = usage.get(&left.usage_key).cloned().unwrap_or_default();
                let right_usage = usage.get(&right.usage_key).cloned().unwrap_or_default();
                right_score
                    .cmp(left_score)
                    .then_with(|| {
                        command_kind_priority(left.kind).cmp(&command_kind_priority(right.kind))
                    })
                    .then_with(|| right_usage.use_count.cmp(&left_usage.use_count))
                    .then_with(|| right_usage.last_used_at.cmp(&left_usage.last_used_at))
                    .then_with(|| left.title.cmp(&right.title))
            });
            return;
        }

        // ── No query: three tiers ───────────────────────────────────
        // Tier 0 – plugin returned clipboard_boost > 0  → score first
        // Tier 1 – previously used, no clipboard match  → usage first
        // Tier 2 – everything else                       → usage first
        //
        // Within each tier, CommandKind acts as tiebreaker:
        // Plugin (0) > DynamicAction (1) > App (2).
        // This ensures plugins surface first when nothing has usage data,
        // but a used app outranks an unused plugin.
        scored.sort_by(|(left_score, left), (right_score, right)| {
            let left_usage = usage.get(&left.usage_key).cloned().unwrap_or_default();
            let right_usage = usage.get(&right.usage_key).cloned().unwrap_or_default();

            let left_tier = command_sort_tier(left, left_usage.use_count, boost_map);
            let right_tier = command_sort_tier(right, right_usage.use_count, boost_map);

            let tier_cmp = left_tier.cmp(&right_tier);
            if tier_cmp != std::cmp::Ordering::Equal {
                return tier_cmp;
            }

            if left_tier == 0 {
                // Tier 0: score (with context boost) first
                right_score
                    .cmp(left_score)
                    .then_with(|| right_usage.use_count.cmp(&left_usage.use_count))
                    .then_with(|| right_usage.last_used_at.cmp(&left_usage.last_used_at))
                    .then_with(|| {
                        command_kind_priority(left.kind).cmp(&command_kind_priority(right.kind))
                    })
                    .then_with(|| left.title.cmp(&right.title))
            } else {
                // Tier 1 & 2: usage first
                right_usage
                    .use_count
                    .cmp(&left_usage.use_count)
                    .then_with(|| right_usage.last_used_at.cmp(&left_usage.last_used_at))
                    .then_with(|| right_score.cmp(left_score))
                    .then_with(|| {
                        command_kind_priority(left.kind).cmp(&command_kind_priority(right.kind))
                    })
                    .then_with(|| left.title.cmp(&right.title))
            }
        });
    }

    /// Returns the sort tier for a command.
    ///
    /// Tier 0 — specific-content clipboard match (Json, Image, Url, …).
    /// Tier 1 — previously used (`use_count > 0`) but no clipboard match.
    /// Tier 2 — neither used nor clipboard-matched.
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

    pub fn record_command_launch(&self, command: &Command) {
        self.record_usage_key(&command.usage_key);
    }

    pub fn record_command_launch_background(&self, command: &Command, cx: &mut App) {
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

    pub fn manifests(&self) -> Vec<Manifest> {
        let mut manifests = self
            .plugins
            .values()
            .map(|plugin| plugin.manifest())
            .collect::<Vec<_>>();
        manifests.sort_by(|a, b| a.name.cmp(&b.name));
        manifests
    }

    pub fn open(&mut self, plugin_id: &str, cx: &mut App) -> anyhow::Result<PluginView> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("plugin not registered: {plugin_id}"))?;
        let expected_mode = plugin.manifest().mode;
        let events = self.events.clone();
        let mut plugin_cx = PluginCx::new(events, cx);
        let view = call_plugin(plugin_id, || plugin.open(&mut plugin_cx))
            .with_context(|| format!("plugin {plugin_id} panicked while opening"))?;
        debug_assert_eq!(
            expected_mode,
            view.mode(),
            "plugin {plugin_id} returned a view that does not match manifest mode"
        );
        Ok(view)
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
        let plugin = self
            .plugins
            .get_mut(plugin_id.as_str())
            .ok_or_else(|| anyhow::anyhow!("plugin not registered: {plugin_id}"))?;
        call_plugin(&plugin_id, || plugin.handle_command(invocation, cx))
            .with_context(|| format!("plugin {plugin_id} panicked while handling command"))
    }

    pub fn start_background(&mut self, cx: &mut App) {
        for plugin in self.plugins.values_mut() {
            let background = plugin.manifest().background;
            if !background {
                continue;
            }
            let id = plugin.manifest().id.clone();
            let events = self.events.clone();
            let result = catch_unwind(AssertUnwindSafe(|| plugin.start_background(events, cx)));
            if let Err(error) = result {
                tracing::error!(
                    plugin_id = %id,
                    error = %panic_message(error),
                    "plugin panicked in start_background"
                );
            }
        }
    }

    pub fn shutdown(&mut self) {
        for plugin in self.plugins.values_mut() {
            let id = plugin.manifest().id.clone();
            let result = catch_unwind(AssertUnwindSafe(|| plugin.shutdown()));
            if let Err(error) = result {
                tracing::error!(
                    plugin_id = %id,
                    error = %panic_message(error),
                    "plugin panicked in shutdown"
                );
            }
        }
    }

    pub fn close_idle(&mut self, plugin_id: &str) {
        if let Some(plugin) = self.plugins.get_mut(plugin_id) {
            let background = plugin.manifest().background;
            if !background {
                let id = plugin.manifest().id.clone();
                let result = catch_unwind(AssertUnwindSafe(|| plugin.close_idle()));
                if let Err(error) = result {
                    tracing::error!(
                        plugin_id = %id,
                        error = %panic_message(error),
                        "plugin panicked in close_idle"
                    );
                }
            }
        }
    }
}

/// Single panic boundary for plugin activation dispatch.
///
/// All plugin calls that constitute "activation" (open, handle_command) pass
/// through this function so that a panicking plugin never takes down the
/// launcher or other plugin windows.  This is the **one** isolation seam
/// required by the architecture (§6.4); lifecycle hooks (start_background,
/// shutdown, close_idle) keep their own lightweight guards.
fn call_plugin<T>(plugin_id: &str, f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|error| {
        Err(anyhow::anyhow!(
            "plugin {plugin_id} panicked: {}",
            panic_message(error)
        ))
    })
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

/// Sort tier for the no-query launcher results.
///
/// Tier 0 – plugin signalled clipboard relevance (in `boost_map`).
/// Tier 1 – previously used (`use_count > 0`) but no clipboard match.
/// Tier 2 – neither used nor clipboard-matched.
fn command_sort_tier(command: &Command, use_count: i64, boost_map: &HashMap<String, i32>) -> u8 {
    if boost_map.get(&command.plugin_id).copied().unwrap_or(0) > 0 {
        return 0;
    }
    if use_count > 0 {
        return 1;
    }
    2
}

/// Priority within the same sort tier.
///
/// Lower number = higher priority.
/// Plugins surface first by default so the user discovers them;
/// usage data (use_count) still dominates, so a frequently-used app
/// outranks an unused plugin.
pub fn command_kind_priority(kind: CommandKind) -> u8 {
    match kind {
        CommandKind::Plugin => 0,
        CommandKind::DynamicAction => 1,
        CommandKind::App => 2,
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
        let plugin = Command::plugin_open(
            "quick-launch",
            "快速启动",
            "启动项",
            ["quick"],
            ["ql"],
            "icons/rocket.svg",
        );
        let app = Command::plugin_action(
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
        let app = Command::plugin_action(
            "app-launcher",
            "open-fixture",
            "Fixture App",
            "dev.fixture.app",
            ["fixture"],
            ["app"],
            "",
            Some(String::from("/Applications/Fixture.app")),
        );
        let quick_launch = Command::plugin_action(
            "quick-launch",
            "action-42",
            "Build Project",
            "Run local build",
            ["build"],
            ["ql", "quick"],
            "icons/bolt.svg",
            Some(String::from("42")),
        )
        .with_usage_key("quick-launch:action:42");

        let sorted = manager.sorted_commands("", vec![app, quick_launch], false);

        assert_eq!(sorted[0].title, "Build Project");
    }
}
