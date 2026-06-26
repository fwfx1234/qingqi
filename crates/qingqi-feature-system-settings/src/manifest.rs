use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "system-settings";
pub const TRAY_MANAGER_PLUGIN_ID: &str = "tray-manager";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "系统设置".into(),
        description: "主题切换与应用偏好设置".into(),
        keywords: ["设置", "settings", "主题", "theme", "偏好"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/settings.svg"),
        prefixes: vec!["set".into(), "settings".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::ratio(0.72, 0.7),
        category: PluginCategory::System,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/settings.svg"),
            accent: PluginAccent::Slate,
            category: PluginCategory::System,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.72, 0.7),
        }),
        stats: Some(PluginStats {
            primary: "主题设置".into(),
            secondary: "配置持久化".into(),
            tertiary: "偏好设置".into(),
        }),
        command_hint: Some("主题、窗口保留、应用索引与诊断信息".into()),
        command_prefixes: ["set", "settings"].into_iter().map(Into::into).collect(),
    }
}

pub fn tray_manager_manifest() -> Manifest {
    Manifest {
        id: TRAY_MANAGER_PLUGIN_ID.into(),
        name: "托盘管理".into(),
        description: "管理菜单栏托盘项、显示内容与详情弹窗".into(),
        keywords: ["托盘", "tray", "菜单栏", "网速", "状态栏"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/settings.svg"),
        prefixes: vec!["tray".into(), "托盘".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::ratio(0.62, 0.68),
        category: PluginCategory::System,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/settings.svg"),
            accent: PluginAccent::Blue,
            category: PluginCategory::System,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.62, 0.68),
        }),
        stats: Some(PluginStats {
            primary: "托盘项".into(),
            secondary: "网速已接入".into(),
            tertiary: "弹窗配置".into(),
        }),
        command_hint: Some("管理托盘里的网速和后续扩展项".into()),
        command_prefixes: ["tray", "托盘"].into_iter().map(Into::into).collect(),
    }
}
