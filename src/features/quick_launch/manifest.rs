use crate::core::{
    icon::IconRef,
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "quick-launch";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "快速启动".into(),
        description: "系统命令与常用动作快速执行".into(),
        keywords: [
            "快速", "启动", "脚本", "命令", "quick", "launch", "系统", "命令",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        background: true,
        dynamic_commands: true,
        visual: PluginVisualSpec {
            icon: IconRef::asset("qta/fa5s.bolt.png"),
            accent: PluginAccent::Amber,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::fixed(860.0, 620.0),
        },
        stats: PluginStats {
            primary: "动作仓库".into(),
            secondary: "运行记录".into(),
            tertiary: "seed data 已接入".into(),
        },
        command_hint: "系统命令与常用动作快速执行".into(),
        command_prefixes: ["ql", "quick"].into_iter().map(Into::into).collect(),
    }
}
