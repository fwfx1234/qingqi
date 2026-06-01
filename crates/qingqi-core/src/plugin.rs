use std::{
    collections::{HashMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use anyhow::Context;
use gpui::App;
use qingqi_plugin::plugin::panic_message;
pub use qingqi_plugin::plugin::{
    InlineView, ListItem, ListView, Manifest, Plugin, PluginCx, PluginId, PluginListItem,
    PluginView, WindowView, recommended_plugin_command,
};
use qingqi_plugin::{
    command::{
        ClipboardPayload, Command, CommandInvocation, CommandKind, CommandOutcome,
        build_launcher_context,
    },
    events::AppEventBus,
    shortcut::ShortcutDescriptor,
};

use crate::command_usage::{CommandUsage, CommandUsageStore};

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
        self.sorted_commands("", &self.command_cache, false)
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
        self.sorted_commands_with_clipboard("", &self.command_cache, false, boost_map)
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
        commands: &[Command],
        require_positive_score: bool,
    ) -> Vec<Command> {
        self.sorted_commands_with_clipboard(query, commands, require_positive_score, &HashMap::new())
    }

    fn sorted_commands_with_clipboard(
        &self,
        query: &str,
        commands: &[Command],
        require_positive_score: bool,
        boost_map: &HashMap<String, i32>,
    ) -> Vec<Command> {
        let known_prefixes = self.known_prefixes();
        let context = build_launcher_context(query, &known_prefixes);
        let mut scored = commands
            .iter()
            .cloned()
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

    /// 统一排序：总分 = 关键词匹配分 + 使用频度分（Frecency）。
    /// 空查询时关键词分为 0，退化为纯 Frecency 排序。
    /// Frecency 权重 5.0：一个使用中的高频应用 (frecency~10) 获得 +50，
    /// 不足以覆盖精确标题匹配 (120)，但足以把近期常用项排在前面。
    pub const FRECENCY_WEIGHT: f64 = 5.0;

    /// 公开的排序函数：总分 = 关键词匹配分 + 使用频度分 + clipboard boost。
    /// 供 Launcher 等外部调用方复用统一的排序逻辑。
    pub fn sort_commands(
        scored: &mut [(i32, Command)],
        usage_map: &HashMap<String, CommandUsage>,
        boost_map: &HashMap<String, i32>,
    ) {
        scored.sort_by(|(left_score, left), (right_score, right)| {
            let left_u = usage_map
                .get(&left.usage_key)
                .cloned()
                .unwrap_or_default();
            let right_u = usage_map
                .get(&right.usage_key)
                .cloned()
                .unwrap_or_default();
            let left_boost = boost_map.get(&left.plugin_id).copied().unwrap_or(0) as f64;
            let right_boost = boost_map.get(&right.plugin_id).copied().unwrap_or(0) as f64;
            let left_total =
                *left_score as f64 + left_u.frecency * Self::FRECENCY_WEIGHT + left_boost;
            let right_total =
                *right_score as f64 + right_u.frecency * Self::FRECENCY_WEIGHT + right_boost;
            right_total
                .partial_cmp(&left_total)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    command_kind_priority(left.kind).cmp(&command_kind_priority(right.kind))
                })
                .then_with(|| left.title.cmp(&right.title))
        });
    }

    fn sort_scored_commands(
        &self,
        scored: &mut [(i32, Command)],
        _has_query: bool,
        boost_map: &HashMap<String, i32>,
    ) {
        Self::sort_commands(scored, &self.usage_map(), boost_map);
    }

    pub fn usage_map(&self) -> HashMap<String, CommandUsage> {
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
        // Spawn directly on the background executor — no GPUI event-loop hop.
        // This guarantees the SQLite write starts immediately and completes
        // in a few ms, well before the next launcher open reads usage_map().
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = usage_store.record_launch(&usage_key) {
                    tracing::warn!(usage_key, error = %error, "command usage record failed");
                }
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

fn call_plugin<T>(plugin_id: &str, f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|error| {
        Err(anyhow::anyhow!(
            "plugin {plugin_id} panicked: {}",
            panic_message(error)
        ))
    })
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

pub fn command_kind_priority(kind: CommandKind) -> u8 {
    match kind {
        CommandKind::Plugin => 0,
        CommandKind::DynamicAction => 1,
        CommandKind::App => 2,
    }
}
