//! macOS 毛玻璃公共组件
use gpui::{hsla, point, px, BoxShadow, Hsla};
use qingqi_ui::theme::{self, semantic};

pub fn bg(dark: bool) -> Hsla {
    if dark { theme::rgba_with_alpha(semantic().bg_surface, 0.78) }
    else { theme::rgba_with_alpha(theme::white(), 0.86) }
}
pub fn border(dark: bool) -> Hsla {
    theme::rgba_with_alpha(semantic().border_default, if dark { 0.54 } else { 0.72 })
}
pub fn shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow { color: theme::rgba_with_alpha(semantic().shadow, 0.08), offset: point(px(0.0), px(8.0)), blur_radius: px(24.0), spread_radius: px(-8.0) },
        BoxShadow { color: theme::rgba_with_alpha(semantic().shadow, 0.04), offset: point(px(0.0), px(2.0)), blur_radius: px(6.0), spread_radius: px(0.0) },
    ]
}
pub fn tint(dark: bool) -> Hsla {
    if dark { hsla(220.0/360.0, 0.16, 0.08, 1.0) }
    else { hsla(220.0/360.0, 0.36, 0.97, 1.0) }
}
pub fn divider(dark: bool) -> Hsla {
    theme::rgba_with_alpha(semantic().border_default, if dark { 0.20 } else { 0.16 })
}
pub fn hover_bg(dark: bool) -> Hsla {
    if dark { hsla(1.0, 1.0, 1.0, 0.06) } else { hsla(0.0, 0.0, 0.0, 0.04) }
}
