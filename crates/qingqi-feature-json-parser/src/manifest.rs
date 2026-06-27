use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "json-parser";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "JSON 解析".into(),
        description: "JSON 格式化、验证与 JSONPath 查询".into(),
        keywords: ["json", "格式化", "format", "parse", "解析", "query"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/json.svg"),
        prefixes: vec!["json".into(), "jq".into()],
        mode: PluginWindowMode::Inline,
        window: WindowSpec::auto(),
        category: PluginCategory::Tool,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        has_settings: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/json.svg"),
            accent: PluginAccent::Green,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::auto(),
        }),
        stats: Some(PluginStats {
            primary: "格式化".into(),
            secondary: "JSONPath".into(),
            tertiary: "serde_json".into(),
        }),
        command_hint: Some("双栏输入/输出与 JSONPath 查询".into()),
        command_prefixes: ["json", "jq"].into_iter().map(Into::into).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_is_json_parser() {
        assert_eq!(PLUGIN_ID, "json-parser");
    }

    #[test]
    fn manifest_has_correct_id() {
        let manifest = manifest();
        assert_eq!(manifest.id.as_ref(), "json-parser");
    }

    #[test]
    fn manifest_has_correct_name() {
        let manifest = manifest();
        assert_eq!(manifest.name.as_ref(), "JSON 解析");
    }

    #[test]
    fn manifest_has_correct_accent() {
        let manifest = manifest();
        assert_eq!(
            manifest.visual.as_ref().unwrap().accent,
            PluginAccent::Green
        );
    }

    #[test]
    fn manifest_has_correct_category() {
        let manifest = manifest();
        assert_eq!(manifest.category, PluginCategory::Tool);
    }

    #[test]
    fn manifest_has_prefixes() {
        let manifest = manifest();
        assert!(
            manifest
                .command_prefixes
                .iter()
                .any(|p| p.as_ref() == "json")
        );
        assert!(manifest.command_prefixes.iter().any(|p| p.as_ref() == "jq"));
    }

    #[test]
    fn manifest_not_background() {
        let manifest = manifest();
        assert!(!manifest.background);
    }
}
