use gpui::{App, Global};

pub mod context_menu;
mod dialog;
pub mod notification;
pub mod popover;
mod sheet;
mod tooltip;

pub use context_menu::{ContextMenuExt, MenuItemVariant, PopupMenu, PopupMenuItem};
pub use dialog::{ActiveDialog, Dialog, DialogButton};
pub use notification::{Notification, NotificationList, NotificationType};
pub use popover::Popover;
pub use sheet::{ActiveSheet, Placement, Sheet};
pub use tooltip::Tooltip;

#[allow(dead_code)]
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
