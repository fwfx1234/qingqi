//! Root view — local replacement for qingqi-ui::Root.

use super::theme::ActiveTheme;
use gpui::{AnyView, App, Context, Hsla, IntoElement, Render, Window, div, prelude::*};

/// Root view that wraps the application content.
pub struct Root {
    view: AnyView,
    /// Override for the root background color. When `None`, uses the theme's
    /// default background. Set to a fully transparent color to let the OS
    /// window blur (frosted glass) show through.
    background: Option<Hsla>,
}

impl Root {
    pub fn new(view: impl Into<AnyView>, _window: &Window, _cx: &mut Context<Self>) -> Self {
        Self {
            view: view.into(),
            background: None,
        }
    }
    pub fn view(&self) -> &AnyView {
        &self.view
    }

    /// Set the root background color. Pass `None` to fall back to the theme
    /// default; pass `Some(Hsla::transparent_black())` (or `gpui::transparent()`
    /// / `Hsla { a: 0.0, .. }`) to make the background transparent so the
    /// window's native blur shows through.
    pub fn set_background(&mut self, color: impl Into<Option<Hsla>>) {
        self.background = color.into();
    }

    /// Stub: render notification layer (no-op in local implementation).
    pub fn render_notification_layer(
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<gpui::AnyElement> {
        None
    }
}

/// Extension trait for Window (dialog/sheet/notification stubs).
pub trait WindowExt: Sized {
    fn open_dialog<F>(&mut self, cx: &mut App, _build: F)
    where
        F: Fn(crate::layer::Dialog, &mut Window, &mut App) -> crate::layer::Dialog + 'static;
    fn has_active_dialog(&mut self, _cx: &mut App) -> bool {
        false
    }
    fn close_dialog(&mut self, _cx: &mut App) {}
    fn close_all_dialogs(&mut self, _cx: &mut App) {}
    fn open_sheet<F>(&mut self, cx: &mut App, _build: F)
    where
        F: Fn(crate::layer::Sheet, &mut Window, &mut App) -> crate::layer::Sheet + 'static;
    fn push_notification(&mut self, _note: impl Into<crate::layer::Notification>, _cx: &mut App) {}
    fn clear_notifications(&mut self, _cx: &mut App) {}
    fn remove_notification<T: Sized + 'static>(&mut self, _cx: &mut App) {}
}

impl WindowExt for Window {
    fn open_dialog<F>(&mut self, _cx: &mut App, _build: F)
    where
        F: Fn(crate::layer::Dialog, &mut Window, &mut App) -> crate::layer::Dialog + 'static,
    {
    }
    fn open_sheet<F>(&mut self, _cx: &mut App, _build: F)
    where
        F: Fn(crate::layer::Sheet, &mut Window, &mut App) -> crate::layer::Sheet + 'static,
    {
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = self.background.unwrap_or_else(|| cx.theme().background());
        div()
            .id("root")
            .key_context("Root")
            .relative()
            .size_full()
            .bg(bg)
            .text_color(cx.theme().foreground())
            .font_family(".SystemUIFont")
            .child(self.view.clone())
    }
}
