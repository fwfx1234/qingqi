/// Show a system notification for download completion
pub fn notify_download_complete(file_name: &str, save_path: &str) {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"display notification "文件已保存到: {}" with title "Qingqi 下载完成" subtitle "{}""#,
            save_path, file_name
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
                     $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
                     $text = $template.GetElementsByTagName('text'); \
                     $text[0].AppendChild($template.CreateTextNode('Qingqi 下载完成')); \
                     $text[1].AppendChild($template.CreateTextNode('{}')); \
                     $toast = [Windows.UI.Notifications.ToastNotification]::new($template); \
                     [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Qingqi').Show($toast)",
                    file_name
                )
            ])
            .output();
    }
}

/// Notify when a download fails
pub fn notify_download_failed(file_name: &str, error: &str) {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"display notification "{}" with title "Qingqi 下载失败" subtitle "{}""#,
            error, file_name
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();
    }
}
