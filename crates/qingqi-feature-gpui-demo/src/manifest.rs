use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "gpui-demo";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "GPUI 学习演示".into(),
        description: "GPUI 组件、布局和交互的 Rust 实验场".into(),
        keywords: ["gpui", "rust", "学习", "demo", "组件", "演示", "教程"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/school.svg"),
        prefixes: vec!["gpui".into(), "demo".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::ratio(0.8, 0.8),
        category: PluginCategory::Tool,
        status: PluginStatus::Preview,
        background: false,
        dynamic_commands: false,
        has_settings: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/school.svg"),
            accent: PluginAccent::Purple,
            category: PluginCategory::Tool,
            status: PluginStatus::Preview,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.8, 0.8),
        }),
        stats: Some(PluginStats {
            primary: "控件范式".into(),
            secondary: "布局样例".into(),
            tertiary: "持续沉淀".into(),
        }),
        command_hint: Some("用于沉淀 Qingqi 的 GPUI 组件、布局和交互范式".into()),
        command_prefixes: ["gpui", "demo"].into_iter().map(Into::into).collect(),
    }
}
