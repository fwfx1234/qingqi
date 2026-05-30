use std::{fmt, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activation {
    Run(Action),
    Open { plugin_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    LaunchApp {
        path: String,
    },
    PluginAction {
        plugin_id: String,
        action_id: String,
        payload: Option<String>,
    },
}

impl Activation {
    pub fn plugin_id(&self) -> &str {
        match self {
            Self::Run(Action::LaunchApp { .. }) => "app",
            Self::Open { plugin_id } | Self::Run(Action::PluginAction { plugin_id, .. }) => {
                plugin_id
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandKind {
    App,
    Plugin,
    DynamicAction,
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::App => f.write_str("应用"),
            Self::Plugin => f.write_str("插件"),
            Self::DynamicAction => f.write_str("动作"),
        }
    }
}

/// Type alias for the ongoing `Command` → `CommandItem` migration.
pub type CommandItem = Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub plugin_id: String,
    pub title: String,
    pub subtitle: String,
    pub keywords: Vec<String>,
    pub prefixes: Vec<String>,
    pub icon: String,
    pub kind: CommandKind,
    pub activation: Activation,
    pub usage_key: String,
    pub recommend_matchers: Vec<ContextMatcher>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandMatch {
    pub score: i32,
    pub reason: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMatcher {
    pub kind: ContextKind,
    pub boost: i32,
}

impl ContextMatcher {
    pub fn new(kind: ContextKind, boost: i32) -> Self {
        Self { kind, boost }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextKind {
    Clipboard,
    Text,
    Json,
    Url,
    File,
    Image,
    ImageFile,
}

/// Lightweight clipboard snapshot passed to plugins so they can decide
/// whether the current clipboard content is relevant to them.
///
/// This is the payload that [`crate::core::plugin::Plugin::clipboard_boost`]
/// receives — each plugin performs its own matching logic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardPayload {
    pub text: Option<String>,
    pub image_path: Option<String>,
    pub file_paths: Option<Vec<String>>,
}

impl ClipboardPayload {
    pub fn is_empty(&self) -> bool {
        self.text.as_deref().map_or(true, |t| t.trim().is_empty())
            && self.image_path.is_none()
            && self.file_paths.as_ref().map_or(true, |p| p.is_empty())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LauncherContext {
    pub prefix: Option<String>,
    pub input_body: String,
    pub input_kinds: Vec<ContextKind>,
    /// Clipboard content payload — set when the launcher opens and the
    /// latest clipboard record is available.  Plugins that implement
    /// [`crate::core::plugin::Plugin::clipboard_boost`] receive this to
    /// perform their own content matching.
    pub clipboard_payload: Option<ClipboardPayload>,
}

impl Command {
    pub fn app_launch(
        path: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        keywords: impl IntoIterator<Item = impl Into<String>>,
        icon: impl Into<String>,
    ) -> Self {
        let path = path.into();
        Self {
            id: format!("app:{path}"),
            plugin_id: String::from("app"),
            title: title.into(),
            subtitle: subtitle.into(),
            keywords: keywords.into_iter().map(Into::into).collect(),
            prefixes: vec![String::from("app"), String::from("open")],
            icon: icon.into(),
            kind: CommandKind::App,
            activation: Activation::Run(Action::LaunchApp { path }),
            usage_key: String::new(),
            recommend_matchers: Vec::new(),
        }
        .with_default_usage_key()
    }

    pub fn plugin_open(
        plugin_id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        keywords: impl IntoIterator<Item = impl Into<String>>,
        prefixes: impl IntoIterator<Item = impl Into<String>>,
        icon: impl Into<String>,
    ) -> Self {
        let plugin_id = plugin_id.into();
        Self {
            id: format!("{plugin_id}.open"),
            plugin_id: plugin_id.clone(),
            title: title.into(),
            subtitle: subtitle.into(),
            keywords: keywords.into_iter().map(Into::into).collect(),
            prefixes: prefixes.into_iter().map(Into::into).collect(),
            icon: icon.into(),
            kind: CommandKind::Plugin,
            activation: Activation::Open { plugin_id },
            usage_key: String::new(),
            recommend_matchers: Vec::new(),
        }
        .with_default_usage_key()
    }

    pub fn plugin_action(
        plugin_id: impl Into<String>,
        action_id: impl Into<String>,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        keywords: impl IntoIterator<Item = impl Into<String>>,
        prefixes: impl IntoIterator<Item = impl Into<String>>,
        icon: impl Into<String>,
        payload: Option<String>,
    ) -> Self {
        let plugin_id = plugin_id.into();
        let action_id = action_id.into();
        Self {
            id: format!("{plugin_id}.{action_id}"),
            plugin_id: plugin_id.clone(),
            title: title.into(),
            subtitle: subtitle.into(),
            keywords: keywords.into_iter().map(Into::into).collect(),
            prefixes: prefixes.into_iter().map(Into::into).collect(),
            icon: icon.into(),
            kind: CommandKind::DynamicAction,
            activation: Activation::Run(Action::PluginAction {
                plugin_id,
                action_id,
                payload,
            }),
            usage_key: String::new(),
            recommend_matchers: Vec::new(),
        }
        .with_default_usage_key()
    }

    pub fn with_recommend_matchers(
        mut self,
        matchers: impl IntoIterator<Item = ContextMatcher>,
    ) -> Self {
        self.recommend_matchers = matchers.into_iter().collect();
        self
    }

    pub fn with_usage_key(mut self, usage_key: impl Into<String>) -> Self {
        self.usage_key = usage_key.into();
        self
    }

    pub fn score(&self, query: &str) -> Option<CommandMatch> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Some(CommandMatch {
                score: 10,
                reason: "default",
            });
        }

        let (prefix, tail) = q
            .split_once(' ')
            .map(|(prefix, tail)| (prefix, tail.trim()))
            .unwrap_or(("", q.as_str()));
        if !prefix.is_empty()
            && self
                .prefixes
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(prefix))
        {
            if tail.is_empty() {
                return Some(CommandMatch {
                    score: 130,
                    reason: "prefix",
                });
            }
            return self.score_plain(tail).map(|mut command_match| {
                command_match.score += 80;
                command_match.reason = "prefix+text";
                command_match
            });
        }

        self.score_plain(&q)
    }

    pub fn score_with_context(&self, context: &LauncherContext) -> Option<CommandMatch> {
        let base_match = self
            .score(context.input_body.as_str())
            .unwrap_or(CommandMatch {
                score: 0,
                reason: "none",
            });
        let mut matched = base_match.clone();
        let mut recommended = matched.score > 0;

        let prefix_hit = context.prefix.as_deref().is_some_and(|prefix| {
            self.prefixes
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(prefix))
        });
        if prefix_hit {
            matched.score += 240;
            matched.reason = "prefix";
            recommended = true;
        }

        for matcher in &self.recommend_matchers {
            if context.input_kinds.contains(&matcher.kind) {
                matched.score += matcher.boost;
                matched.reason = "context";
                recommended = true;
            }
        }

        recommended.then_some(matched)
    }

    pub fn launch_input(&self, query: &str) -> String {
        let context = build_launcher_context(query, &self.prefixes);
        let input_body = context.input_body.trim();
        if context.prefix.as_deref().is_some_and(|prefix| {
            self.prefixes
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(prefix))
        }) {
            return input_body.to_string();
        }

        if query.trim().is_empty() {
            return String::new();
        }

        let base_score = self
            .score_plain(context.input_body.as_str())
            .map(|matched| matched.score)
            .unwrap_or(0);
        let input_context_match = self
            .recommend_matchers
            .iter()
            .any(|matcher| context.input_kinds.contains(&matcher.kind));

        if input_context_match && base_score <= 0 {
            input_body.to_string()
        } else {
            String::new()
        }
    }

    /// Like [`launch_input`], but when the query is empty and `boost_map`
    /// contains this command's plugin (i.e. the plugin opted into clipboard
    /// content via [`crate::core::plugin::Plugin::clipboard_boost`]), the
    /// matching clipboard payload is returned so the plugin receives it on
    /// open.
    ///
    /// When the query is **non-empty** this delegates straight to
    /// [`launch_input`] — clipboard content is never leaked while the user
    /// is actively typing.
    pub fn launch_input_with_context(
        &self,
        query: &str,
        context: &LauncherContext,
        boost_map: &std::collections::HashMap<String, i32>,
    ) -> String {
        if !query.trim().is_empty() {
            return self.launch_input(query);
        }

        if boost_map.get(&self.plugin_id).copied().unwrap_or(0) <= 0 {
            return String::new();
        }

        let payload = match &context.clipboard_payload {
            Some(p) => p,
            None => return String::new(),
        };

        if let Some(text) = &payload.text {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(path) = &payload.image_path {
            return path.clone();
        }
        if let Some(paths) = &payload.file_paths {
            return paths.join("\n");
        }
        String::new()
    }

    fn with_default_usage_key(mut self) -> Self {
        self.usage_key = match &self.activation {
            Activation::Run(Action::LaunchApp { path }) => format!("app:{path}"),
            Activation::Open { plugin_id } => format!("plugin:{plugin_id}"),
            Activation::Run(Action::PluginAction {
                payload: Some(payload),
                ..
            }) if is_app_path(payload) => format!("app:{payload}"),
            Activation::Run(Action::PluginAction { .. }) => self.id.clone(),
        };
        self
    }

    fn score_plain(&self, query: &str) -> Option<CommandMatch> {
        if query.is_empty() {
            return Some(CommandMatch {
                score: 10,
                reason: "empty",
            });
        }

        let title = self.title.to_lowercase();
        if title == query {
            return Some(CommandMatch {
                score: 120,
                reason: "title-exact",
            });
        }
        if title.starts_with(query) {
            return Some(CommandMatch {
                score: 105,
                reason: "title-prefix",
            });
        }
        if title.contains(query) {
            return Some(CommandMatch {
                score: 90,
                reason: "title",
            });
        }

        for keyword in &self.keywords {
            let keyword = keyword.to_lowercase();
            if keyword == query {
                return Some(CommandMatch {
                    score: 100,
                    reason: "keyword-exact",
                });
            }
            if keyword.contains(query) {
                return Some(CommandMatch {
                    score: 70,
                    reason: "keyword",
                });
            }
        }

        let subtitle = self.subtitle.to_lowercase();
        if subtitle.contains(query) {
            return Some(CommandMatch {
                score: 45,
                reason: "subtitle",
            });
        }
        None
    }
}

pub fn build_launcher_context(query: &str, known_prefixes: &[String]) -> LauncherContext {
    let (prefix, input_body) = parse_prefix(query, known_prefixes);
    LauncherContext {
        input_kinds: detect_text_context_kinds(&input_body),
        prefix,
        input_body,
        ..Default::default()
    }
}

fn parse_prefix(query: &str, known_prefixes: &[String]) -> (Option<String>, String) {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return (None, String::new());
    }

    if let Some((candidate, tail)) = trimmed.split_once(char::is_whitespace) {
        if known_prefixes
            .iter()
            .any(|prefix| prefix.eq_ignore_ascii_case(candidate))
        {
            return (Some(candidate.to_lowercase()), tail.trim().to_string());
        }
        return (None, trimmed.to_string());
    }

    if known_prefixes
        .iter()
        .any(|prefix| prefix.eq_ignore_ascii_case(trimmed))
    {
        return (Some(trimmed.to_lowercase()), String::new());
    }
    (None, trimmed.to_string())
}

pub fn detect_text_context_kinds(value: &str) -> Vec<ContextKind> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut kinds = Vec::new();
    push_context_kind(&mut kinds, ContextKind::Text);
    if is_json_like(trimmed) {
        push_context_kind(&mut kinds, ContextKind::Json);
    }
    if is_url_like(trimmed) {
        push_context_kind(&mut kinds, ContextKind::Url);
    }
    if is_file_like(trimmed) {
        push_context_kind(&mut kinds, ContextKind::File);
    }
    if is_image_file_like(trimmed) {
        push_context_kind(&mut kinds, ContextKind::ImageFile);
    }
    kinds
}

pub fn push_context_kind(kinds: &mut Vec<ContextKind>, kind: ContextKind) {
    if !kinds.contains(&kind) {
        kinds.push(kind);
    }
}

pub fn unique_context_kinds(kinds: impl IntoIterator<Item = ContextKind>) -> Vec<ContextKind> {
    let mut unique = Vec::new();
    for kind in kinds {
        push_context_kind(&mut unique, kind);
    }
    unique
}

fn is_json_like(value: &str) -> bool {
    if !(value.starts_with('{') || value.starts_with('[')) {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(value)
        .map(|value| value.is_object() || value.is_array())
        .unwrap_or(false)
}

fn is_url_like(value: &str) -> bool {
    let lower = value.to_lowercase();
    matches!(
        lower.split_once("://").map(|(scheme, _)| scheme),
        Some("http" | "https" | "ws" | "wss")
    ) || lower.starts_with("www.")
}

fn is_file_like(value: &str) -> bool {
    path_like(value).is_some_and(|path| {
        path.exists()
            || path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some()
    })
}

fn is_image_file_like(value: &str) -> bool {
    let Some(path) = path_like(value) else {
        return false;
    };
    let Some(extension) = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
    else {
        return false;
    };
    matches!(
        extension.as_str(),
        "apng" | "avif" | "bmp" | "gif" | "heic" | "jpeg" | "jpg" | "png" | "tif" | "tiff" | "webp"
    )
}

fn path_like(value: &str) -> Option<&Path> {
    let trimmed = value.trim().trim_matches('"').trim_matches('\'');
    let normalized = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    if normalized.is_empty() {
        return None;
    }
    Some(Path::new(normalized))
}

fn is_app_path(value: &str) -> bool {
    value.ends_with(".app") || value.contains(".app/")
}

#[derive(Clone, Debug)]
pub struct CommandInvocation {
    pub activation: Activation,
}

#[derive(Clone, Debug, Default)]
pub struct CommandOutcome {
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_parses_known_prefix() {
        let context = build_launcher_context("json {\"a\":1}", &[String::from("json")]);

        assert_eq!(context.prefix.as_deref(), Some("json"));
        assert_eq!(context.input_body, "{\"a\":1}");
        assert!(context.input_kinds.contains(&ContextKind::Json));
    }

    #[test]
    fn context_matcher_boosts_matching_content() {
        let command = Command::plugin_open("json-parser", "JSON 解析", "", ["json"], ["json"], "")
            .with_recommend_matchers([ContextMatcher::new(ContextKind::Json, 180)]);
        let context = build_launcher_context("{\"a\":1}", &[]);

        let matched = command.score_with_context(&context).unwrap();

        assert!(matched.score >= 180);
    }

    #[test]
    fn app_actions_use_launch_path_as_usage_key() {
        let command = Command::plugin_action(
            "app-launcher",
            "open-safari",
            "Safari",
            "",
            ["Safari"],
            ["app"],
            "",
            Some(String::from("/Applications/Safari.app")),
        );

        assert_eq!(command.usage_key, "app:/Applications/Safari.app");
    }

    #[test]
    fn launch_input_strips_matching_command_prefix() {
        let command = Command::plugin_open("qr-code", "二维码", "", ["qr"], ["qr", "qrcode"], "");

        assert_eq!(command.launch_input("qr hello world"), "hello world");
    }

    #[test]
    fn launch_input_keeps_context_content_when_command_is_recommended() {
        let command = Command::plugin_open(
            "qr-code",
            "二维码",
            "",
            ["二维码", "qr"],
            ["qr", "qrcode"],
            "",
        )
        .with_recommend_matchers([ContextMatcher::new(ContextKind::Url, 120)]);

        assert_eq!(
            command.launch_input("https://example.com/docs"),
            "https://example.com/docs"
        );
    }

    #[test]
    fn launch_input_ignores_plain_command_search() {
        let command = Command::plugin_open(
            "qr-code",
            "二维码",
            "",
            ["二维码", "qr"],
            ["qr", "qrcode"],
            "",
        )
        .with_recommend_matchers([ContextMatcher::new(ContextKind::Url, 120)]);

        assert_eq!(command.launch_input("二维码"), "");
    }

    // ── launch_input_with_context ─────────────────────────────────

    #[test]
    fn launch_input_with_context_returns_text_when_boosted() {
        let command = Command::plugin_open("json-parser", "JSON 解析", "", ["json"], ["json"], "");

        let mut context = LauncherContext::default();
        context.clipboard_payload = Some(ClipboardPayload {
            text: Some(r#"{"key": "value"}"#.to_string()),
            ..Default::default()
        });

        let mut boost_map = std::collections::HashMap::new();
        boost_map.insert("json-parser".to_string(), 100);

        assert_eq!(
            command.launch_input_with_context("", &context, &boost_map),
            r#"{"key": "value"}"#
        );
    }

    #[test]
    fn launch_input_with_context_ignores_clipboard_when_query_non_empty() {
        let command = Command::plugin_open("json-parser", "JSON 解析", "", ["json"], ["json"], "");

        let mut context = LauncherContext::default();
        context.clipboard_payload = Some(ClipboardPayload {
            text: Some("secret".to_string()),
            ..Default::default()
        });

        let mut boost_map = std::collections::HashMap::new();
        boost_map.insert("json-parser".to_string(), 100);

        // User typed "计算" — clipboard must not leak
        assert_eq!(
            command.launch_input_with_context("计算", &context, &boost_map),
            ""
        );
    }

    #[test]
    fn launch_input_with_context_returns_image_path_when_boosted() {
        let command = Command::plugin_open("image-compress", "图片压缩", "", ["img"], ["img"], "");

        let mut context = LauncherContext::default();
        context.clipboard_payload = Some(ClipboardPayload {
            image_path: Some("/tmp/clipboard-42.png".to_string()),
            ..Default::default()
        });

        let mut boost_map = std::collections::HashMap::new();
        boost_map.insert("image-compress".to_string(), 160);

        assert_eq!(
            command.launch_input_with_context("", &context, &boost_map),
            "/tmp/clipboard-42.png"
        );
    }

    #[test]
    fn launch_input_with_context_empty_when_not_boosted() {
        let command = Command::plugin_open("json-parser", "JSON 解析", "", ["json"], ["json"], "");

        let context = LauncherContext::default();
        let boost_map = std::collections::HashMap::new(); // empty

        assert_eq!(
            command.launch_input_with_context("", &context, &boost_map),
            ""
        );
    }
}
