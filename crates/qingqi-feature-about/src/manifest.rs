use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "about";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "关于".into(),
        description: "桌面工具箱版本信息".into(),
        keywords: ["关于", "about", "版本", "version"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/about.svg"),
        prefixes: vec!["about".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::auto(),
        category: PluginCategory::About,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/about.svg"),
            accent: PluginAccent::Amber,
            category: PluginCategory::About,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::auto(),
        }),
        stats: Some(PluginStats {
            primary: "版本信息".into(),
            secondary: "项目概览".into(),
            tertiary: "Rust + GPUI".into(),
        }),
        command_hint: Some("桌面工具箱版本、技术栈与模块概览".into()),
        command_prefixes: ["about"].into_iter().map(Into::into).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_is_about() {
        assert_eq!(PLUGIN_ID, "about");
    }

    #[test]
    fn manifest_has_correct_id() {
        let m = manifest();
        assert_eq!(m.id.as_ref(), "about");
    }

    #[test]
    fn manifest_has_correct_name() {
        let m = manifest();
        assert_eq!(m.name.as_ref(), "关于");
    }

    #[test]
    fn manifest_has_correct_accent() {
        let m = manifest();
        assert_eq!(m.visual.as_ref().unwrap().accent, PluginAccent::Amber);
    }

    #[test]
    fn manifest_has_correct_category() {
        let m = manifest();
        assert_eq!(m.category, PluginCategory::About);
    }

    #[test]
    fn manifest_has_prefixes() {
        let m = manifest();
        assert!(!m.command_prefixes.is_empty());
        assert!(m.command_prefixes.iter().any(|p| p.as_ref() == "about"));
    }

    #[test]
    fn manifest_not_background() {
        let m = manifest();
        assert!(!m.background);
    }
}
