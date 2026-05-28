use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "qr-code";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "二维码",
        description: "二维码生成与扫描",
        keywords: &["二维码", "qr", "qrcode", "barcode", "生成", "扫描"],
        background: false,
        visual: PluginVisualSpec {
            icon: "qta/mdi6.qrcode.png",
            accent: PluginAccent::Blue,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.76, 0.76),
        },
        stats: PluginStats {
            primary: "生成",
            secondary: "剪贴板导入",
            tertiary: "qrcode crate",
        },
        command_hint: "输入文本生成二维码",
        command_prefixes: &["qr", "qrcode"],
    }
}
