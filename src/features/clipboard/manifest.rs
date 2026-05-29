use crate::core::{
    icon::IconRef,
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "clipboard";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "剪贴板历史".into(),
        description: "搜索、复制和管理剪贴板文本历史".into(),
        keywords: [
            "剪贴板",
            "clipboard",
            "copy",
            "paste",
            "历史",
            "复制",
            "粘贴",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        background: true,
        dynamic_commands: false,
        visual: PluginVisualSpec {
            icon: IconRef::asset("qta/mdi6.clipboard-text-outline.png"),
            accent: PluginAccent::Blue,
            category: PluginCategory::Tool,
            status: PluginStatus::Background,
            mode: PluginWindowMode::Window,
            window: WindowSpec::fixed_topmost(1024.0, 558.0),
        },
        stats: PluginStats {
            primary: "文本历史".into(),
            secondary: "SQLite 持久化".into(),
            tertiary: "后台轮询".into(),
        },
        command_hint: "复制、搜索、置顶、清理".into(),
        command_prefixes: ["clip", "clipboard"].into_iter().map(Into::into).collect(),
    }
}
