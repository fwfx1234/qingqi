use gpui::{Hsla, Pixels, Rgba, hsla, px, rgb};

use qingqi_plugin::plugin_spec::PluginAccent;

// ── Spacing ──────────────────────────────────────────────────────────────

pub fn space_0p5() -> Pixels {
    px(2.0)
}
pub fn space_1() -> Pixels {
    px(4.0)
}
pub fn space_1p5() -> Pixels {
    px(6.0)
}
pub fn space_2() -> Pixels {
    px(8.0)
}
pub fn space_2p5() -> Pixels {
    px(10.0)
}
pub fn space_3() -> Pixels {
    px(12.0)
}
pub fn space_4() -> Pixels {
    px(16.0)
}
pub fn space_5() -> Pixels {
    px(20.0)
}
pub fn space_6() -> Pixels {
    px(24.0)
}

// ── Border Radii ─────────────────────────────────────────────────────────

pub fn radius_xs() -> Pixels {
    px(4.0)
}
pub fn radius_sm() -> Pixels {
    px(6.0)
}
pub fn radius_md() -> Pixels {
    px(8.0)
}
pub fn radius_lg() -> Pixels {
    px(10.0)
}
pub fn radius_xl() -> Pixels {
    px(12.0)
}
pub fn radius_xxl() -> Pixels {
    px(16.0)
}
pub fn radius_sheet() -> Pixels {
    px(18.0)
}

// ── Font Sizes ───────────────────────────────────────────────────────────

pub fn font_size_title() -> Pixels {
    px(20.0)
}
pub fn font_size_heading() -> Pixels {
    px(15.0)
}
pub fn font_size_body() -> Pixels {
    px(13.0)
}
pub fn font_size_mono() -> Pixels {
    px(12.0)
}
pub fn font_size_nav() -> Pixels {
    px(13.0)
}
pub fn font_size_caption() -> Pixels {
    px(11.0)
}

// ── HTTP Method Colors ───────────────────────────────────────────────────

pub fn http_method_color(method: &str, dark: bool) -> Rgba {
    match method {
        "GET" => {
            if dark {
                rgb(0x34d399)
            } else {
                rgb(0x10b981)
            }
        }
        "POST" => {
            if dark {
                rgb(0xfbbf24)
            } else {
                rgb(0xf59e0b)
            }
        }
        "PUT" => {
            if dark {
                rgb(0xfb923c)
            } else {
                rgb(0xf97316)
            }
        }
        "DELETE" => {
            if dark {
                rgb(0xf87171)
            } else {
                rgb(0xef4444)
            }
        }
        "PATCH" => {
            if dark {
                rgb(0xc084fc)
            } else {
                rgb(0xa855f7)
            }
        }
        _ => {
            if dark {
                rgb(0x94a3b8)
            } else {
                rgb(0x64748b)
            }
        }
    }
}

// ── Accent color mapping ─────────────────────────────────────────────────

pub fn accent_color(accent: PluginAccent) -> Rgba {
    match accent {
        PluginAccent::Blue => rgb(0x3b82f6),
        PluginAccent::Cyan => rgb(0x0ea5e9),
        PluginAccent::Green => rgb(0x16a34a),
        PluginAccent::Purple => rgb(0x8b5cf6),
        PluginAccent::Amber => rgb(0xf59e0b),
        PluginAccent::Rose => rgb(0xf43f5e),
        PluginAccent::Slate => rgb(0x64748b),
    }
}

pub fn accent_soft(accent: PluginAccent) -> Rgba {
    match accent {
        PluginAccent::Blue => rgb(0xdbeafe),
        PluginAccent::Cyan => rgb(0xcffafe),
        PluginAccent::Green => rgb(0xdcfce7),
        PluginAccent::Purple => rgb(0xede9fe),
        PluginAccent::Amber => rgb(0xfef3c7),
        PluginAccent::Rose => rgb(0xffe4e6),
        PluginAccent::Slate => rgb(0xe2e8f0),
    }
}

pub fn accent_soft_dark(accent: PluginAccent) -> Rgba {
    match accent {
        PluginAccent::Blue => rgb(0x1e3a5f),
        PluginAccent::Cyan => rgb(0x164e63),
        PluginAccent::Green => rgb(0x14532d),
        PluginAccent::Purple => rgb(0x3b0764),
        PluginAccent::Amber => rgb(0x451a03),
        PluginAccent::Rose => rgb(0x4c0519),
        PluginAccent::Slate => rgb(0x1e293b),
    }
}

// ── Helpers for rgba → hsla (for GPUI alpha compositing) ──────────────

pub fn rgba_with_alpha(color: Rgba, alpha: f32) -> Hsla {
    let r = color.r;
    let g = color.g;
    let b = color.b;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return hsla(0.0, 0.0, l, alpha);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + (if g < b { 6.0 } else { 0.0 })
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    hsla(h / 6.0, s, l, alpha)
}
