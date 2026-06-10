//! macOS 毛玻璃公共组件
//!
//! 提供统一的玻璃风格 API，用于实现 macOS 毛玻璃效果。
//! 这些函数返回 Hsla 颜色值，可直接用于 gpui 的 bg、border 等方法。

use crate::theme::{self, semantic};
use gpui::{BoxShadow, Hsla, hsla, point, px};

/// 主面板背景色
pub fn bg(dark: bool) -> Hsla {
    if dark {
        theme::rgba_with_alpha(semantic().bg_surface, 0.22)
    } else {
        theme::rgba_with_alpha(theme::white(), 0.82)
    }
}

/// 面板边框色
pub fn border(dark: bool) -> Hsla {
    theme::rgba_with_alpha(semantic().border_default, if dark { 0.28 } else { 0.24 })
}

/// 双层阴影效果
pub fn shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: theme::rgba_with_alpha(semantic().shadow, 0.10),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(42.0),
            spread_radius: px(-18.0),
        },
        BoxShadow {
            color: theme::rgba_with_alpha(semantic().shadow, 0.06),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(14.0),
            spread_radius: px(0.0),
        },
    ]
}

/// 分割线颜色
pub fn divider(dark: bool) -> Hsla {
    theme::rgba_with_alpha(semantic().border_default, if dark { 0.20 } else { 0.16 })
}

/// 悬停背景色
pub fn hover_bg(dark: bool) -> Hsla {
    if dark {
        hsla(0.0, 0.0, 1.0, 0.055)
    } else {
        hsla(0.0, 0.0, 0.88, 0.34)
    }
}

/// 子面板背景色
pub fn panel(dark: bool) -> Hsla {
    if dark {
        theme::rgba_with_alpha(semantic().bg_elevated, 0.18)
    } else {
        theme::rgba_with_alpha(theme::white(), 0.78)
    }
}

/// 凹陷区域背景色（如编辑器、响应内容区）
pub fn inset(dark: bool) -> Hsla {
    if dark {
        hsla(225.0 / 360.0, 0.18, 0.10, 0.18)
    } else {
        theme::rgba_with_alpha(theme::white(), 0.50)
    }
}

/// 工具栏/标签栏背景色
pub fn bar(dark: bool) -> Hsla {
    if dark {
        hsla(225.0 / 360.0, 0.16, 0.14, 0.26)
    } else {
        theme::rgba_with_alpha(theme::white(), 0.68)
    }
}
