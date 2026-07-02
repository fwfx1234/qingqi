use gpui::*;

pub struct PopupMenu {
    pub items: Vec<PopupMenuItem>,
}

pub enum PopupMenuItem {
    Item {
        label: SharedString,
        icon: Option<String>,
        disabled: bool,
        variant: MenuItemVariant,
    },
    Divider,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum MenuItemVariant { #[default] Normal, Danger }

pub trait ContextMenuExt: InteractiveElement + ParentElement + Styled {
    fn context_menu(
        self,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> ContextMenu<Self> {
        ContextMenu::new(self)
    }
}

pub struct ContextMenu<E: ParentElement + Styled> {
    element: E,
}

impl<E: ParentElement + Styled> ContextMenu<E> {
    fn new(element: E) -> Self { Self { element } }
}
