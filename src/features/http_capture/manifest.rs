use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "http-capture";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "HTTP 抓包",
        description: "HTTP 请求捕获与分析",
        keywords: &["抓包", "capture", "http", "https", "proxy", "代理", "请求"],
        background: true,
        visual: PluginVisualSpec {
            icon: "icons/capture.svg",
            accent: PluginAccent::Cyan,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.86, 0.82),
        },
        stats: PluginStats {
            primary: "代理抓包",
            secondary: "请求筛选",
            tertiary: "HTTPS MITM",
        },
        command_hint: "启动代理、观察流量、按方法/域名/状态过滤",
        command_prefixes: &["cap", "capture", "httpcap"],
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
        assert_eq!(manifest.id, "http-capture");
    }

    #[test]
    fn manifest_has_correct_name() {
        let manifest = manifest();
        assert_eq!(manifest.name, "HTTP 抓包");
    }

    #[test]
    fn manifest_has_correct_accent() {
        let manifest = manifest();
        assert_eq!(manifest.visual.accent, PluginAccent::Cyan);
    }

    #[test]
    fn manifest_is_background() {
        let manifest = manifest();
        assert!(manifest.background);
    }

    #[test]
    fn manifest_has_prefixes() {
        let manifest = manifest();
        assert!(manifest.command_prefixes.contains(&"cap"));
        assert!(manifest.command_prefixes.contains(&"capture"));
        assert!(manifest.command_prefixes.contains(&"httpcap"));
    }
}
