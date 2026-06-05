use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "quick-launch";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "快速启动".into(),
        description: "系统命令与常用动作快速执行".into(),
        keywords: [
            "快速", "启动", "脚本", "命令", "quick", "launch", "系统", "命令",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        icon: IconRef::asset("icons/bolt.svg"),
        prefixes: vec!["ql".into(), "quick".into()],
        mode: PluginWindowMode::Window,
        window: WindowSpec::fixed(860.0, 620.0),
        category: PluginCategory::Tool,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: true,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/bolt.svg"),
            accent: PluginAccent::Amber,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::fixed(860.0, 620.0),
        }),
        stats: Some(PluginStats {
            primary: "动作仓库".into(),
            secondary: "运行记录".into(),
            tertiary: "seed data 已接入".into(),
        }),
        command_hint: Some("系统命令与常用动作快速执行".into()),
        command_prefixes: ["ql", "quick"].into_iter().map(Into::into).collect(),
    }
}
