use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "app-launcher";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "软件快速启动",
        description: "搜索并快速启动 macOS 应用程序",
        keywords: &["软件", "启动", "app", "launch", "程序", "打开"],
        background: true,
        visual: PluginVisualSpec {
            icon: "icons/rocket.svg",
            accent: PluginAccent::Rose,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::List,
            window: WindowSpec::fixed(760.0, 560.0),
        },
        stats: PluginStats {
            primary: "本机应用索引",
            secondary: "搜索启动",
            tertiary: "缓存后台刷新",
        },
        command_hint: "本机应用搜索与快速打开",
        command_prefixes: &["app", "open"],
    }
}
