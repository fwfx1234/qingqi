//! macOS 原生交通灯占位组件。
//!
//! macOS 窗口使用 `appears_transparent: true` + `WindowDecorations::Client` 时，
//! 系统会在标题栏左上角保留红/黄/绿三个交通灯按钮。
//! 使用此组件在标题栏左侧占位 86px，避免内容被交通灯遮挡。

use gpui::{InteractiveElement, Styled, WindowControlArea, div, px};

/// macOS 交通灯占位区域（86px 宽，可拖拽）。
/// 非 macOS 返回空元素。
pub fn macos_traffic_lights() -> gpui::Div {
    if cfg!(target_os = "macos") {
        div()
            .w(px(86.0))
            .h_full()
            .flex_none()
            .window_control_area(WindowControlArea::Drag)
    } else {
        div()
    }
}
