use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "display-optimizer";
const ICON: &str = "lucide/monitor-up.svg";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "外接屏优化".into(),
        description: "为外接 2K 显示器启用真正的 HiDPI 模式".into(),
        keywords: ["显示器", "屏幕", "2K", "HiDPI", "分辨率", "Retina"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset(ICON),
        prefixes: vec!["display".into(), "hidpi".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::ratio(0.72, 0.72),
        category: PluginCategory::System,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        has_settings: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset(ICON),
            accent: PluginAccent::Cyan,
            category: PluginCategory::System,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.72, 0.72),
        }),
        stats: Some(PluginStats {
            primary: "2K HiDPI".into(),
            secondary: "可验证回滚".into(),
            tertiary: "macOS 12.4+".into(),
        }),
        command_hint: Some("检测外接 2560×1440 屏幕并安装 HiDPI 模式".into()),
        command_prefixes: ["display", "hidpi"].into_iter().map(Into::into).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_inline_system_plugin() {
        let manifest = manifest();
        assert_eq!(manifest.id.as_ref(), PLUGIN_ID);
        assert_eq!(manifest.category, PluginCategory::System);
        assert_eq!(manifest.mode, PluginWindowMode::Inline);
        assert_eq!(
            manifest.visual.as_ref().map(|visual| visual.accent),
            Some(PluginAccent::Cyan)
        );
    }
}
