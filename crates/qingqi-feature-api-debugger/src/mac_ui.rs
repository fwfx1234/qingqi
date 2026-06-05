use qingqi_ui::ui;

pub fn workspace_chrome_config() -> ui::WindowChromeConfig {
    ui::WindowChromeConfig::new()
        .title("")
        .transparent(true)
}
