use std::sync::Arc;

use gpui::{App, Window};
use serde::{Deserialize, Serialize};

use crate::{
    command::{ClipboardPayload, Command, CommandInvocation, CommandOutcome, ContextMatcher},
    events::{AppEventBus, AppEventKind},
    icon::IconRef,
    plugin_spec::{
        PluginCategory, PluginStats, PluginStatus, PluginVisualSpec, ViewMode, WindowSize,
        WindowSpec,
    },
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
pub type PluginId = Arc<str>;

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
    fn on_input_changed(&mut self, _text: &str, _cx: &mut App) {}
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

    fn start_background(&mut self, _events: AppEventBus, _cx: &mut App) {}

    fn clipboard_boost(&self, _payload: &ClipboardPayload) -> Option<i32> {
        None
    }

    fn shutdown(&mut self) {}
    fn close_idle(&mut self) {}
}

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
    #[serde(skip)]
    pub visual: Option<PluginVisualSpec>,
    #[serde(skip)]
    pub stats: Option<PluginStats>,
    #[serde(skip)]
    pub command_hint: Option<Arc<str>>,
    #[serde(skip)]
    pub command_prefixes: Vec<Arc<str>>,
}

impl Manifest {
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

    pub fn with_keywords(
        mut self,
        keywords: impl IntoIterator<Item = impl Into<Arc<str>>>,
    ) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_prefixes(
        mut self,
        prefixes: impl IntoIterator<Item = impl Into<Arc<str>>>,
    ) -> Self {
        self.prefixes = prefixes.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_dynamic_commands(mut self) -> Self {
        self.dynamic_commands = true;
        self
    }

    pub fn with_background(mut self) -> Self {
        self.background = true;
        self.status = PluginStatus::Background;
        self
    }

    pub fn with_visual(mut self, visual: PluginVisualSpec) -> Self {
        self.visual = Some(visual);
        self
    }

    pub fn with_stats(mut self, stats: PluginStats) -> Self {
        self.stats = Some(stats);
        self
    }

    pub fn with_command_hint(mut self, hint: impl Into<Arc<str>>) -> Self {
        self.command_hint = Some(hint.into());
        self
    }

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

pub fn panic_message(error: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = error.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = error.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else {
        String::from("unknown panic payload")
    }
}
