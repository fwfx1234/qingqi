use gpui::*;

use crate::token::tokens;

pub struct PopupMenu {
    pub items: Vec<PopupMenuItem>,
}

pub enum PopupMenuItem {
    Item {
        label: SharedString,
        icon: Option<String>,
        disabled: bool,
        variant: MenuItemVariant,
        on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    },
    Divider,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum MenuItemVariant { #[default] Normal, Danger }

impl PopupMenu {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn item(mut self, label: impl Into<SharedString>) -> Self {
        self.items.push(PopupMenuItem::Item {
            label: label.into(),
            icon: None,
            disabled: false,
            variant: MenuItemVariant::Normal,
            on_click: None,
        });
        self
    }

    pub fn divider(mut self) -> Self {
        self.items.push(PopupMenuItem::Divider);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl RenderOnce for PopupMenu {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = tokens(cx);
        div()
            .bg(token.surface)
            .rounded(px(8.0))
            .border_1()
            .border_color(token.border)
            .shadow_lg()
            .py_1()
            .min_w(px(160.0))
            .flex()
            .flex_col()
            .children(self.items.into_iter().map(|item| {
                match item {
                    PopupMenuItem::Item { label, icon, disabled, variant, on_click } => {
                        let token = tokens(cx);
                        let text_color = match variant {
                            MenuItemVariant::Normal => token.foreground,
                            MenuItemVariant::Danger => token.danger,
                        };
                        let mut item_div = div()
                            .px_3()
                            .py_1p5()
                            .text_size(px(13.0))
                            .text_color(text_color)
                            .flex()
                            .gap_2()
                            .items_center();

                        if let Some(i) = icon {
                            item_div = item_div.child(i);
                        }

                        item_div = item_div.child(label);

                        if !disabled && on_click.is_some() {
                            let handler = on_click.unwrap();
                            item_div = item_div
                                .hover(|s| s.bg(token.surface_hover).cursor_pointer());
                            item_div.interactivity().on_click(move |e, w, cx| handler(e, w, cx));
                        } else if disabled {
                            item_div = item_div.opacity(0.4);
                        }

                        item_div.into_any_element()
                    }
                    PopupMenuItem::Divider => {
                        div()
                            .my_1()
                            .mx_2()
                            .h(px(1.0))
                            .bg(token.border)
                            .into_any_element()
                    }
                }
            }))
    }
}

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
    fn new(element: E) -> Self {
        Self { element }
    }
}

impl<E: ParentElement + Styled> Styled for ContextMenu<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl<E: ParentElement + Styled> ParentElement for ContextMenu<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.element.extend(elements);
    }
}
