use crate::core::{
    plugin::PluginManifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "ftp-sftp-ssh-client";

pub fn manifest() -> PluginManifest {
    PluginManifest {
        id: PLUGIN_ID,
        name: "FTP/SFTP/SSH 客户端",
        description: "多 session 的 FTP、SFTP、SSH 远程工作台，支持协议日志与终端",
        keywords: &["ftp", "sftp", "ftps", "ssh", "远程", "文件", "服务器"],
        background: false,
        visual: PluginVisualSpec {
            icon: "qta/mdi6.folder-network-outline.png",
            accent: PluginAccent::Purple,
            category: PluginCategory::Tool,
            status: PluginStatus::Preview,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio(0.86, 0.82),
        },
        stats: PluginStats {
            primary: "多 Session",
            secondary: "文件与终端",
            tertiary: "文本回传",
        },
        command_hint: "多连接切换、远程文件区、SSH 终端与 FTP 命令日志",
        command_prefixes: &["ftp", "sftp", "ssh"],
    }
}
