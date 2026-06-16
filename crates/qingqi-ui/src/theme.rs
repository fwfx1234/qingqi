use gpui::{Hsla, Pixels, Rgba, hsla, px, rgb};

use qingqi_plugin::plugin_spec::PluginAccent;

// ── Color Palette (matching suishou Theme.qml colors exactly) ──────────────
// Using functions instead of const since rgb() isn't const in GPUI 0.2.2

pub fn slate_50() -> Rgba {
    rgb(0xf8fafc)
}
pub fn slate_100() -> Rgba {
    rgb(0xf1f5f9)
}
pub fn slate_200() -> Rgba {
    rgb(0xe2e8f0)
}
pub fn slate_400() -> Rgba {
    rgb(0x94a3b8)
}
pub fn slate_500() -> Rgba {
    rgb(0x64748b)
}
pub fn slate_600() -> Rgba {
    rgb(0x475569)
}
pub fn slate_700() -> Rgba {
    rgb(0x334155)
}
pub fn slate_800() -> Rgba {
    rgb(0x1e293b)
}
pub fn slate_900() -> Rgba {
    rgb(0x0f172a)
}

pub fn blue_50() -> Rgba {
    rgb(0xeff6ff)
}
pub fn blue_100() -> Rgba {
    rgb(0xdbeafe)
}
pub fn blue_200() -> Rgba {
    rgb(0xbfdbfe)
}
pub fn blue_300() -> Rgba {
    rgb(0x93c5fd)
}
pub fn blue_400() -> Rgba {
    rgb(0x60a5fa)
}
pub fn blue_500() -> Rgba {
    rgb(0x0a84ff)
}
pub fn blue_600() -> Rgba {
    rgb(0x007aff)
}
pub fn blue_700() -> Rgba {
    rgb(0x0066cc)
}

pub fn violet_50() -> Rgba {
    rgb(0xf5f3ff)
}
pub fn violet_100() -> Rgba {
    rgb(0xede9fe)
}
pub fn violet_300() -> Rgba {
    rgb(0xc4b5fd)
}
pub fn violet_500() -> Rgba {
    rgb(0x8b5cf6)
}
pub fn violet_600() -> Rgba {
    rgb(0x7c3aed)
}
pub fn violet_700() -> Rgba {
    rgb(0x6d28d9)
}

pub fn green_400() -> Rgba {
    rgb(0x4ade80)
}
pub fn green_500() -> Rgba {
    rgb(0x16a34a)
}
pub fn green_600() -> Rgba {
    rgb(0x10b981)
}

pub fn amber_400() -> Rgba {
    rgb(0xfbbf24)
}
pub fn amber_500() -> Rgba {
    rgb(0xf59e0b)
}
pub fn amber_600() -> Rgba {
    rgb(0xd97706)
}

pub fn red_400() -> Rgba {
    rgb(0xf87171)
}
pub fn red_500() -> Rgba {
    rgb(0xef4444)
}
pub fn red_600() -> Rgba {
    rgb(0xdc2626)
}

pub fn cyan_300() -> Rgba {
    rgb(0x67e8f9)
}
pub fn cyan_400() -> Rgba {
    rgb(0x22d3ee)
}
pub fn cyan_500() -> Rgba {
    rgb(0x0ea5e9)
}

pub fn white() -> Rgba {
    rgb(0xffffff)
}

// ── Spacing (matching suishou Theme.qml space) ──────────────────────────

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

// ── Border Radii (matching suishou Theme.qml radii) ─────────────────────

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

// ── Font Sizes (matching suishou Theme.qml fontSize) ─────────────────────

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

// ── Semantic Color Tokens (compile-time safe, replaces string-based lookup) ──

/// Layer 2: Semantic UI color tokens.
///
/// Only tokens that are **truly cross-component** and **have light/dark mode
/// differences** belong here (e.g. `bg_surface`, `text_primary`, `danger`).
/// Component-specific colors live in Layer 3 contextual functions below
/// (e.g. `http_method_color()`, `launcher_glass()`).
///
/// Access via `theme::semantic().field` for compile-time safety.
pub struct SemanticColors {
    pub bg_page: Rgba,
    pub bg_surface: Rgba,
    pub bg_elevated: Rgba,
    pub bg_subtle: Rgba,
    pub bg_subtle_2: Rgba,
    pub bg_glass: Hsla,
    pub bg_hover: Rgba,
    pub border_default: Rgba,
    pub border_strong: Rgba,
    pub text_primary: Rgba,
    pub text_body: Rgba,
    pub text_secondary: Rgba,
    pub text_placeholder: Rgba,
    pub primary: Rgba,
    pub primary_hover: Rgba,
    pub primary_active: Rgba,
    pub primary_bg: Rgba,
    pub primary_soft: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub info: Rgba,
    pub shadow: Rgba,
    pub overlay_backdrop: Hsla,
}

fn build_light() -> SemanticColors {
    SemanticColors {
        bg_page: rgb(0xf5f6f8),
        bg_surface: rgb(0xffffff),
        bg_elevated: rgb(0xffffff),
        bg_subtle: rgb(0xeef1f5),
        bg_subtle_2: rgb(0xf8f9fb),
        bg_glass: hsla(0.0, 0.0, 1.0, 0.98),
        bg_hover: rgb(0xeef4fb),
        border_default: rgb(0xd7dce5),
        border_strong: rgb(0xb8c0cc),
        text_primary: rgb(0x1d1d1f),
        text_body: rgb(0x3a3a3c),
        text_secondary: rgb(0x8a8f98),
        text_placeholder: rgb(0xa5acb8),
        primary: rgb(0x0a84ff),
        primary_hover: rgb(0x3398ff),
        primary_active: rgb(0x007aff),
        primary_bg: rgb(0xe8f3ff),
        primary_soft: rgb(0xd6e9ff),
        success: rgb(0x10b981),
        warning: rgb(0xf59e0b),
        danger: rgb(0xff3b30),
        info: rgb(0x0ea5e9),
        shadow: rgb(0x000000),
        overlay_backdrop: hsla(0.0, 0.0, 0.0, 0.24),
    }
}

fn build_dark() -> SemanticColors {
    SemanticColors {
        bg_page: rgb(0x101216),
        bg_surface: rgb(0x1b1e24),
        bg_elevated: rgb(0x242830),
        bg_subtle: rgb(0x252a33),
        bg_subtle_2: rgb(0x181b21),
        bg_glass: hsla(0.0, 0.0, 0.0, 0.30),
        bg_hover: rgb(0x222a35),
        border_default: rgb(0x3a414d),
        border_strong: rgb(0x596171),
        text_primary: rgb(0xf5f5f7),
        text_body: rgb(0xd8dce3),
        text_secondary: rgb(0x989faa),
        text_placeholder: rgb(0x7f8793),
        primary: rgb(0x0a84ff),
        primary_hover: rgb(0x3398ff),
        primary_active: rgb(0x0a84ff),
        primary_bg: rgb(0x0d2a45),
        primary_soft: rgb(0x143b5f),
        success: rgb(0x10b981),
        warning: rgb(0xf59e0b),
        danger: rgb(0xff453a),
        info: rgb(0x0ea5e9),
        shadow: rgb(0x000000),
        overlay_backdrop: hsla(0.0, 0.0, 0.0, 0.44),
    }
}

/// Get the semantic color set for the current theme mode.
#[inline(always)]
pub fn semantic() -> SemanticColors {
    if crate::theme_mode::is_dark() {
        build_dark()
    } else {
        build_light()
    }
}

// ── Layer 3: HTTP Method Colors ─────────────────────────

pub fn http_method_color(method: &str) -> Rgba {
    let dark = crate::theme_mode::is_dark();
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
                slate_400()
            } else {
                slate_500()
            }
        }
    }
}

// ── Layer 3: Terminal Colors ────────────────────────────

pub fn terminal_bg() -> Rgba {
    if crate::theme_mode::is_dark() {
        rgb(0x0b1118)
    } else {
        rgb(0x111827)
    }
}
pub fn terminal_fg() -> Rgba {
    if crate::theme_mode::is_dark() {
        rgb(0xd7e2ee)
    } else {
        rgb(0xe5e7eb)
    }
}
pub fn terminal_muted() -> Rgba {
    if crate::theme_mode::is_dark() {
        rgb(0x7dd3fc)
    } else {
        rgb(0xbfdbfe)
    }
}
pub fn terminal_border() -> Rgba {
    if crate::theme_mode::is_dark() {
        rgb(0x1f2937)
    } else {
        rgb(0x374151)
    }
}

// ── Layer 3: Keycap background ─────────────────────────

pub fn keycap_bg() -> Hsla {
    if crate::theme_mode::is_dark() {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        hsla(0.0, 0.0, 0.0, 0.04)
    }
}

// ── Layer 3: Launcher colors ───────────────────────────

/// Row hover background used in launcher item hover states.
pub fn launcher_row_selected() -> Rgba {
    if crate::theme_mode::is_dark() {
        rgb(0x241b48)
    } else {
        rgb(0xf7f7fa)
    }
}

/// Icon surface background (unselected state).  Returns a colour with
/// alpha so it composes correctly on top of the launcher glass.
pub fn launcher_icon_surface() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        rgba_with_alpha(rgb(0xf8f8fb), 0.78)
    }
}

/// Icon border (unselected state).
pub fn launcher_icon_border() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.04)
    } else {
        rgba_with_alpha(rgb(0xe7e7ee), 0.72)
    }
}

pub fn launcher_accent() -> Rgba {
    let dark = crate::theme_mode::is_dark();
    if dark { rgb(0xc8b8ff) } else { rgb(0x6b4fcf) }
}

pub fn launcher_glass() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        rgba_with_alpha(rgb(0x0f0f23), 0.3)
    } else {
        rgba_with_alpha(rgb(0xffffff), 0.98)
    }
}

pub fn launcher_soft_line() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.04)
    } else {
        rgba_with_alpha(rgb(0xe6e6eb), 0.9)
    }
}

pub fn launcher_muted_text() -> Rgba {
    let dark = crate::theme_mode::is_dark();
    if dark { rgb(0x7777aa) } else { rgb(0x8888aa) }
}

pub fn launcher_faint_text() -> Rgba {
    let dark = crate::theme_mode::is_dark();
    if dark { rgb(0x55557a) } else { rgb(0x9999bb) }
}

/// Title text in result/list rows.
pub fn launcher_title_text() -> Rgba {
    let dark = crate::theme_mode::is_dark();
    if dark { rgb(0xddd8ec) } else { rgb(0x333348) }
}

/// Icon border in selected state.
pub fn launcher_icon_border_selected() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        rgba_with_alpha(rgb(0xe2e2ea), 0.2)
    } else {
        rgba_with_alpha(rgb(0xe2e2ea), 0.9)
    }
}

/// Icon surface in selected state.
pub fn launcher_icon_surface_selected() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        rgba_with_alpha(rgb(0xf2f2f7), 0.15)
    } else {
        rgba_with_alpha(rgb(0xf2f2f7), 0.9)
    }
}

/// Row hover background.
pub fn launcher_row_hover() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.025)
    } else {
        rgba_with_alpha(rgb(0xf7f7fa), 0.72)
    }
}

/// Badge / tag background.
pub fn launcher_badge_bg() -> Hsla {
    let dark = crate::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        rgba_with_alpha(rgb(0xf7f7fa), 0.82)
    }
}

/// Selected row background in light mode.
pub fn launcher_row_bg_selected_light() -> Hsla {
    rgba_with_alpha(rgb(0xf6f6fa), 0.96)
}

/// Selected row border in light mode.
pub fn launcher_row_border_selected_light() -> Hsla {
    rgba_with_alpha(rgb(0xe2e2ea), 0.95)
}

/// Subtle glow behind the selected row in dark mode.
pub fn launcher_row_glow_dark() -> Hsla {
    hsla(0.72, 0.72, 0.56, 0.04)
}

/// Fully transparent background placeholder (used for unselected row default).
pub fn launcher_transparent() -> Hsla {
    hsla(0.0, 0.0, 0.0, 0.0)
}

/// Plugin-specific icon tint color in the launcher.
pub fn launcher_plugin_icon_tint(plugin_id: &str) -> Rgba {
    let dark = crate::theme_mode::is_dark();
    if dark {
        match plugin_id {
            "api-debugger" => rgb(0xc8b8ff),
            "clipboard" => rgb(0x88dd88),
            "http-capture" => rgb(0xff8888),
            "image-compress" => rgb(0xffcc44),
            "json-parser" => rgb(0xaaccff),
            "ftp-sftp-ssh-client" => rgb(0x88ddff),
            "system-settings" => rgb(0xaaccff),
            _ => launcher_accent(),
        }
    } else {
        match plugin_id {
            "api-debugger" => rgb(0x6b4fcf),
            "clipboard" => rgb(0x55aa55),
            "http-capture" => rgb(0xcc6666),
            "image-compress" => rgb(0xccaa33),
            "json-parser" => rgb(0x6688cc),
            "ftp-sftp-ssh-client" => rgb(0x5599cc),
            "system-settings" => rgb(0x6688cc),
            _ => launcher_accent(),
        }
    }
}

// ── Accent color mapping ───────────────────────────────────────────────

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

pub fn accent_hover(accent: PluginAccent) -> Rgba {
    match accent {
        PluginAccent::Blue => rgb(0x2563eb),
        PluginAccent::Cyan => rgb(0x0284c7),
        PluginAccent::Green => rgb(0x15803d),
        PluginAccent::Purple => rgb(0x7c3aed),
        PluginAccent::Amber => rgb(0xd97706),
        PluginAccent::Rose => rgb(0xe11d48),
        PluginAccent::Slate => rgb(0x475569),
    }
}

// ── Helpers for rgba → hsla (for GPUI alpha compositing) ────────────────

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
