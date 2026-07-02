use gpui::*;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
pub enum NotificationType { #[default] Info, Success, Warning, Error }

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
    pub fn auto_hide(mut self, dur: Duration) -> Self { self.auto_hide = Some(dur); self }
    pub fn sticky(mut self) -> Self { self.auto_hide = None; self }
}

#[derive(Clone)]
pub struct NotificationList {
    pub notifications: Vec<Notification>,
}

impl NotificationList {
    pub fn new() -> Self { Self { notifications: Vec::new() } }
    pub fn push(&mut self, note: Notification) { self.notifications.push(note); }
    pub fn clear(&mut self) { self.notifications.clear(); }
}
