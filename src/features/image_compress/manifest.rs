use crate::core::{
    icon::IconRef,
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "image-compress";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "图片压缩".into(),
        description: "PNG/JPEG/WebP 批量压缩".into(),
        keywords: [
            "图片", "压缩", "image", "compress", "png", "jpg", "jpeg", "webp",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("qta/mdi6.image-size-select-large.png"),
            accent: PluginAccent::Amber,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.82, 0.8),
        }),
        stats: Some(PluginStats {
            primary: "批量压缩".into(),
            secondary: "目录导出".into(),
            tertiary: "image crate".into(),
        }),
        command_hint: "拖入图片后批量压缩导出".into(),
        command_prefixes: ["img", "image", "compress"]
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}
