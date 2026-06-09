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
        name: "远程管理工作区".into(),
        description: "多会话远程管理工作区。支持 SSH 全功能终端、SFTP/FTP/FTPS 文件浏览与传输、下载编辑回传。".into(),
        keywords: ["ssh", "sftp", "ftp", "ftps", "远程", "文件", "服务器", "终端", "管理"]
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
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/folder-network.svg"),
            accent: PluginAccent::Purple,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio_blurred(0.86, 0.82),
        }),
        stats: Some(PluginStats {
            primary: "多会话标签页".into(),
            secondary: "终端 + 文件传输".into(),
            tertiary: "下载编辑回传".into(),
        }),
        command_hint: Some("多 session 工作区、SSH 终端、SFTP/FTP/FTPS 文件浏览、上传下载与编辑回传".into()),
        command_prefixes: ["ftp", "sftp", "ssh"].into_iter().map(Into::into).collect(),
    }
}
