use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{PluginCategory, PluginStatus, ViewMode, WindowSpec},
};

pub const PLUGIN_ID: &str = "remote-control";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "远程控制".into(),
        description: "通过手机在局域网内控制电脑：关机、休眠、进程管理等".into(),
        keywords: ["远程控制", "remote", "control", "关机", "休眠", "进程"]
            .into_iter()
            .map(Into::into)
            .collect(),
        background: true,
        dynamic_commands: false,
        has_settings: false,
        icon: IconRef::asset("icons/remote.svg"),
        mode: ViewMode::Window,
        window: WindowSpec::fixed(900.0, 640.0),
        category: PluginCategory::Tool,
        status: PluginStatus::Background,
        prefixes: ["remote", "rc"].into_iter().map(Into::into).collect(),
        visual: None,
        stats: None,
        command_hint: None,
        command_prefixes: Vec::new(),
    }
}
