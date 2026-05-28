use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "clipboard";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "剪贴板历史",
        description: "搜索、复制和管理剪贴板文本历史",
        keywords: &[
            "剪贴板",
            "clipboard",
            "copy",
            "paste",
            "历史",
            "复制",
            "粘贴",
        ],
        background: true,
        visual: PluginVisualSpec {
            icon: "qta/mdi6.clipboard-text-outline.png",
            accent: PluginAccent::Blue,
            category: PluginCategory::Tool,
            status: PluginStatus::Background,
            mode: PluginWindowMode::Window,
            window: WindowSpec::fixed_topmost(870.0, 480.0),
        },
        stats: PluginStats {
            primary: "文本历史",
            secondary: "SQLite 持久化",
            tertiary: "后台轮询",
        },
        command_hint: "复制、搜索、置顶、清理",
        command_prefixes: &["clip", "clipboard"],
    }
}
