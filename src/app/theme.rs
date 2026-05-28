use gpui::{Hsla, Pixels, Rgba, hsla, px, rgb};

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

pub fn green_500() -> Rgba {
    rgb(0x16a34a)
}
pub fn green_600() -> Rgba {
    rgb(0x10b981)
}

pub fn amber_500() -> Rgba {
    rgb(0xf59e0b)
}
pub fn amber_600() -> Rgba {
    rgb(0xd97706)
}

pub fn red_500() -> Rgba {
    rgb(0xef4444)
}
pub fn red_600() -> Rgba {
    rgb(0xdc2626)
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

// ── Light Mode Semantic Tokens (matching suishou Theme.qml tokensLight) ──

pub fn token_light(name: &str) -> Rgba {
    match name {
        "color-bg-page" => rgb(0xf5f6f8),
        "color-bg-surface" => rgb(0xffffff),
        "color-bg-elevated" => rgb(0xffffff),
        "color-bg-subtle" => rgb(0xeef1f5),
        "color-bg-subtle-2" => rgb(0xf8f9fb),
        "color-border-default" => rgb(0xd7dce5),
        "color-border-strong" => rgb(0xb8c0cc),
        "color-text-primary" => rgb(0x1d1d1f),
        "color-text-regular" => rgb(0x3a3a3c),
        "color-text-secondary" => rgb(0x8a8f98),
        "color-text-placeholder" => rgb(0xa5acb8),
        "color-primary" => rgb(0x0a84ff),
        "color-primary-hover" => rgb(0x3398ff),
        "color-primary-active" => rgb(0x007aff),
        "color-primary-bg" => rgb(0xe8f3ff),
        "color-primary-soft" => rgb(0xd6e9ff),
        "color-success" => rgb(0x10b981),
        "color-warning" => rgb(0xf59e0b),
        "color-danger" => rgb(0xff3b30),
        "color-info" => rgb(0x0ea5e9),
        "color-nav-idle" => rgb(0x334155),
        "color-nav-active-bg" => rgb(0xd6e9ff),
        "color-nav-item-active-bg" => rgb(0xe8f3ff),
        "color-nav-active-text" => rgb(0x0066cc),
        "color-nav-icon-idle-bg" => rgb(0xf1f5f9),
        "color-nav-icon-active-bg" => rgb(0x0a84ff),
        "color-nav-icon-active-bg-soft" => rgb(0xd6e9ff),
        "color-method-get" => rgb(0x16a34a),
        "color-method-post" => rgb(0xf59e0b),
        "color-method-put" => rgb(0x3b82f6),
        "color-method-delete" => rgb(0xef4444),
        "color-method-patch" => rgb(0x0ea5e9),
        "color-table-header" => rgb(0xfafafb),
        "color-row-hover" => rgb(0xeef4fb),
        "color-row-selected" => rgb(0xddeeff),
        "color-status-bar-bg" => rgb(0xf7f8fa),
        "color-shadow" => rgb(0x000000),
        _ => rgb(0x000000),
    }
}

// ── Dark Mode Semantic Tokens (matching suishou Theme.qml tokensDark) ────

pub fn token_dark(name: &str) -> Rgba {
    match name {
        "color-bg-page" => rgb(0x101216),
        "color-bg-surface" => rgb(0x1b1e24),
        "color-bg-elevated" => rgb(0x242830),
        "color-bg-subtle" => rgb(0x252a33),
        "color-bg-subtle-2" => rgb(0x181b21),
        "color-border-default" => rgb(0x3a414d),
        "color-border-strong" => rgb(0x596171),
        "color-text-primary" => rgb(0xf5f5f7),
        "color-text-regular" => rgb(0xd8dce3),
        "color-text-secondary" => rgb(0x989faa),
        "color-text-placeholder" => rgb(0x7f8793),
        "color-primary" => rgb(0x0a84ff),
        "color-primary-hover" => rgb(0x3398ff),
        "color-primary-active" => rgb(0x0a84ff),
        "color-primary-bg" => rgb(0x0d2a45),
        "color-primary-soft" => rgb(0x143b5f),
        "color-success" => rgb(0x10b981),
        "color-warning" => rgb(0xf59e0b),
        "color-danger" => rgb(0xff453a),
        "color-info" => rgb(0x0ea5e9),
        "color-nav-idle" => rgb(0x94a3b8),
        "color-nav-active-bg" => rgb(0x0a84ff),
        "color-nav-item-active-bg" => rgb(0x0d2a45),
        "color-nav-active-text" => rgb(0xe8f3ff),
        "color-nav-icon-idle-bg" => rgb(0x242a34),
        "color-nav-icon-active-bg" => rgb(0x0a84ff),
        "color-nav-icon-active-bg-soft" => rgb(0x143b5f),
        "color-method-get" => rgb(0x22c55e),
        "color-method-post" => rgb(0xfbbf24),
        "color-method-put" => rgb(0x60a5fa),
        "color-method-delete" => rgb(0xf87171),
        "color-method-patch" => rgb(0x22d3ee),
        "color-table-header" => rgb(0x0f1623),
        "color-row-hover" => rgb(0x222a35),
        "color-row-selected" => rgb(0x143b5f),
        "color-status-bar-bg" => rgb(0x171b22),
        "color-shadow" => rgb(0x000000),
        _ => rgb(0x000000),
    }
}

/// Get a semantic token value for the current theme mode.
pub fn token(name: &str, dark: bool) -> Rgba {
    if dark {
        token_dark(name)
    } else {
        token_light(name)
    }
}

// ── Launcher-specific colors (from suishou LauncherWindow.qml) ──────────

pub fn launcher_panel(dark: bool) -> Rgba {
    if dark { rgb(0x0f0f23) } else { rgb(0xf8f5ff) }
}

pub fn launcher_panel_border(dark: bool) -> Rgba {
    if dark { rgb(0x34304f) } else { rgb(0xded4ff) }
}

pub fn launcher_field(dark: bool) -> Rgba {
    if dark { rgb(0x15152c) } else { rgb(0xffffff) }
}

pub fn launcher_field_border(dark: bool) -> Rgba {
    if dark { rgb(0x2a2842) } else { rgb(0xe4dcff) }
}

pub fn launcher_row_selected(dark: bool) -> Rgba {
    if dark { rgb(0x241b48) } else { rgb(0xf7f7fa) }
}

pub fn launcher_icon_surface(dark: bool) -> Rgba {
    if dark { rgb(0x1a1a35) } else { rgb(0xf8f8fb) }
}

pub fn launcher_accent(dark: bool) -> Rgba {
    if dark { rgb(0xc8b8ff) } else { rgb(0x6b4fcf) }
}

pub fn launcher_deep_background(dark: bool) -> Rgba {
    if dark { rgb(0x0b0b1a) } else { rgb(0xf5f5f7) }
}

pub fn launcher_glass(dark: bool) -> Hsla {
    if dark {
        rgba_with_alpha(rgb(0x0f0f23), 0.3)
    } else {
        rgba_with_alpha(rgb(0xffffff), 0.98)
    }
}

pub fn launcher_glass_border(dark: bool) -> Hsla {
    if dark {
        hsla(0.0, 0.0, 1.0, 0.08)
    } else {
        rgba_with_alpha(rgb(0xffffff), 0.92)
    }
}

pub fn launcher_soft_line(dark: bool) -> Hsla {
    if dark {
        hsla(0.0, 0.0, 1.0, 0.04)
    } else {
        rgba_with_alpha(rgb(0xe6e6eb), 0.9)
    }
}

pub fn launcher_keycap(dark: bool) -> Hsla {
    if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        rgba_with_alpha(rgb(0xf8f8fb), 0.78)
    }
}

pub fn launcher_muted_text(dark: bool) -> Rgba {
    if dark { rgb(0x7777aa) } else { rgb(0x8888aa) }
}

pub fn launcher_faint_text(dark: bool) -> Rgba {
    if dark { rgb(0x55557a) } else { rgb(0x9999bb) }
}

// ── Accent color mapping ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeAccent {
    Blue,
    Cyan,
    Green,
    Purple,
    Amber,
    Rose,
    Slate,
}

pub fn accent_color(accent: ThemeAccent) -> Rgba {
    match accent {
        ThemeAccent::Blue => rgb(0x3b82f6),
        ThemeAccent::Cyan => rgb(0x0ea5e9),
        ThemeAccent::Green => rgb(0x16a34a),
        ThemeAccent::Purple => rgb(0x8b5cf6),
        ThemeAccent::Amber => rgb(0xf59e0b),
        ThemeAccent::Rose => rgb(0xf43f5e),
        ThemeAccent::Slate => rgb(0x64748b),
    }
}

pub fn accent_soft(accent: ThemeAccent) -> Rgba {
    match accent {
        ThemeAccent::Blue => rgb(0xdbeafe),
        ThemeAccent::Cyan => rgb(0xcffafe),
        ThemeAccent::Green => rgb(0xdcfce7),
        ThemeAccent::Purple => rgb(0xede9fe),
        ThemeAccent::Amber => rgb(0xfef3c7),
        ThemeAccent::Rose => rgb(0xffe4e6),
        ThemeAccent::Slate => rgb(0xe2e8f0),
    }
}

pub fn accent_soft_dark(accent: ThemeAccent) -> Rgba {
    match accent {
        ThemeAccent::Blue => rgb(0x1e3a5f),
        ThemeAccent::Cyan => rgb(0x164e63),
        ThemeAccent::Green => rgb(0x14532d),
        ThemeAccent::Purple => rgb(0x3b0764),
        ThemeAccent::Amber => rgb(0x451a03),
        ThemeAccent::Rose => rgb(0x4c0519),
        ThemeAccent::Slate => rgb(0x1e293b),
    }
}

pub fn accent_hover(accent: ThemeAccent) -> Rgba {
    match accent {
        ThemeAccent::Blue => rgb(0x2563eb),
        ThemeAccent::Cyan => rgb(0x0284c7),
        ThemeAccent::Green => rgb(0x15803d),
        ThemeAccent::Purple => rgb(0x7c3aed),
        ThemeAccent::Amber => rgb(0xd97706),
        ThemeAccent::Rose => rgb(0xe11d48),
        ThemeAccent::Slate => rgb(0x475569),
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
