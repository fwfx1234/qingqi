use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{PluginCategory, PluginStatus, ViewMode, WindowSpec},
};

pub const PLUGIN_ID: &str = "clipboard";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "剪贴板".into(),
        description: "搜索、复制和管理剪贴板文本历史".into(),
        keywords: [
            "剪贴板",
            "clipboard",
            "copy",
            "paste",
            "历史",
            "复制",
            "粘贴",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        background: true,
        dynamic_commands: false,
        has_settings: false,
        icon: IconRef::asset("icons/clipboard.svg"),
        mode: ViewMode::Window,
        window: WindowSpec::fixed_topmost(1024.0, 558.0),
        category: PluginCategory::Tool,
        status: PluginStatus::Background,
        prefixes: ["clip", "clipboard"].into_iter().map(Into::into).collect(),
        visual: None,
        stats: None,
        command_hint: None,
        command_prefixes: Vec::new(),
    }
}
