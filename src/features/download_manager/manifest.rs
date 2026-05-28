use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "download-manager";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "下载管理器",
        description: "多任务文件下载，支持断点续传与速度监控",
        keywords: &["下载", "download", "文件", "file", "http", "url"],
        background: false,
        visual: PluginVisualSpec {
            icon: "qta/mdi6.download.png",
            accent: PluginAccent::Green,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.86, 0.82),
        },
        stats: PluginStats {
            primary: "多任务下载",
            secondary: "断点续传",
            tertiary: "reqwest",
        },
        command_hint: "输入 URL 或粘贴链接开始下载",
        command_prefixes: &["down", "download"],
    }
}
