use qingqi_ui::ui;

pub fn workspace_chrome_config() -> ui::WindowChromeConfig {
    ui::WindowChromeConfig::new()
        .title("远程管理工作区")
        .titlebar_slot_alignment(ui::WindowChromeTitlebarSlotAlignment::Leading)
        .transparent(true)
}
