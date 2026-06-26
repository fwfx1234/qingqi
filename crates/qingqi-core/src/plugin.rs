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
        ClipboardPayload, Command, CommandInvocation, CommandKind, CommandOutcome, MatchScore,
        build_launcher_context,
    },
    events::AppEventBus,
    shortcut::ShortcutDescriptor,
};

use crate::{
    command_catalog::CommandCatalogStore,
    command_usage::{CommandUsage, CommandUsageStore},
};

pub struct PluginManager {
    plugins: HashMap<Arc<str>, Box<dyn Plugin>>,
    plugin_order: Vec<Arc<str>>,
    dynamic_plugin_ids: HashSet<Arc<str>>,
    command_cache: Vec<Command>,
    command_cache_valid: bool,
    usage_store: CommandUsageStore,
    command_catalog_store: CommandCatalogStore,
    events: AppEventBus,
}

impl PluginManager {
    pub fn new(
        events: AppEventBus,
        usage_store: CommandUsageStore,
        command_catalog_store: CommandCatalogStore,
    ) -> Self {
        Self {
            plugins: HashMap::new(),
            plugin_order: Vec::new(),
            dynamic_plugin_ids: HashSet::new(),
            command_cache: Vec::new(),
            command_cache_valid: false,
            usage_store,
            command_catalog_store,
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
                match call_plugin_value(&plugin_id, || plugin.shortcuts()) {
                    Ok(shortcuts) => shortcuts,
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %error,
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
        call_plugin(&pid, || plugin.set_shortcut(&sid, &acc, enabled))
            .with_context(|| format!("plugin {pid} panicked in set_shortcut"))
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
                match call_plugin_value(id.as_ref(), || plugin.commands("")) {
                    Ok(plugin_commands) => commands.extend(plugin_commands),
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %id,
                            error = %error,
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

    pub fn rebuild_command_catalog(&mut self) -> anyhow::Result<()> {
        let commands = self.build_commands();
        self.command_catalog_store.save_commands(&commands)?;
        self.command_cache = commands;
        self.command_cache_valid = true;
        Ok(())
    }

    pub fn catalog_commands(&self) -> Vec<Command> {
        self.command_catalog_store
            .load_commands()
            .unwrap_or_else(|error| {
                tracing::warn!(error = %error, "command catalog read failed");
                Vec::new()
            })
    }

    pub fn query_catalog_commands_with_clipboard(
        &self,
        query: &str,
        limit: usize,
        boost_map: &HashMap<String, i32>,
    ) -> Vec<Command> {
        let commands = self.catalog_commands();
        let known_prefixes = commands
            .iter()
            .flat_map(|command| command.prefixes.iter().cloned())
            .collect::<Vec<_>>();
        let context = build_launcher_context(query, &known_prefixes);
        let scored = commands
            .into_iter()
            .filter_map(|command| {
                command
                    .score_with_context(&context)
                    .map(|matched| (matched, command))
            })
            .collect::<Vec<_>>();
        let mut sorted = self.sort_scored_commands(scored, boost_map);
        if limit > 0 {
            sorted.truncate(limit);
        }
        sorted
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
        let scored = self.scored_cached_commands(query);
        let mut sorted = self.sort_scored_commands(scored, boost_map);
        sorted.truncate(limit);
        sorted
    }

    fn scored_cached_commands(&mut self, query: &str) -> Vec<(MatchScore, Command)> {
        let known_prefixes = self.known_prefixes();
        let context = build_launcher_context(query, &known_prefixes);
        let cached_scored = self.command_cache.iter().cloned().filter_map(|command| {
            command
                .score_with_context(&context)
                .map(|matched| (matched, command))
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
            let dynamic_commands =
                match call_plugin_value(plugin_id.as_ref(), || plugin.commands(plugin_query)) {
                    Ok(commands) => commands,
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %error,
                            "plugin panicked in commands()"
                        );
                        continue;
                    }
                };
            for command in dynamic_commands {
                if !seen.insert(command.id.clone()) {
                    continue;
                }
                if let Some(matched) = command.score_with_context(&context) {
                    scored.push((matched, command));
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
        commands: &[Command],
        require_positive_score: bool,
        boost_map: &HashMap<String, i32>,
    ) -> Vec<Command> {
        let known_prefixes = self.known_prefixes();
        let context = build_launcher_context(query, &known_prefixes);
        let scored = commands
            .iter()
            .cloned()
            .filter_map(|command| {
                command
                    .score_with_context(&context)
                    .map(|matched| (matched, command))
            })
            .filter(|(score, _)| !require_positive_score || score.keyword > 0 || score.intent > 0)
            .collect::<Vec<_>>();
        self.sort_scored_commands(scored, boost_map)
    }

    pub fn build_clipboard_boost_map(&self, payload: &ClipboardPayload) -> HashMap<String, i32> {
        self.plugins
            .iter()
            .filter_map(|(id, plugin)| {
                let plugin_id = id.clone();
                match call_plugin_value(&plugin_id, || plugin.clipboard_boost(payload)) {
                    Ok(Some(boost)) => Some((plugin_id.to_string(), boost)),
                    Ok(None) => None,
                    Err(error) => {
                        tracing::error!(
                            plugin_id = %plugin_id,
                            error = %error,
                            "plugin panicked in clipboard_boost()"
                        );
                        None
                    }
                }
            })
            .collect()
    }

    /// 加权融合的权重。关键字为原生分主项（×1）；意图、使用度按下列系数融入。
    /// 关键字接近时高使用度/强意图可翻盘小差距，但明显更强的关键字不会被压垮。
    /// 手感偏移时只需调这两个常量。
    pub const INTENT_WEIGHT: f64 = 0.25;
    pub const USAGE_WEIGHT: f64 = 1.5;

    /// 唯一排序函数（默认列表与查询列表共用）。
    ///
    /// `total = keyword + (intent + clipboard_boost) × INTENT_WEIGHT + effective_frecency × USAGE_WEIGHT`，
    /// 按 `total DESC → 插件优先 → 名称` 排序。`now_unix` 在调用前取一次传入，
    /// 保证排序传递性，且每项的 `effective_frecency`（含 powf）只计算一次。
    pub fn sort_commands(
        scored: Vec<(MatchScore, Command)>,
        usage_map: &HashMap<String, CommandUsage>,
        boost_map: &HashMap<String, i32>,
        now_unix: i64,
    ) -> Vec<Command> {
        // decorate：一次性预算 (总分, kind 优先级, command)。
        let mut decorated: Vec<(f64, u8, Command)> = scored
            .into_iter()
            .map(|(score, command)| {
                let intent =
                    (score.intent + boost_map.get(&command.plugin_id).copied().unwrap_or(0)) as f64;
                let freq = usage_map
                    .get(&command.usage_key)
                    .map(|usage| usage.effective_frecency(now_unix))
                    .unwrap_or(0.0);
                let total =
                    score.keyword as f64 + intent * Self::INTENT_WEIGHT + freq * Self::USAGE_WEIGHT;
                (total, command_kind_priority(command.kind), command)
            })
            .collect();
        decorated.sort_by(
            |(left_total, left_kind, left), (right_total, right_kind, right)| {
                right_total
                    .partial_cmp(left_total)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left_kind.cmp(right_kind))
                    .then_with(|| left.title.cmp(&right.title))
            },
        );
        decorated
            .into_iter()
            .map(|(_, _, command)| command)
            .collect()
    }

    fn sort_scored_commands(
        &self,
        scored: Vec<(MatchScore, Command)>,
        boost_map: &HashMap<String, i32>,
    ) -> Vec<Command> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        Self::sort_commands(scored, &self.usage_map(), boost_map, now)
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
            if let Err(error) =
                call_plugin_value(id.as_ref(), || plugin.start_background(events, cx))
            {
                tracing::error!(
                    plugin_id = %id,
                    error = %error,
                    "plugin panicked in start_background"
                );
            }
        }
    }

    pub fn shutdown(&mut self) {
        for plugin in self.plugins.values_mut() {
            let id = plugin.manifest().id.clone();
            if let Err(error) = call_plugin_value(id.as_ref(), || plugin.shutdown()) {
                tracing::error!(
                    plugin_id = %id,
                    error = %error,
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
                if let Err(error) = call_plugin_value(id.as_ref(), || plugin.close_idle()) {
                    tracing::error!(
                        plugin_id = %id,
                        error = %error,
                        "plugin panicked in close_idle"
                    );
                }
            }
        }
    }
}

fn call_plugin<T>(plugin_id: &str, f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    call_plugin_value(plugin_id, f)?
}

fn call_plugin_value<T>(plugin_id: &str, f: impl FnOnce() -> T) -> anyhow::Result<T> {
    let span = tracing::info_span!("plugin", plugin_id = %plugin_id);
    let _enter = span.enter();
    catch_unwind(AssertUnwindSafe(f))
        .map_err(|error| anyhow::anyhow!("plugin {plugin_id} panicked: {}", panic_message(error)))
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

#[cfg(test)]
mod sort_tests {
    use super::*;

    const NOW: i64 = 1_700_000_000;

    fn used(frecency: f64) -> CommandUsage {
        CommandUsage {
            use_count: 1,
            last_used_at: NOW,
            frecency,
        }
    }

    fn titles(commands: &[Command]) -> Vec<&str> {
        commands.iter().map(|c| c.title.as_str()).collect()
    }

    #[test]
    fn usage_overrides_small_keyword_gap() {
        // Alpha: 关键字 105 + 高频(20) → 105 + 20×1.5 = 135；Beta: 关键字 120 → 120。
        let alpha =
            Command::plugin_open("alpha", "Alpha", "", ["a"], std::iter::empty::<&str>(), "");
        let beta = Command::plugin_open("beta", "Beta", "", ["b"], std::iter::empty::<&str>(), "");
        let mut usage = HashMap::new();
        usage.insert("plugin:alpha".to_string(), used(20.0));

        let scored = vec![
            (
                MatchScore {
                    keyword: 120,
                    intent: 0,
                },
                beta,
            ),
            (
                MatchScore {
                    keyword: 105,
                    intent: 0,
                },
                alpha,
            ),
        ];
        let sorted = PluginManager::sort_commands(scored, &usage, &HashMap::new(), NOW);
        assert_eq!(titles(&sorted), vec!["Alpha", "Beta"]);
    }

    #[test]
    fn keyword_dominates_pure_intent() {
        // Word: 关键字 90 → 90；Ctx: 意图 180 → 180×0.25 = 45。关键字胜。
        let word = Command::plugin_open("word", "Word", "", ["w"], std::iter::empty::<&str>(), "");
        let ctx = Command::plugin_open("ctx", "Ctx", "", ["c"], std::iter::empty::<&str>(), "");
        let scored = vec![
            (
                MatchScore {
                    keyword: 0,
                    intent: 180,
                },
                ctx,
            ),
            (
                MatchScore {
                    keyword: 90,
                    intent: 0,
                },
                word,
            ),
        ];
        let sorted = PluginManager::sort_commands(scored, &HashMap::new(), &HashMap::new(), NOW);
        assert_eq!(titles(&sorted), vec!["Word", "Ctx"]);
    }

    #[test]
    fn intent_outranks_moderate_usage() {
        // Ctx: 意图 180 → 45；Freq: 高频(20) → 30。意图胜。
        let ctx = Command::plugin_open("ctx", "Ctx", "", ["c"], std::iter::empty::<&str>(), "");
        let freq = Command::plugin_open("freq", "Freq", "", ["f"], std::iter::empty::<&str>(), "");
        let mut usage = HashMap::new();
        usage.insert("plugin:freq".to_string(), used(20.0));

        let scored = vec![
            (
                MatchScore {
                    keyword: 0,
                    intent: 0,
                },
                freq,
            ),
            (
                MatchScore {
                    keyword: 0,
                    intent: 180,
                },
                ctx,
            ),
        ];
        let sorted = PluginManager::sort_commands(scored, &usage, &HashMap::new(), NOW);
        assert_eq!(titles(&sorted), vec!["Ctx", "Freq"]);
    }

    #[test]
    fn recorded_beats_unrecorded_and_plugin_first_on_tie() {
        // 有记录的 app(7.5) > 无记录项(0)；无记录项之间插件(kind0)在 app(kind2) 前。
        let used_app = Command::app_launch("/apps/Zoom", "Zoom", "", ["zoom"], "");
        let unused_plugin =
            Command::plugin_open("calc", "Calc", "", ["calc"], std::iter::empty::<&str>(), "");
        let unused_app = Command::app_launch("/apps/Notes", "Notes", "", ["notes"], "");
        let mut usage = HashMap::new();
        usage.insert("app:/apps/Zoom".to_string(), used(5.0));

        let scored = vec![
            (MatchScore::default(), unused_app),
            (MatchScore::default(), used_app),
            (MatchScore::default(), unused_plugin),
        ];
        let sorted = PluginManager::sort_commands(scored, &usage, &HashMap::new(), NOW);
        assert_eq!(titles(&sorted), vec!["Zoom", "Calc", "Notes"]);
    }

    #[test]
    fn clipboard_boost_counts_as_intent() {
        // clipboard boost 按 plugin_id 并入意图分：160×0.25 = 40 > 无加成项。
        let boosted = Command::plugin_open("img", "Img", "", ["i"], std::iter::empty::<&str>(), "");
        let plain =
            Command::plugin_open("plain", "Plain", "", ["p"], std::iter::empty::<&str>(), "");
        let mut boost = HashMap::new();
        boost.insert("img".to_string(), 160);

        let scored = vec![
            (
                MatchScore {
                    keyword: 10,
                    intent: 0,
                },
                plain,
            ),
            (
                MatchScore {
                    keyword: 10,
                    intent: 0,
                },
                boosted,
            ),
        ];
        let sorted = PluginManager::sort_commands(scored, &HashMap::new(), &boost, NOW);
        assert_eq!(titles(&sorted), vec!["Img", "Plain"]);
    }
}
