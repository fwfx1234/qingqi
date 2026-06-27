use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "image-compress";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "图片压缩".into(),
        description: "PNG/JPEG/WebP 批量压缩".into(),
        keywords: [
            "图片", "压缩", "image", "compress", "png", "jpg", "jpeg", "webp",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        icon: IconRef::asset("icons/image.svg"),
        prefixes: vec!["img".into(), "image".into(), "compress".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::auto(),
        category: PluginCategory::Tool,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        has_settings: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/image.svg"),
            accent: PluginAccent::Amber,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::auto(),
        }),
        stats: Some(PluginStats {
            primary: "批量压缩".into(),
            secondary: "目录导出".into(),
            tertiary: "image crate".into(),
        }),
        command_hint: Some("拖入图片后批量压缩导出".into()),
        command_prefixes: ["img", "image", "compress"]
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}
