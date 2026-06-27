use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "network-speed";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "托盘管理".into(),
        description: "管理系统托盘图标和菜单栏网速显示".into(),
        keywords: ["网速", "network", "speed", "托盘", "tray", "上传", "下载"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/settings.svg"),
        prefixes: vec!["net".into(), "网络".into(), "网速".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::ratio(0.46, 0.5),
        category: PluginCategory::System,
        status: PluginStatus::Ready,
        background: true,
        dynamic_commands: false,
        has_settings: true,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/settings.svg"),
            accent: PluginAccent::Green,
            category: PluginCategory::System,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.46, 0.5),
        }),
        stats: Some(PluginStats {
            primary: "网速监控".into(),
            secondary: "菜单栏实时更新".into(),
            tertiary: "详情弹窗".into(),
        }),
        command_hint: Some("网速托盘监控".into()),
        command_prefixes: ["net", "网络", "网速"]
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}
