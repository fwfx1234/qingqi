pub mod components;
pub mod glass;
pub mod traffic_light;
mod window_chrome;

pub use window_chrome::{
    WindowChromeConfig, WindowChromeMetrics, WindowChromeMode, WindowChromeStyle,
    WindowChromeTitlebarSlotAlignment, popup_window_chrome, popup_window_chrome_with_titlebar_slot,
};

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Window, div, hsla, img, px, rgb, svg,
};
use gpui_component::theme::Theme;

use crate::{assets, theme};
use qingqi_plugin::plugin_spec::{PluginAccent, PluginCategory, PluginStatus};

// ── Background Colors ────────────────────────────────────────────────────

pub fn bg_canvas(cx: &App) -> gpui::Hsla {
    Theme::global(cx).background
}

pub fn bg_surface(cx: &App) -> gpui::Hsla {
    Theme::global(cx).list
}

pub fn bg_subtle(cx: &App) -> gpui::Hsla {
    Theme::global(cx).muted
}

pub fn bg_hover(cx: &App) -> gpui::Hsla {
    Theme::global(cx).list_hover
}

// ── Text Colors ─────────────────────────────────────────────────────────

pub fn text_primary(cx: &App) -> gpui::Hsla {
    Theme::global(cx).foreground
}

pub fn text_secondary(cx: &App) -> gpui::Hsla {
    Theme::global(cx).muted_foreground
}

pub fn text_tertiary(cx: &App) -> gpui::Hsla {
    Theme::global(cx).muted_foreground
}

// ── Border Colors ────────────────────────────────────────────────────────

pub fn border_light(cx: &App) -> gpui::Hsla {
    Theme::global(cx).border
}

pub fn border_strong(cx: &App) -> gpui::Hsla {
    Theme::global(cx).border
}

// ── Status Colors ────────────────────────────────────────────────────────

pub fn success(cx: &App) -> gpui::Hsla {
    Theme::global(cx).success
}

pub fn warning(cx: &App) -> gpui::Hsla {
    Theme::global(cx).warning
}

pub fn danger(cx: &App) -> gpui::Hsla {
    Theme::global(cx).danger
}

pub fn info(cx: &App) -> gpui::Hsla {
    Theme::global(cx).info
}

pub fn overlay_backdrop(cx: &App) -> gpui::Hsla {
    Theme::global(cx).overlay
}

pub fn row_hover(cx: &App) -> gpui::Hsla {
    Theme::global(cx).list_hover
}

pub fn white() -> gpui::Hsla {
    hsla(0., 0., 1., 1.)
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

pub fn status_color(status: PluginStatus, cx: &App) -> gpui::Hsla {
    match status {
        PluginStatus::Ready => success(cx),
        PluginStatus::Background => accent_color(PluginAccent::Cyan).into(),
        PluginStatus::Preview => warning(cx),
    }
}

// ── Typography tokens ────────────────────────────────────────────────────

pub fn font_ui() -> &'static str {
    "Inter, PingFang SC"
}

pub fn font_mono() -> &'static str {
    "SF Mono, Menlo, Monaco, Courier New, monospace"
}

pub fn font_terminal() -> &'static str {
    "Menlo"
}

// ── Shared UI Components ─────────────────────────────────────────────────

pub fn section_card(cx: &App) -> gpui::Div {
    div()
        .rounded(theme::radius_lg())
        .bg(bg_surface(cx))
        .border_1()
        .border_color(border_light(cx))
}

pub fn page_title(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &App,
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
                .text_color(text_primary(cx))
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(text_secondary(cx))
                .child(subtitle),
        )
}

pub fn separator(cx: &App) -> impl IntoElement {
    div().h(px(1.0)).bg(border_light(cx))
}

pub fn status_bar(
    message: impl Into<SharedString>,
    color: gpui::Hsla,
    cx: &App,
) -> impl IntoElement {
    let message = message.into();
    div()
        .h(px(30.0))
        .rounded(theme::radius_md())
        .bg(bg_subtle(cx))
        .px_3()
        .flex()
        .items_center()
        .text_size(px(12.0))
        .text_color(color)
        .child(message)
}

pub fn mono_block(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let text = text.into();
    div()
        .rounded(theme::radius_md())
        .bg(bg_subtle(cx))
        .border_1()
        .border_color(border_light(cx))
        .p_3()
        .font_family(font_mono())
        .text_size(theme::font_size_mono())
        .line_height(px(18.0))
        .text_color(text_primary(cx))
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

pub fn toolbar_button(label: impl Into<SharedString>, cx: &App) -> gpui::Div {
    let label = label.into();
    div()
        .h(px(34.0))
        .px_3()
        .rounded(theme::radius_md())
        .bg(bg_surface(cx))
        .border_1()
        .border_color(border_light(cx))
        .hover(|style| style.bg(Theme::global(cx).list_hover).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(text_primary(cx))
        .child(label)
}

pub fn primary_button(label: impl Into<SharedString>, cx: &App) -> gpui::Div {
    let label = label.into();
    let accent = Theme::global(cx).primary;
    div()
        .h(px(34.0))
        .px_3()
        .rounded(theme::radius_md())
        .bg(accent)
        .hover(|style| style.bg(Theme::global(cx).primary_hover).cursor_pointer())
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
    cx: &App,
) -> gpui::Div {
    let value = value.into();
    let placeholder = placeholder.into();
    let has_value = !value.is_empty();
    div()
        .h(px(38.0))
        .rounded(theme::radius_md())
        .bg(bg_surface(cx))
        .border_1()
        .border_color(border_light(cx))
        .px_3()
        .flex()
        .items_center()
        .text_size(theme::font_size_body())
        .text_color(if has_value {
            text_primary(cx)
        } else {
            text_tertiary(cx)
        })
        .child(if has_value { value } else { placeholder })
}

pub fn metric_pill(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    accent: PluginAccent,
    cx: &App,
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
                .text_color(text_secondary(cx))
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
    cx: &App,
) -> impl IntoElement {
    let label = label.into();
    let value = value.into();
    let color = accent_color(accent);
    div()
        .min_w(px(116.0))
        .rounded(theme::radius_lg())
        .bg(bg_surface(cx))
        .border_1()
        .border_color(border_light(cx))
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(theme::font_size_caption())
                .text_color(text_tertiary(cx))
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

pub fn category_pill(
    label: impl Into<SharedString>,
    category: PluginCategory,
    cx: &App,
) -> impl IntoElement {
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
        .text_color(text_secondary(cx))
        .child(label)
}

pub fn row_card(selected: bool, cx: &App) -> gpui::Div {
    let selected_border: gpui::Hsla = rgb(0xbfdbfe).into();
    div()
        .rounded(theme::radius_md())
        .bg(if selected {
            rgb(0xeff6ff).into()
        } else {
            bg_surface(cx)
        })
        .border_1()
        .border_color(if selected {
            selected_border
        } else {
            border_light(cx)
        })
}

pub fn plugin_surface(cx: &App) -> gpui::Div {
    div()
        .size_full()
        .bg(Theme::global(cx).background)
        .font_family(font_ui())
        .text_color(Theme::global(cx).foreground)
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

// ── Shared UI Component Library ──────────────────────────────────────────

pub fn ui_button(
    label: impl Into<SharedString>,
    variant: &str,
    dark: bool,
    icon: Option<SharedString>,
    danger: bool,
    cx: &App,
) -> gpui::Div {
    let label = label.into();
    let is_primary = variant == "primary";
    let is_ghost = variant == "ghost";

    let t = Theme::global(cx);

    let (bg_idle, text_col, border_col) = if is_primary {
        if danger {
            (t.danger, hsla(0., 0., 1., 1.), t.border)
        } else {
            (
                t.primary,
                hsla(0., 0., 1., 1.),
                if dark {
                    rgb(0x1a1a1a).into()
                } else {
                    rgb(0x00000010).into()
                },
            )
        }
    } else if danger {
        (hsla(0., 0., 1., 1.), t.danger, t.danger)
    } else {
        let idle = if dark {
            t.popover
        } else {
            hsla(0., 0., 1., 1.)
        };
        (idle, t.foreground, t.border)
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

pub fn ui_icon_button(icon_text: SharedString, size_px: f32, cx: &App) -> gpui::Div {
    let t = Theme::global(cx);
    div()
        .size(px(size_px))
        .rounded(theme::radius_md())
        .hover(|style| style.bg(t.list_hover).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_size(px(size_px * 0.5))
                .text_color(t.muted_foreground)
                .child(icon_text),
        )
}

pub fn window_close_button(cx: &App) -> impl IntoElement {
    div()
        .id("qingqi-window-close")
        .w(px(40.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::radius_md())
        .text_size(px(13.0))
        .text_color(text_secondary(cx))
        .hover(|style| style.bg(danger(cx)).text_color(white()).cursor_pointer())
        .on_click(|_event, window, app| {
            window.defer(app, |window, _app| window.remove_window());
        })
        .child("✕")
}

pub fn ui_card(cx: &App) -> gpui::Div {
    div()
        .rounded(theme::radius_lg())
        .bg(bg_surface(cx))
        .border_1()
        .border_color(border_light(cx))
        .p_4()
}

pub fn ui_empty_state(message: impl Into<SharedString>, cx: &App) -> impl IntoElement {
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
                .text_color(Theme::global(cx).muted_foreground)
                .child(message),
        )
}

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

pub fn ui_divider(label: Option<impl Into<SharedString>>, cx: &App) -> impl IntoElement {
    if let Some(l) = label {
        let l = l.into();
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().h(px(1.0)).bg(border_light(cx)))
            .child(
                div()
                    .text_size(theme::font_size_caption())
                    .text_color(text_tertiary(cx))
                    .child(l),
            )
            .child(div().flex_1().h(px(1.0)).bg(border_light(cx)))
            .into_any_element()
    } else {
        div()
            .w_full()
            .h(px(1.0))
            .bg(border_light(cx))
            .into_any_element()
    }
}

pub fn focus_ring(active: bool, accent: PluginAccent, cx: &App) -> gpui::Hsla {
    if active {
        accent_color(accent).into()
    } else {
        border_light(cx)
    }
}

// ── Utility functions ────────────────────────────────────────────────────

pub fn hsla_from_rgba(_rgba: gpui::Rgba, alpha: f32) -> gpui::Hsla {
    hsla(0.0, 0.0, 0.0, alpha)
}

pub fn notify_window(window: &mut Window) {
    window.refresh();
}

pub fn asset_path(relative: &str) -> String {
    assets::resolve_string(relative)
}
