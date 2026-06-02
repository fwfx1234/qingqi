use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, WindowControlArea, div, hsla, px,
};
use gpui_component::{Icon, IconName, Sizable, Size as ComponentSize};

use crate::{theme, ui};

pub const TITLE_BAR_HEIGHT: f32 = 36.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowChromeStyle {
    Windows,
    MacOs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowChromeMode {
    Floating,
    Immersive,
}

#[derive(Clone, Debug)]
pub struct WindowChromeConfig {
    pub title: Option<SharedString>,
    pub style: WindowChromeStyle,
    pub mode: WindowChromeMode,
    pub transparent: bool,
    pub show_minimize: bool,
    pub show_maximize: bool,
    pub show_close: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct WindowChromeMetrics {
    pub titlebar_height: f32,
    pub content_top_padding: f32,
    pub safe_left: f32,
    pub safe_right: f32,
}

impl WindowChromeStyle {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Windows
        }
    }
}

impl Default for WindowChromeConfig {
    fn default() -> Self {
        Self {
            title: None,
            style: WindowChromeStyle::current(),
            mode: WindowChromeMode::Floating,
            transparent: false,
            show_minimize: true,
            show_maximize: true,
            show_close: true,
        }
    }
}

impl WindowChromeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn style(mut self, style: WindowChromeStyle) -> Self {
        self.style = style;
        self
    }

    pub fn mode(mut self, mode: WindowChromeMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn immersive(mut self, immersive: bool) -> Self {
        self.mode = if immersive {
            WindowChromeMode::Immersive
        } else {
            WindowChromeMode::Floating
        };
        self
    }

    pub fn transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    pub fn controls(mut self, minimize: bool, maximize: bool, close: bool) -> Self {
        self.show_minimize = minimize;
        self.show_maximize = maximize;
        self.show_close = close;
        self
    }

    pub fn metrics(&self) -> WindowChromeMetrics {
        WindowChromeMetrics::for_style_with_mode(self.style, self.mode)
    }
}

impl WindowChromeMetrics {
    pub fn for_current_platform() -> Self {
        Self::for_current_platform_with_mode(WindowChromeMode::Floating)
    }

    pub fn for_current_platform_with_mode(mode: WindowChromeMode) -> Self {
        Self::for_style_with_mode(WindowChromeStyle::current(), mode)
    }

    pub fn for_style(style: WindowChromeStyle) -> Self {
        Self::for_style_with_mode(style, WindowChromeMode::Floating)
    }

    pub fn for_style_with_mode(style: WindowChromeStyle, mode: WindowChromeMode) -> Self {
        let content_top_padding = match mode {
            WindowChromeMode::Floating => TITLE_BAR_HEIGHT,
            WindowChromeMode::Immersive => 0.0,
        };

        let (safe_left, safe_right) = match (style, mode) {
            (_, WindowChromeMode::Floating) => (0.0, 0.0),
            (WindowChromeStyle::Windows, WindowChromeMode::Immersive) => (12.0, 148.0),
            (WindowChromeStyle::MacOs, WindowChromeMode::Immersive) => (86.0, 12.0),
        };

        Self {
            titlebar_height: TITLE_BAR_HEIGHT,
            content_top_padding,
            safe_left,
            safe_right,
        }
    }
}

pub fn popup_window_chrome(config: WindowChromeConfig) -> impl IntoElement {
    popup_window_chrome_with_titlebar_slot(config, None)
}

pub fn popup_window_chrome_with_titlebar_slot(
    config: WindowChromeConfig,
    titlebar_slot: Option<AnyElement>,
) -> impl IntoElement {
    match config.style {
        WindowChromeStyle::MacOs => macos_window_chrome(config, titlebar_slot).into_any_element(),
        WindowChromeStyle::Windows => {
            windows_window_chrome(config, titlebar_slot).into_any_element()
        }
    }
}

fn windows_window_chrome(
    config: WindowChromeConfig,
    titlebar_slot: Option<AnyElement>,
) -> impl IntoElement {
    let immersive = config.mode == WindowChromeMode::Immersive;
    let background = if immersive || config.transparent {
        hsla(0.0, 0.0, 0.0, 0.0)
    } else {
        theme::rgba_with_alpha(theme::semantic().bg_surface, 0.72)
    };

    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(TITLE_BAR_HEIGHT))
        .flex()
        .items_center()
        .bg(background)
        .border_b_1()
        .border_color(if immersive || config.transparent {
            hsla(0.0, 0.0, 0.0, 0.0)
        } else {
            ui::border_light()
        })
        .child(windows_titlebar_content(
            config.title.clone(),
            titlebar_slot,
        ))
        .children(config.show_minimize.then(|| {
            windows_control_button(
                "qingqi-window-minimize",
                IconName::WindowMinimize,
                false,
                |window, _cx| window.minimize_window(),
            )
        }))
        .children(config.show_maximize.then(|| {
            windows_control_button(
                "qingqi-window-maximize",
                IconName::WindowMaximize,
                false,
                |window, _cx| window.zoom_window(),
            )
        }))
        .children(config.show_close.then(|| {
            windows_control_button(
                "qingqi-window-close",
                IconName::WindowClose,
                true,
                |window, cx| {
                    window.defer(cx, |window, _cx| window.remove_window());
                },
            )
        }))
}

fn windows_titlebar_content(
    title: Option<SharedString>,
    titlebar_slot: Option<AnyElement>,
) -> impl IntoElement {
    if let Some(slot) = titlebar_slot {
        div()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .flex()
            .items_center()
            .pl(px(12.0))
            .pr(px(8.0))
            .child(
                div()
                    .w(px(110.0))
                    .h_full()
                    .flex_none()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .justify_center()
                    .child(slot),
            )
            .child(
                div()
                    .w(px(110.0))
                    .h_full()
                    .flex_none()
                    .window_control_area(WindowControlArea::Drag),
            )
    } else {
        title_drag_region().pl(px(12.0)).pr(px(8.0)).child(
            div()
                .max_w(px(360.0))
                .line_clamp(1)
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme::semantic().text_secondary)
                .children(title),
        )
    }
}

fn windows_control_button(
    id: &'static str,
    icon: IconName,
    is_close: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(46.0))
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme::semantic().text_secondary)
        .hover(move |style| {
            let style = if is_close {
                style.bg(ui::danger()).text_color(ui::white())
            } else {
                style
                    .bg(ui::bg_keycap())
                    .text_color(theme::semantic().text_primary)
            };
            style.cursor_pointer()
        })
        .on_mouse_down(MouseButton::Left, |_, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_event, window, cx| {
            action(window, cx);
            cx.stop_propagation();
        })
        .child(Icon::new(icon).with_size(ComponentSize::Small))
}

fn macos_window_chrome(
    config: WindowChromeConfig,
    titlebar_slot: Option<AnyElement>,
) -> impl IntoElement {
    let immersive = config.mode == WindowChromeMode::Immersive;
    let background = if immersive || config.transparent {
        hsla(0.0, 0.0, 0.0, 0.0)
    } else {
        theme::rgba_with_alpha(theme::semantic().bg_surface, 0.62)
    };

    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(TITLE_BAR_HEIGHT))
        .flex()
        .items_center()
        .bg(background)
        .border_b_1()
        .border_color(if immersive || config.transparent {
            hsla(0.0, 0.0, 0.0, 0.0)
        } else {
            ui::border_light()
        })
        .child(
            div()
                .w(px(86.0))
                .h_full()
                .flex_none()
                .flex()
                .items_center()
                .pl(px(14.0))
                .gap(px(8.0))
                .children(config.show_close.then(|| {
                    macos_traffic_light(
                        "qingqi-window-close",
                        rgb_hex(0xff5f57),
                        true,
                        |window, cx| {
                            window.defer(cx, |window, _cx| window.remove_window());
                        },
                    )
                }))
                .children(config.show_minimize.then(|| {
                    macos_traffic_light(
                        "qingqi-window-minimize",
                        rgb_hex(0xffbd2e),
                        false,
                        |window, _cx| window.minimize_window(),
                    )
                }))
                .children(config.show_maximize.then(|| {
                    macos_traffic_light(
                        "qingqi-window-zoom",
                        rgb_hex(0x28c840),
                        false,
                        |window, _cx| window.zoom_window(),
                    )
                })),
        )
        .child(macos_titlebar_content(config.title.clone(), titlebar_slot))
        .child(div().w(px(86.0)).h_full().flex_none())
}

fn macos_titlebar_content(
    title: Option<SharedString>,
    titlebar_slot: Option<AnyElement>,
) -> impl IntoElement {
    if let Some(slot) = titlebar_slot {
        div()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(86.0))
                    .h_full()
                    .flex_none()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .justify_center()
                    .child(slot),
            )
            .child(
                div()
                    .w(px(86.0))
                    .h_full()
                    .flex_none()
                    .window_control_area(WindowControlArea::Drag),
            )
    } else {
        title_drag_region().justify_center().child(
            div()
                .max_w(px(360.0))
                .line_clamp(1)
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme::semantic().text_secondary)
                .children(title),
        )
    }
}

fn macos_traffic_light(
    id: &'static str,
    color: gpui::Rgba,
    is_close: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(px(12.0))
        .rounded(px(999.0))
        .border_1()
        .border_color(if is_close {
            theme::rgba_with_alpha(gpui::rgb(0x7a1f1b), 0.28)
        } else {
            theme::rgba_with_alpha(gpui::rgb(0x000000), 0.16)
        })
        .bg(color)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(8.0))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(theme::rgba_with_alpha(gpui::rgb(0x000000), 0.50))
        .hover(|style| style.cursor_pointer())
        .on_mouse_down(MouseButton::Left, |_, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_event, window, cx| {
            action(window, cx);
            cx.stop_propagation();
        })
}

fn title_drag_region() -> gpui::Div {
    div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .flex()
        .items_center()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |event, window, cx| {
            if event.click_count >= 2 {
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else {
                    window.zoom_window();
                }
            } else if cfg!(any(target_os = "linux", target_os = "freebsd")) {
                window.start_window_move();
            }
            cx.stop_propagation();
        })
}

fn rgb_hex(value: u32) -> gpui::Rgba {
    gpui::rgb(value)
}
