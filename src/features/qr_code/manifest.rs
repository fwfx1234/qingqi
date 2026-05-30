use crate::core::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "qr-code";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "二维码".into(),
        description: "二维码生成与扫描".into(),
        keywords: ["二维码", "qr", "qrcode", "barcode", "生成", "扫描"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/qr.svg"),
        prefixes: vec!["qr".into(), "qrcode".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::auto(),
        category: PluginCategory::Tool,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/qr.svg"),
            accent: PluginAccent::Blue,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::auto(),
        }),
        stats: Some(PluginStats {
            primary: "生成".into(),
            secondary: "剪贴板导入".into(),
            tertiary: "qrcode crate".into(),
        }),
        command_hint: Some("输入文本生成二维码".into()),
        command_prefixes: ["qr", "qrcode"].into_iter().map(Into::into).collect(),
    }
}
