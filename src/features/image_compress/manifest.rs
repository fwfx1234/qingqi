use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "image-compress";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "图片压缩",
        description: "PNG/JPEG/WebP 批量压缩",
        keywords: &[
            "图片", "压缩", "image", "compress", "png", "jpg", "jpeg", "webp",
        ],
        background: false,
        visual: PluginVisualSpec {
            icon: "qta/mdi6.image-size-select-large.png",
            accent: PluginAccent::Amber,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.82, 0.8),
        },
        stats: PluginStats {
            primary: "批量压缩",
            secondary: "目录导出",
            tertiary: "image crate",
        },
        command_hint: "拖入图片后批量压缩导出",
        command_prefixes: &["img", "image", "compress"],
    }
}
