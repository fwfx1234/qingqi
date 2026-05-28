use std::{fmt, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandTarget {
    PluginOpen {
        plugin_id: String,
    },
    PluginAction {
        plugin_id: String,
        action_id: String,
        payload: Option<String>,
    },
}

impl CommandTarget {
    pub fn plugin_id(&self) -> &str {
        match self {
            Self::PluginOpen { plugin_id } | Self::PluginAction { plugin_id, .. } => plugin_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Plugin,
    DynamicAction,
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plugin => f.write_str("插件"),
            Self::DynamicAction => f.write_str("动作"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommandItem {
    pub id: String,
    pub plugin_id: String,
    pub title: String,
    pub subtitle: String,
    pub keywords: Vec<String>,
    pub prefixes: Vec<String>,
    pub icon: String,
    pub kind: CommandKind,
    pub target: CommandTarget,
    pub usage_key: String,
    pub recommend_matchers: Vec<ContextMatcher>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandMatch {
    pub score: i32,
    pub reason: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextMatcher {
    pub source: ContextSource,
    pub kind: ContextKind,
    pub boost: i32,
}

impl ContextMatcher {
    pub fn new(kind: ContextKind, boost: i32) -> Self {
        Self {
            source: ContextSource::Input,
            kind,
            boost,
        }
    }

    pub fn clipboard(kind: ContextKind, boost: i32) -> Self {
        Self {
            source: ContextSource::Clipboard,
            kind,
            boost,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextSource {
    Input,
    Clipboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextKind {
    Clipboard,
    Text,
    Json,
    Url,
    File,
    Image,
    ImageFile,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LauncherContext {
    pub prefix: Option<String>,
    pub input_body: String,
    pub input_kinds: Vec<ContextKind>,
    pub clipboard_kinds: Vec<ContextKind>,
}

impl CommandItem {
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
            target: CommandTarget::PluginOpen { plugin_id },
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
            target: CommandTarget::PluginAction {
                plugin_id,
                action_id,
                payload,
            },
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

    pub fn score(&self, query: &str) -> Option<CommandMatch> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Some(CommandMatch {
                score: match self.kind {
                    CommandKind::Plugin => 20,
                    CommandKind::DynamicAction => 8,
                },
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

        let has_explicit_text = !context.input_body.trim().is_empty();
        for matcher in &self.recommend_matchers {
            let kinds = match matcher.source {
                ContextSource::Input => &context.input_kinds,
                ContextSource::Clipboard => {
                    if has_explicit_text && base_match.score <= 0 && !prefix_hit {
                        continue;
                    }
                    &context.clipboard_kinds
                }
            };
            if kinds.contains(&matcher.kind) {
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
        let input_context_match = self.recommend_matchers.iter().any(|matcher| {
            matcher.source == ContextSource::Input && context.input_kinds.contains(&matcher.kind)
        });

        if input_context_match && base_score <= 0 {
            input_body.to_string()
        } else {
            String::new()
        }
    }

    fn with_default_usage_key(mut self) -> Self {
        self.usage_key = match &self.target {
            CommandTarget::PluginOpen { plugin_id } => format!("plugin:{plugin_id}"),
            CommandTarget::PluginAction {
                payload: Some(payload),
                ..
            } if is_app_path(payload) => format!("app:{payload}"),
            CommandTarget::PluginAction { .. } => self.id.clone(),
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
    build_launcher_context_with_clipboard_kinds(query, known_prefixes, Vec::new())
}

pub fn build_launcher_context_with_clipboard_kinds(
    query: &str,
    known_prefixes: &[String],
    clipboard_kinds: Vec<ContextKind>,
) -> LauncherContext {
    let (prefix, input_body) = parse_prefix(query, known_prefixes);
    LauncherContext {
        input_kinds: detect_text_context_kinds(&input_body),
        clipboard_kinds: unique_context_kinds(clipboard_kinds),
        prefix,
        input_body,
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
    pub target: CommandTarget,
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
        let command =
            CommandItem::plugin_open("json-parser", "JSON 解析", "", ["json"], ["json"], "")
                .with_recommend_matchers([ContextMatcher::new(ContextKind::Json, 180)]);
        let context = build_launcher_context("{\"a\":1}", &[]);

        let matched = command.score_with_context(&context).unwrap();

        assert!(matched.score >= 180);
    }

    #[test]
    fn clipboard_matcher_boosts_empty_query() {
        let command =
            CommandItem::plugin_open("json-parser", "JSON 解析", "", ["json"], ["json"], "")
                .with_recommend_matchers([ContextMatcher::clipboard(ContextKind::Json, 100)]);
        let context = build_launcher_context_with_clipboard_kinds("", &[], vec![ContextKind::Json]);

        let matched = command.score_with_context(&context).unwrap();

        assert!(matched.score >= 100);
    }

    #[test]
    fn clipboard_matcher_skips_unrelated_explicit_query() {
        let command =
            CommandItem::plugin_open("json-parser", "JSON 解析", "", ["json"], ["json"], "")
                .with_recommend_matchers([ContextMatcher::clipboard(ContextKind::Json, 100)]);
        let context =
            build_launcher_context_with_clipboard_kinds("calculator", &[], vec![ContextKind::Json]);

        assert!(command.score_with_context(&context).is_none());
    }

    #[test]
    fn app_actions_use_launch_path_as_usage_key() {
        let command = CommandItem::plugin_action(
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
        let command =
            CommandItem::plugin_open("qr-code", "二维码", "", ["qr"], ["qr", "qrcode"], "");

        assert_eq!(command.launch_input("qr hello world"), "hello world");
    }

    #[test]
    fn launch_input_keeps_context_content_when_command_is_recommended() {
        let command = CommandItem::plugin_open(
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
        let command = CommandItem::plugin_open(
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
}
