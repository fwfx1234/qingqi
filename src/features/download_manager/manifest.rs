use crate::core::{
    icon::IconRef,
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "download-manager";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "下载管理器".into(),
        description: "多任务文件下载，支持断点续传与速度监控".into(),
        keywords: ["下载", "download", "文件", "file", "http", "url"]
            .into_iter()
            .map(Into::into)
            .collect(),
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("qta/mdi6.download.png"),
            accent: PluginAccent::Green,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.86, 0.82),
        }),
        stats: Some(PluginStats {
            primary: "多任务下载".into(),
            secondary: "断点续传".into(),
            tertiary: "reqwest".into(),
        }),
        command_hint: "输入 URL 或粘贴链接开始下载".into(),
        command_prefixes: ["down", "download"].into_iter().map(Into::into).collect(),
    }
}
