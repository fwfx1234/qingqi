use gpui::{IntoElement, ParentElement, Styled, div};
use qingqi_ui::ui;

pub fn workspace_chrome_config() -> ui::WindowChromeConfig {
    ui::WindowChromeConfig::new()
        .title("API 调试器")
        .transparent(true)
}
