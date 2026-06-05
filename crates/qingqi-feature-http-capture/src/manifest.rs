use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "http-capture";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "HTTP 抓包".into(),
        description: "HTTP 请求捕获与分析".into(),
        keywords: ["抓包", "capture", "http", "https", "proxy", "代理", "请求"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/capture.svg"),
        prefixes: vec!["cap".into(), "capture".into(), "httpcap".into()],
        mode: PluginWindowMode::Window,
        window: WindowSpec::ratio(0.86, 0.82),
        category: PluginCategory::Tool,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/capture.svg"),
            accent: PluginAccent::Cyan,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.86, 0.82),
        }),
        stats: Some(PluginStats {
            primary: "代理抓包".into(),
            secondary: "请求筛选".into(),
            tertiary: "HTTPS MITM".into(),
        }),
        command_hint: Some("启动代理、观察流量、按方法/域名/状态过滤".into()),
        command_prefixes: ["cap", "capture", "httpcap"]
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_is_http_capture() {
        assert_eq!(PLUGIN_ID, "http-capture");
    }

    #[test]
    fn manifest_has_correct_id() {
        let manifest = manifest();
        assert_eq!(manifest.id.as_ref(), "http-capture");
    }

    #[test]
    fn manifest_has_correct_name() {
        let manifest = manifest();
        assert_eq!(manifest.name.as_ref(), "HTTP 抓包");
    }

    #[test]
    fn manifest_has_correct_accent() {
        let manifest = manifest();
        assert_eq!(manifest.visual.as_ref().unwrap().accent, PluginAccent::Cyan);
    }

    #[test]
    fn manifest_is_not_background() {
        let manifest = manifest();
        assert!(!manifest.background);
    }

    #[test]
    fn manifest_has_prefixes() {
        let manifest = manifest();
        assert!(
            manifest
                .command_prefixes
                .iter()
                .any(|p| p.as_ref() == "cap")
        );
        assert!(
            manifest
                .command_prefixes
                .iter()
                .any(|p| p.as_ref() == "capture")
        );
        assert!(
            manifest
                .command_prefixes
                .iter()
                .any(|p| p.as_ref() == "httpcap")
        );
    }
}
