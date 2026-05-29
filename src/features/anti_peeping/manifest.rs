use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "anti-peeping";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "防窥屏",
        description: "全屏遮盖屏幕内容，防止旁人窥视",
        keywords: &["防窥屏", "privacy", "遮盖", "屏幕", "防窥", "peeping"],
        background: false,
        visual: PluginVisualSpec {
            icon: "qta/mdi6.shield-eye-outline.png",
            accent: PluginAccent::Slate,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::fixed_topmost(420.0, 320.0),
        },
        stats: PluginStats {
            primary: "全屏遮盖",
            secondary: "自定义图片",
            tertiary: "一键关闭",
        },
        command_hint: "全屏遮盖屏幕内容，按 Esc 退出",
        command_prefixes: &["privacy", "peeping", "防窥", "遮盖"],
    }
}
