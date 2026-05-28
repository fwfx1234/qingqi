use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "quick-launch";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "快速启动",
        description: "系统命令与常用动作快速执行",
        keywords: &[
            "快速", "启动", "脚本", "命令", "quick", "launch", "系统", "命令",
        ],
        background: true,
        visual: PluginVisualSpec {
            icon: "qta/fa5s.bolt.png",
            accent: PluginAccent::Amber,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::fixed(860.0, 620.0),
        },
        stats: PluginStats {
            primary: "动作仓库",
            secondary: "运行记录",
            tertiary: "seed data 已接入",
        },
        command_hint: "系统命令与常用动作快速执行",
        command_prefixes: &["ql", "quick"],
    }
}
