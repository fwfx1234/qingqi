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

// ── Semantic Color Tokens (compile-time safe, replaces string-based lookup) ──

/// All semantic UI colors for one theme mode.
/// Access via `theme::semantic().field` for compile-time safety;
/// legacy code can still use `theme::token("key", dark)` which delegates internally.
pub struct SemanticColors {
    pub bg_page: Rgba,
    pub bg_surface: Rgba,
    pub bg_elevated: Rgba,
    pub bg_subtle: Rgba,
    pub bg_subtle_2: Rgba,
    pub bg_glass: Hsla,
    pub border_default: Rgba,
    pub border_strong: Rgba,
    pub text_primary: Rgba,
    pub text_regular: Rgba,
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
    pub nav_idle: Rgba,
    pub nav_active_bg: Rgba,
    pub nav_item_active_bg: Rgba,
    pub nav_active_text: Rgba,
    pub nav_icon_idle_bg: Rgba,
    pub nav_icon_active_bg: Rgba,
    pub nav_icon_active_bg_soft: Rgba,
    pub method_get: Rgba,
    pub method_post: Rgba,
    pub method_put: Rgba,
    pub method_delete: Rgba,
    pub method_patch: Rgba,
    pub table_header: Rgba,
    pub row_hover: Rgba,
    pub row_selected: Rgba,
    pub status_bar_bg: Rgba,
    pub shadow: Rgba,
    /// Backdrop color for overlay/modal遮罩 (replaces launcher_glass overflow)
    pub overlay_backdrop: Hsla,
    /// Keycap / subtle chip background (replaces launcher_keycap)
    pub keycap_bg: Hsla,
}

fn build_light() -> SemanticColors {
    SemanticColors {
        bg_page: rgb(0xf5f6f8),
        bg_surface: rgb(0xffffff),
        bg_elevated: rgb(0xffffff),
        bg_subtle: rgb(0xeef1f5),
        bg_subtle_2: rgb(0xf8f9fb),
        bg_glass: hsla(0.0, 0.0, 1.0, 0.98),
        border_default: rgb(0xd7dce5),
        border_strong: rgb(0xb8c0cc),
        text_primary: rgb(0x1d1d1f),
        text_regular: rgb(0x3a3a3c),
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
        nav_idle: rgb(0x334155),
        nav_active_bg: rgb(0xd6e9ff),
        nav_item_active_bg: rgb(0xe8f3ff),
        nav_active_text: rgb(0x0066cc),
        nav_icon_idle_bg: rgb(0xf1f5f9),
        nav_icon_active_bg: rgb(0x0a84ff),
        nav_icon_active_bg_soft: rgb(0xd6e9ff),
        method_get: rgb(0x16a34a),
        method_post: rgb(0xf59e0b),
        method_put: rgb(0x3b82f6),
        method_delete: rgb(0xef4444),
        method_patch: rgb(0x0ea5e9),
        table_header: rgb(0xfafafb),
        row_hover: rgb(0xeef4fb),
        row_selected: rgb(0xddeeff),
        status_bar_bg: rgb(0xf7f8fa),
        shadow: rgb(0x000000),
        overlay_backdrop: hsla(0.0, 0.0, 0.0, 0.24),
        keycap_bg: hsla(0.0, 0.0, 0.0, 0.04),
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
        border_default: rgb(0x3a414d),
        border_strong: rgb(0x596171),
        text_primary: rgb(0xf5f5f7),
        text_regular: rgb(0xd8dce3),
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
        nav_idle: rgb(0x94a3b8),
        nav_active_bg: rgb(0x0a84ff),
        nav_item_active_bg: rgb(0x0d2a45),
        nav_active_text: rgb(0xe8f3ff),
        nav_icon_idle_bg: rgb(0x242a34),
        nav_icon_active_bg: rgb(0x0a84ff),
        nav_icon_active_bg_soft: rgb(0x143b5f),
        method_get: rgb(0x22c55e),
        method_post: rgb(0xfbbf24),
        method_put: rgb(0x60a5fa),
        method_delete: rgb(0xf87171),
        method_patch: rgb(0x22d3ee),
        table_header: rgb(0x0f1623),
        row_hover: rgb(0x222a35),
        row_selected: rgb(0x143b5f),
        status_bar_bg: rgb(0x171b22),
        shadow: rgb(0x000000),
        overlay_backdrop: hsla(0.0, 0.0, 0.0, 0.44),
        keycap_bg: hsla(0.0, 0.0, 1.0, 0.03),
    }
}

/// Get the semantic color set for the current theme mode.
#[inline(always)]
pub fn semantic() -> SemanticColors {
    if crate::app::theme_mode::is_dark() {
        build_dark()
    } else {
        build_light()
    }
}

/// Legacy string-based lookup.
pub fn token(name: &str) -> Rgba {
    let _dark = crate::app::theme_mode::is_dark();
    let s = semantic();
    match name {
        "color-bg-page" => s.bg_page,
        "color-bg-surface" => s.bg_surface,
        "color-bg-elevated" => s.bg_elevated,
        "color-bg-subtle" => s.bg_subtle,
        "color-bg-subtle-2" => s.bg_subtle_2,
        "color-border-default" => s.border_default,
        "color-border-strong" => s.border_strong,
        "color-text-primary" => s.text_primary,
        "color-text-regular" => s.text_regular,
        "color-text-secondary" => s.text_secondary,
        "color-text-placeholder" => s.text_placeholder,
        "color-primary" => s.primary,
        "color-primary-hover" => s.primary_hover,
        "color-primary-active" => s.primary_active,
        "color-primary-bg" => s.primary_bg,
        "color-primary-soft" => s.primary_soft,
        "color-success" => s.success,
        "color-warning" => s.warning,
        "color-danger" => s.danger,
        "color-info" => s.info,
        "color-nav-idle" => s.nav_idle,
        "color-nav-active-bg" => s.nav_active_bg,
        "color-nav-item-active-bg" => s.nav_item_active_bg,
        "color-nav-active-text" => s.nav_active_text,
        "color-nav-icon-idle-bg" => s.nav_icon_idle_bg,
        "color-nav-icon-active-bg" => s.nav_icon_active_bg,
        "color-nav-icon-active-bg-soft" => s.nav_icon_active_bg_soft,
        "color-method-get" => s.method_get,
        "color-method-post" => s.method_post,
        "color-method-put" => s.method_put,
        "color-method-delete" => s.method_delete,
        "color-method-patch" => s.method_patch,
        "color-table-header" => s.table_header,
        "color-row-hover" => s.row_hover,
        "color-row-selected" => s.row_selected,
        "color-status-bar-bg" => s.status_bar_bg,
        "color-shadow" => s.shadow,
        _ => rgb(0x000000),
    }
}

// ── Launcher-specific colors (from suishou LauncherWindow.qml) ──────────

pub fn launcher_panel() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x0f0f23) } else { rgb(0xf8f5ff) }
}

pub fn launcher_panel_border() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x34304f) } else { rgb(0xded4ff) }
}

pub fn launcher_field() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x15152c) } else { rgb(0xffffff) }
}

pub fn launcher_field_border() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x2a2842) } else { rgb(0xe4dcff) }
}

pub fn launcher_row_selected() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x241b48) } else { rgb(0xf7f7fa) }
}

/// Icon surface background (unselected state).  Returns a colour with
/// alpha so it composes correctly on top of the launcher glass.
pub fn launcher_icon_surface() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        rgba_with_alpha(rgb(0xf8f8fb), 0.78)
    }
}

/// Icon border (unselected state).
pub fn launcher_icon_border() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.04)
    } else {
        rgba_with_alpha(rgb(0xe7e7ee), 0.72)
    }
}

pub fn launcher_accent() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0xc8b8ff) } else { rgb(0x6b4fcf) }
}

pub fn launcher_deep_background() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x0b0b1a) } else { rgb(0xf5f5f7) }
}

pub fn launcher_glass() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        rgba_with_alpha(rgb(0x0f0f23), 0.3)
    } else {
        rgba_with_alpha(rgb(0xffffff), 0.98)
    }
}

pub fn launcher_glass_border() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.08)
    } else {
        rgba_with_alpha(rgb(0xffffff), 0.92)
    }
}

pub fn launcher_soft_line() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.04)
    } else {
        rgba_with_alpha(rgb(0xe6e6eb), 0.9)
    }
}

pub fn launcher_keycap() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.03)
    } else {
        rgba_with_alpha(rgb(0xf8f8fb), 0.78)
    }
}

pub fn launcher_muted_text() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x7777aa) } else { rgb(0x8888aa) }
}

pub fn launcher_faint_text() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0x55557a) } else { rgb(0x9999bb) }
}

/// Title text in result/list rows.
pub fn launcher_title_text() -> Rgba {
    let dark = crate::app::theme_mode::is_dark();
    if dark { rgb(0xddd8ec) } else { rgb(0x333348) }
}

/// Icon border in selected state.
pub fn launcher_icon_border_selected() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        rgba_with_alpha(rgb(0xe2e2ea), 0.2)
    } else {
        rgba_with_alpha(rgb(0xe2e2ea), 0.9)
    }
}

/// Icon surface in selected state.
pub fn launcher_icon_surface_selected() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        rgba_with_alpha(rgb(0xf2f2f7), 0.15)
    } else {
        rgba_with_alpha(rgb(0xf2f2f7), 0.9)
    }
}

/// Row hover background.
pub fn launcher_row_hover() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
    if dark {
        hsla(0.0, 0.0, 1.0, 0.025)
    } else {
        rgba_with_alpha(rgb(0xf7f7fa), 0.72)
    }
}

/// Badge / tag background.
pub fn launcher_badge_bg() -> Hsla {
    let dark = crate::app::theme_mode::is_dark();
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
    let dark = crate::app::theme_mode::is_dark();
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
