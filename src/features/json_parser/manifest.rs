use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "json-parser";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "JSON 解析",
        description: "JSON 格式化、验证与 JSONPath 查询",
        keywords: &["json", "格式化", "format", "parse", "解析", "query"],
        background: false,
        visual: PluginVisualSpec {
            icon: "icons/json.svg",
            accent: PluginAccent::Green,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.82, 0.84),
        },
        stats: PluginStats {
            primary: "格式化",
            secondary: "JSONPath",
            tertiary: "serde_json",
        },
        command_hint: "双栏输入/输出与 JSONPath 查询",
        command_prefixes: &["json", "jq"],
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
        assert_eq!(manifest.id, "json-parser");
    }

    #[test]
    fn manifest_has_correct_name() {
        let manifest = manifest();
        assert_eq!(manifest.name, "JSON 解析");
    }

    #[test]
    fn manifest_has_correct_accent() {
        let manifest = manifest();
        assert_eq!(manifest.visual.accent, PluginAccent::Green);
    }

    #[test]
    fn manifest_has_correct_category() {
        let manifest = manifest();
        assert_eq!(manifest.visual.category, PluginCategory::Tool);
    }

    #[test]
    fn manifest_has_prefixes() {
        let manifest = manifest();
        assert!(manifest.command_prefixes.contains(&"json"));
        assert!(manifest.command_prefixes.contains(&"jq"));
    }

    #[test]
    fn manifest_not_background() {
        let manifest = manifest();
        assert!(!manifest.background);
    }
}
