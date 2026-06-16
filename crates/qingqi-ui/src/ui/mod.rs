pub mod components;
pub mod glass;
pub mod traffic_light;
mod window_chrome;

pub use window_chrome::{
    WindowChromeConfig, WindowChromeMetrics, WindowChromeMode, WindowChromeStyle,
    WindowChromeTitlebarSlotAlignment, popup_window_chrome, popup_window_chrome_with_titlebar_slot,
};

use gpui::{
    InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Window, div, hsla, img, px, rgb, svg,
};

use crate::{assets, theme};
use qingqi_plugin::plugin_spec::{PluginAccent, PluginCategory, PluginStatus};

// ── Background Colors (compile-time safe via theme::semantic) ────────────

pub fn bg_canvas() -> gpui::Rgba {
    theme::semantic().bg_page
}

pub fn bg_surface() -> gpui::Rgba {
    theme::semantic().bg_surface
}

pub fn bg_subtle() -> gpui::Rgba {
    theme::semantic().bg_subtle
}

pub fn bg_hover() -> gpui::Rgba {
    theme::semantic().bg_subtle
}

// ── Text Colors ─────────────────────────────────────────────────────────

pub fn text_primary() -> gpui::Rgba {
    theme::semantic().text_primary
}

pub fn text_secondary() -> gpui::Rgba {
    theme::semantic().text_body
}

pub fn text_tertiary() -> gpui::Rgba {
    theme::semantic().text_secondary
}

// ── Border Colors ───────────────────────────────────────────────────────

pub fn border_light() -> gpui::Hsla {
    theme::semantic().border_default.into()
}

pub fn border_strong() -> gpui::Hsla {
    theme::semantic().border_strong.into()
}

pub fn success() -> gpui::Rgba {
    theme::semantic().success
}

pub fn warning() -> gpui::Rgba {
    theme::semantic().warning
}

pub fn danger() -> gpui::Rgba {
    theme::semantic().danger
}

pub fn info() -> gpui::Rgba {
    theme::semantic().info
}

/// Backdrop color for overlay/modal遮罩 (replaces individual hsla in overlay_shell)
pub fn overlay_backdrop() -> gpui::Hsla {
    theme::semantic().overlay_backdrop
}

/// Keycap / subtle chip background (replaces launcher_keycap)
pub fn bg_keycap() -> gpui::Hsla {
    theme::keycap_bg()
}

/// Row hover background (replaced by theme::semantic().bg_hover)
pub fn row_hover() -> gpui::Rgba {
    theme::semantic().bg_hover
}

pub fn white() -> gpui::Rgba {
    theme::white()
}

pub fn panel_heading_text() -> gpui::Rgba {
    let dark = crate::theme_mode::is_dark();
    if dark {
        theme::semantic().text_primary
    } else {
        rgb(0x444458)
    }
}

pub fn accent_color(accent: PluginAccent) -> gpui::Rgba {
    theme::accent_color(accent)
}

pub fn accent_soft(accent: PluginAccent) -> gpui::Rgba {
    theme::accent_soft(accent)
}

pub fn category_tint(category: PluginCategory) -> gpui::Rgba {
    match category {
        PluginCategory::Tool => rgb(0xe0f2fe),
        PluginCategory::System => rgb(0xf3e8ff),
        PluginCategory::About => rgb(0xfef3c7),
    }
}

pub fn status_color(status: PluginStatus) -> gpui::Rgba {
    match status {
        PluginStatus::Ready => success(),
        PluginStatus::Background => accent_color(PluginAccent::Cyan),
        PluginStatus::Preview => warning(),
    }
}

// ── Typography tokens ────────────────────────────────────────────────────

/// Unified UI font stack (Latin via Inter, CJK via PingFang SC).
/// Call sites must not hardcode font family names (conventions §8.3).
pub fn font_ui() -> &'static str {
    "Inter, PingFang SC"
}

/// Monospace font for code / logs / hex / mono blocks.
pub fn font_mono() -> &'static str {
    "SF Mono, Menlo, Monaco, Courier New, monospace"
}

/// Terminal panel font (macOS default).
pub fn font_terminal() -> &'static str {
    "Menlo"
}

// ── Shared UI Components ─────────────────────────────────────────────────

pub fn section_card() -> gpui::Div {
    div()
        .rounded(theme::radius_lg())
        .bg(bg_surface())
        .border_1()
        .border_color(border_light())
}

pub fn page_title(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> impl IntoElement {
    let title = title.into();
    let subtitle = subtitle.into();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(text_primary())
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(text_secondary())
                .child(subtitle),
        )
}

pub fn separator() -> impl IntoElement {
    div().h(px(1.0)).bg(border_light())
}

pub fn status_bar(message: impl Into<SharedString>, color: gpui::Rgba) -> impl IntoElement {
    let message = message.into();
    div()
        .h(px(30.0))
        .rounded(theme::radius_md())
        .bg(bg_subtle())
        .px_3()
        .flex()
        .items_center()
        .text_size(px(12.0))
        .text_color(color)
        .child(message)
}

pub fn mono_block(text: impl Into<SharedString>) -> impl IntoElement {
    let text = text.into();
    div()
        .rounded(theme::radius_md())
        .bg(bg_subtle())
        .border_1()
        .border_color(border_light())
        .p_3()
        .font_family(font_mono())
        .text_size(theme::font_size_mono())
        .line_height(px(18.0))
        .text_color(text_primary())
        .child(text)
}

pub fn icon_element(icon: &str, tint: gpui::Rgba, size_px: f32) -> impl IntoElement {
    let resolved = resolve_icon_path(icon);
    if icon.ends_with(".png") || icon.ends_with("app-icon.svg") {
        let path = if icon.ends_with("app-icon.svg") {
            resolve_icon_path(app_icon_png_for_size(size_px))
        } else {
            resolved
        };
        if assets::embedded(&path).is_some() {
            img(path).size(px(size_px)).into_any_element()
        } else {
            img(std::path::PathBuf::from(path))
                .size(px(size_px))
                .into_any_element()
        }
    } else {
        svg()
            .path(resolved)
            .size(px(size_px))
            .text_color(tint)
            .into_any_element()
    }
}

fn app_icon_png_for_size(size_px: f32) -> &'static str {
    if size_px <= 20.0 {
        "app_icon_16.png"
    } else if size_px <= 40.0 {
        "app_icon_32.png"
    } else if size_px <= 56.0 {
        "app_icon_64.png"
    } else if size_px <= 96.0 {
        "app_icon_128.png"
    } else if size_px <= 192.0 {
        "app_icon_256.png"
    } else {
        "app_icon_512.png"
    }
}

/// Resolve an icon path to an absolute filesystem path.
/// Input can be absolute, relative to assets/, or a short name like "icons/about.svg".
fn resolve_icon_path(icon: &str) -> String {
    assets::resolve_string(icon)
}

pub fn icon_tile(icon: &str, accent: PluginAccent, size_px: f32) -> impl IntoElement {
    let accent_rgba = accent_color(accent);
    let soft = accent_soft(accent);
    div()
        .size(px(size_px))
        .rounded(px((size_px / 5.0).round()))
        .bg(soft)
        .flex()
        .items_center()
        .justify_center()
        .child(icon_element(icon, accent_rgba, size_px * 0.52))
}

pub fn toolbar_button(label: impl Into<SharedString>) -> gpui::Div {
    let label = label.into();
    div()
        .h(px(34.0))
        .px_3()
        .rounded(theme::radius_md())
        .bg(bg_surface())
        .border_1()
        .border_color(border_light())
        .hover(|style| style.bg(theme::slate_100()).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(text_primary())
        .child(label)
}

pub fn primary_button(label: impl Into<SharedString>) -> gpui::Div {
    let label = label.into();
    let accent = theme::blue_500();
    div()
        .h(px(34.0))
        .px_3()
        .rounded(theme::radius_md())
        .bg(accent)
        .hover(|style| style.bg(theme::blue_600()).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(rgb(0xffffff))
        .child(label)
}

pub fn text_input_shell(
    value: impl Into<SharedString>,
    placeholder: impl Into<SharedString>,
) -> gpui::Div {
    let value = value.into();
    let placeholder = placeholder.into();
    let has_value = !value.is_empty();
    div()
        .h(px(38.0))
        .rounded(theme::radius_md())
        .bg(bg_surface())
        .border_1()
        .border_color(border_light())
        .px_3()
        .flex()
        .items_center()
        .text_size(theme::font_size_body())
        .text_color(if has_value {
            text_primary()
        } else {
            text_tertiary()
        })
        .child(if has_value { value } else { placeholder })
}

pub fn metric_pill(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    accent: PluginAccent,
) -> impl IntoElement {
    let label = label.into();
    let value = value.into();
    let accent_rgba = accent_color(accent);
    let soft = accent_soft(accent);
    div()
        .px_3()
        .py_2()
        .rounded(theme::radius_md())
        .bg(soft)
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_size(theme::font_size_caption())
                .text_color(text_secondary())
                .child(label),
        )
        .child(
            div()
                .text_size(theme::font_size_body())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(accent_rgba)
                .child(value),
        )
}

pub fn stat_card(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    accent: PluginAccent,
) -> impl IntoElement {
    let label = label.into();
    let value = value.into();
    let color = accent_color(accent);
    div()
        .min_w(px(116.0))
        .rounded(theme::radius_lg())
        .bg(bg_surface())
        .border_1()
        .border_color(border_light())
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(theme::font_size_caption())
                .text_color(text_tertiary())
                .child(label),
        )
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(color)
                .child(value),
        )
}

pub fn category_pill(label: impl Into<SharedString>, category: PluginCategory) -> impl IntoElement {
    let label = label.into();
    div()
        .px_2()
        .h(px(24.0))
        .rounded(px(999.0))
        .bg(category_tint(category))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .text_color(text_secondary())
        .child(label)
}

pub fn row_card(selected: bool) -> gpui::Div {
    let selected_border: gpui::Hsla = rgb(0xbfdbfe).into();
    div()
        .rounded(theme::radius_md())
        .bg(if selected {
            rgb(0xeff6ff)
        } else {
            bg_surface()
        })
        .border_1()
        .border_color(if selected {
            selected_border
        } else {
            border_light()
        })
}

pub fn plugin_surface() -> gpui::Div {
    div()
        .size_full()
        .bg(theme::semantic().bg_page)
        .font_family(font_ui())
        .text_color(theme::semantic().text_primary)
}

pub fn plugin_content() -> gpui::Div {
    div().size_full().p_3()
}

pub fn plugin_scroll_content() -> gpui::Stateful<gpui::Div> {
    plugin_content()
        .id("plugin-scroll-content")
        .overflow_y_scroll()
        .scrollbar_width(px(6.0))
}

// ── Shared UI Component Library (ported from suishou QML Ui* components) ──

/// Multi-variant button: primary, secondary, ghost, danger
pub fn ui_button(
    label: impl Into<SharedString>,
    variant: &str,
    dark: bool,
    icon: Option<SharedString>,
    danger: bool,
) -> gpui::Div {
    let label = label.into();
    let is_primary = variant == "primary";
    let is_ghost = variant == "ghost";

    let (bg_idle, text_col, border_col) = if is_primary {
        if danger {
            (
                theme::semantic().danger,
                theme::white(),
                theme::semantic().border_default,
            )
        } else {
            (
                theme::semantic().primary,
                theme::white(),
                if dark { rgb(0x1a1a1a) } else { rgb(0x00000010) },
            )
        }
    } else if danger {
        (
            theme::white(),
            theme::semantic().danger,
            theme::semantic().danger,
        )
    } else {
        let idle = if dark {
            theme::semantic().bg_elevated
        } else {
            theme::white()
        };
        (
            idle,
            theme::semantic().text_primary,
            theme::semantic().border_default,
        )
    };

    let mut btn = div()
        .h(px(30.0))
        .px(px(12.0))
        .rounded(theme::radius_md())
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .text_size(theme::font_size_body())
        .text_color(text_col);

    if is_ghost {
        btn = btn.bg(hsla(0.0, 0.0, 0.0, 0.0));
    } else {
        btn = btn.bg(bg_idle).border_1().border_color(border_col);
    }

    if let Some(icon_name) = icon {
        btn = btn.child(div().text_size(px(15.0)).child(icon_name));
    }

    btn.child(label).min_w(px(76.0))
}

/// Icon-only button (matching suishou UiIconButton)
pub fn ui_icon_button(icon_text: SharedString, size_px: f32) -> gpui::Div {
    let dark = crate::theme_mode::is_dark();
    div()
        .size(px(size_px))
        .rounded(theme::radius_md())
        .hover(|style| style.bg(theme::slate_100()).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_size(px(size_px * 0.5))
                .text_color(if dark {
                    theme::slate_400()
                } else {
                    theme::slate_500()
                })
                .child(icon_text),
        )
}

/// System-style window close button (✕) for client-drawn / always-on-top
/// plugin windows that have no native OS titlebar.  Subtle by default, red
/// on hover (mimicking the Windows close button).  Clicking closes the
/// window it is rendered in.
pub fn window_close_button() -> impl IntoElement {
    div()
        .id("qingqi-window-close")
        .w(px(40.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::radius_md())
        .text_size(px(13.0))
        .text_color(text_secondary())
        .hover(|style| style.bg(danger()).text_color(white()).cursor_pointer())
        .on_click(|_event, window, app| {
            window.defer(app, |window, _app| window.remove_window());
        })
        .child("✕")
}

/// Card container (matching suishou UiCard)
pub fn ui_card() -> gpui::Div {
    div()
        .rounded(theme::radius_lg())
        .bg(bg_surface())
        .border_1()
        .border_color(border_light())
        .p_4()
}

/// Empty state display (matching suishou UiEmptyState)
pub fn ui_empty_state(message: impl Into<SharedString>) -> impl IntoElement {
    let message = message.into();
    div()
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(14.0))
                .text_color(theme::semantic().text_body)
                .child(message),
        )
}

/// Chip/tag element (matching suishou UiChip)
pub fn ui_chip(
    label: impl Into<SharedString>,
    accent: PluginAccent,
    dark: bool,
) -> impl IntoElement {
    let label = label.into();
    let bg = if dark {
        theme::accent_soft_dark(accent)
    } else {
        theme::accent_soft(accent)
    };
    let text = theme::accent_color(accent);
    div()
        .px_2()
        .h(px(24.0))
        .rounded(theme::radius_sm())
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::font_size_caption())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(text)
        .child(label)
}

/// Divider with optional label (matching suishou UiDivider)
pub fn ui_divider(label: Option<impl Into<SharedString>>) -> impl IntoElement {
    if let Some(l) = label {
        let l = l.into();
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().h(px(1.0)).bg(border_light()))
            .child(
                div()
                    .text_size(theme::font_size_caption())
                    .text_color(text_tertiary())
                    .child(l),
            )
            .child(div().flex_1().h(px(1.0)).bg(border_light()))
            .into_any_element()
    } else {
        div()
            .w_full()
            .h(px(1.0))
            .bg(border_light())
            .into_any_element()
    }
}

pub fn focus_ring(active: bool, accent: PluginAccent) -> gpui::Hsla {
    if active {
        accent_color(accent).into()
    } else {
        border_light()
    }
}

// ── Utility functions ────────────────────────────────────────────────────

pub fn hsla_from_rgba(_rgba: gpui::Rgba, alpha: f32) -> gpui::Hsla {
    hsla(0.0, 0.0, 0.0, alpha)
}

pub fn notify_window(window: &mut Window) {
    window.refresh();
}

/// Resolves asset path relative to executable location.
pub fn asset_path(relative: &str) -> String {
    assets::resolve_string(relative)
}
