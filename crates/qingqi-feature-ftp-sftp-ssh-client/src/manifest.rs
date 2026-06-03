use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "ftp-sftp-ssh-client";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "SSH 管理工具".into(),
        description: "多会话远程管理工具。支持 SFTP、FTP、FTPS、SSH 协议，远程文件浏览与编辑回传，SSH 终端，标签页管理。".into(),
        keywords: ["ssh", "sftp", "ftp", "ftps", "远程", "文件", "服务器", "终端", "管理"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/folder-network.svg"),
        prefixes: vec!["ssh".into(), "sftp".into(), "ftp".into()],
        mode: PluginWindowMode::Window,
        window: WindowSpec::ratio(0.86, 0.82),
        category: PluginCategory::Tool,
        status: PluginStatus::Preview,
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/folder-network.svg"),
            accent: PluginAccent::Purple,
            category: PluginCategory::Tool,
            status: PluginStatus::Preview,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.86, 0.82),
        }),
        stats: Some(PluginStats {
            primary: "多会话标签页".into(),
            secondary: "SSH 终端 + 文件管理".into(),
            tertiary: "文件编辑回传".into(),
        }),
        command_hint: Some("多标签页管理、远程文件浏览器、SSH 终端、FTP 命令日志".into()),
        command_prefixes: ["ftp", "sftp", "ssh"].into_iter().map(Into::into).collect(),
    }
}
