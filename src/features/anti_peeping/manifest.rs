use crate::core::{
    icon::IconRef,
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "anti-peeping";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "防窥屏".into(),
        description: "全屏遮盖屏幕内容，防止旁人窥视".into(),
        keywords: ["防窥屏", "privacy", "遮盖", "屏幕", "防窥", "peeping"]
            .into_iter()
            .map(Into::into)
            .collect(),
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("qta/mdi6.shield-eye-outline.png"),
            accent: PluginAccent::Slate,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::fixed_topmost(420.0, 320.0),
        }),
        stats: Some(PluginStats {
            primary: "全屏遮盖".into(),
            secondary: "自定义图片".into(),
            tertiary: "一键关闭".into(),
        }),
        command_hint: Some("全屏遮盖屏幕内容，按 Esc 退出".into()),
        command_prefixes: ["privacy", "peeping", "防窥", "遮盖"]
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}
