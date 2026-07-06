use gpui::*;
use std::time::Duration;

use crate::token::tokens;

#[derive(Debug, Clone, Copy, Default)]
pub enum NotificationType { #[default] Info, Success, Warning, Error }

impl NotificationType {
    pub fn color(&self, cx: &App) -> Hsla {
        let token = tokens(cx);
        match self {
            Self::Info => token.info,
            Self::Success => token.success,
            Self::Warning => token.warning,
            Self::Error => token.danger,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Success => "✓",
            Self::Warning => "⚠",
            Self::Error => "✕",
        }
    }
}

#[derive(Clone)]
pub struct Notification {
    pub type_: NotificationType,
    pub title: Option<SharedString>,
    pub message: SharedString,
    pub auto_hide: Option<Duration>,
}

impl Notification {
    pub fn new(type_: NotificationType, message: impl Into<SharedString>) -> Self {
        Self { type_, title: None, message: message.into(), auto_hide: Some(Duration::from_secs(3)) }
    }
    pub fn success(message: impl Into<SharedString>) -> Self { Self::new(NotificationType::Success, message) }
    pub fn error(message: impl Into<SharedString>) -> Self { Self::new(NotificationType::Error, message) }
    pub fn warning(message: impl Into<SharedString>) -> Self { Self::new(NotificationType::Warning, message) }
    pub fn info(message: impl Into<SharedString>) -> Self { Self::new(NotificationType::Info, message) }
    pub fn title(mut self, t: impl Into<SharedString>) -> Self { self.title = Some(t.into()); self }
    pub fn auto_hide(mut self, dur: Duration) -> Self { self.auto_hide = Some(dur); self }
    pub fn sticky(mut self) -> Self { self.auto_hide = None; self }
}

#[derive(Clone)]
pub struct NotificationList {
    pub notifications: Vec<Notification>,
    max_visible: usize,
}

impl NotificationList {
    pub fn new() -> Self { Self { notifications: Vec::new(), max_visible: 5 } }
    pub fn push(&mut self, note: Notification) { self.notifications.push(note); }
    pub fn clear(&mut self) { self.notifications.clear(); }
    pub fn is_empty(&self) -> bool { self.notifications.is_empty() }
    pub fn len(&self) -> usize { self.notifications.len() }
}

impl RenderOnce for NotificationList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui::prelude::FluentBuilder;

        let token = tokens(cx);
        div()
            .absolute()
            .top_0()
            .right_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .children(self.notifications.iter().take(self.max_visible).map(|note| {
                let color = note.type_.color(cx);
                let icon = note.type_.icon();
                div()
                    .bg(token.surface)
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(token.border)
                    .p_3()
                    .flex()
                    .gap_2()
                    .items_start()
                    .shadow_lg()
                    .child(div().text_color(color).child(icon))
                    .child(div().flex().flex_col().gap_0p5()
                        .when_some(note.title.as_ref(), |d, t| {
                            d.child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).text_color(token.foreground).child(t.clone()))
                        })
                        .child(div().text_size(px(12.0)).text_color(token.muted_foreground).child(note.message.clone())))
            }))
    }
}
