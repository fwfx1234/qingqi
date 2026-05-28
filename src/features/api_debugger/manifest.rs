use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "api-debugger";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "API 调试器",
        description: "HTTP 接口测试，支持环境切换、参数编辑与响应查看",
        keywords: &["api", "http", "接口", "测试", "request", "response", "env"],
        background: false,
        visual: PluginVisualSpec {
            icon: "qta/mdi6.api.png",
            accent: PluginAccent::Blue,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.84, 0.84),
        },
        stats: PluginStats {
            primary: "HTTP",
            secondary: "环境变量",
            tertiary: "请求编排",
        },
        command_hint: "集合树、环境切换、请求编辑与响应调试",
        command_prefixes: &["api", "http"],
    }
}
