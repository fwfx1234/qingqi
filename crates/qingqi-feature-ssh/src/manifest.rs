//! 插件元数据

use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "ssh";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "远程管理".into(),
        description: "SSH/SFTP/FTP/FTPS 远程连接管理。多标签页终端与文件传输。".into(),
        keywords: [
            "ssh",
            "sftp",
            "ftp",
            "ftps",
            "远程",
            "终端",
            "文件",
            "传输",
            "服务器",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        icon: IconRef::asset("icons/folder-network.svg"),
        prefixes: vec!["ssh".into(), "sftp".into(), "ftp".into()],
        mode: PluginWindowMode::Window,
        window: WindowSpec::ratio_blurred(0.86, 0.82),
        category: PluginCategory::Tool,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        has_settings: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/folder-network.svg"),
            accent: PluginAccent::Cyan,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio_blurred(0.86, 0.82),
        }),
        stats: Some(PluginStats {
            primary: "多会话标签页".into(),
            secondary: "终端 + 文件传输".into(),
            tertiary: "SSH/SFTP/FTP/FTPS".into(),
        }),
        command_hint: Some("SSH 终端、SFTP/FTP 文件浏览、上传下载".into()),
        command_prefixes: ["ssh", "sftp", "ftp"].into_iter().map(Into::into).collect(),
    }
}
