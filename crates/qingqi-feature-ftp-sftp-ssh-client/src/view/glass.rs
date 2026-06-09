//! macOS 毛玻璃公共组件
use gpui::{BoxShadow, Hsla, hsla, point, px};
use qingqi_ui::theme::{self, semantic};

pub fn bg(dark: bool) -> Hsla {
    if dark {
        theme::rgba_with_alpha(semantic().bg_surface, 0.22)
    } else {
        theme::rgba_with_alpha(theme::white(), 0.82)
    }
}
pub fn border(dark: bool) -> Hsla {
    theme::rgba_with_alpha(semantic().border_default, if dark { 0.22 } else { 0.13 })
}
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
pub fn divider(dark: bool) -> Hsla {
    theme::rgba_with_alpha(semantic().border_default, if dark { 0.18 } else { 0.11 })
}
pub fn hover_bg(dark: bool) -> Hsla {
    if dark {
        hsla(0.0, 0.0, 1.0, 0.055)
    } else {
        hsla(220.0 / 360.0, 0.16, 0.91, 0.56)
    }
}

pub fn panel(dark: bool) -> Hsla {
    if dark {
        theme::rgba_with_alpha(semantic().bg_elevated, 0.14)
    } else {
        theme::rgba_with_alpha(theme::white(), 0.72)
    }
}

pub fn inset(dark: bool) -> Hsla {
    if dark {
        hsla(225.0 / 360.0, 0.18, 0.10, 0.18)
    } else {
        hsla(220.0 / 360.0, 0.17, 0.948, 0.60)
    }
}

#[allow(dead_code)]
pub fn bar(dark: bool) -> Hsla {
    if dark {
        hsla(225.0 / 360.0, 0.16, 0.14, 0.26)
    } else {
        theme::rgba_with_alpha(theme::white(), 0.64)
    }
}

pub fn panel_shadow(dark: bool) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: theme::rgba_with_alpha(semantic().shadow, if dark { 0.08 } else { 0.035 }),
            offset: point(px(0.0), px(14.0)),
            blur_radius: px(30.0),
            spread_radius: px(-24.0),
        },
        BoxShadow {
            color: theme::rgba_with_alpha(semantic().shadow, if dark { 0.06 } else { 0.025 }),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(3.0),
            spread_radius: px(0.0),
        },
    ]
}
