use crate::core::{
    icon::IconRef,
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "api-debugger";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID.into(),
        name: "API 调试器".into(),
        description: "HTTP 接口测试，支持环境切换、参数编辑与响应查看".into(),
        keywords: ["api", "http", "接口", "测试", "request", "response", "env"]
            .into_iter()
            .map(Into::into)
            .collect(),
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("qta/mdi6.api.png"),
            accent: PluginAccent::Blue,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.84, 0.84),
        }),
        stats: Some(PluginStats {
            primary: "HTTP".into(),
            secondary: "环境变量".into(),
            tertiary: "请求编排".into(),
        }),
        command_hint: Some("集合树、环境切换、请求编辑与响应调试".into()),
        command_prefixes: ["api", "http"].into_iter().map(Into::into).collect(),
    }
}
