use gpui::{App, Global};

mod dialog;
mod sheet;
mod notification;
mod context_menu;
mod popover;
mod tooltip;

pub use dialog::{Dialog, DialogButton, ActiveDialog};
pub use sheet::{Sheet, ActiveSheet, Placement};
pub use notification::{Notification, NotificationList, NotificationType};
pub use context_menu::{ContextMenuExt, PopupMenu, PopupMenuItem, MenuItemVariant};
pub use popover::Popover;
pub use tooltip::Tooltip;

pub struct LayerManager {
    pub(crate) sheets: Vec<ActiveSheet>,
    pub(crate) dialogs: Vec<ActiveDialog>,
    pub(crate) notifications: NotificationList,
}

impl Global for LayerManager {}

impl LayerManager {
    pub fn new() -> Self {
        Self {
            sheets: Vec::new(),
            dialogs: Vec::new(),
            notifications: NotificationList::new(),
        }
    }

    pub fn init(cx: &mut App) {
        cx.set_global(LayerManager::new());
    }
}
