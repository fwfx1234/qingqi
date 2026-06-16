use gpui::{AnyElement, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, Window, div, px, prelude::FluentBuilder};
use qingqi_ui::theme;

/// A styled dropdown menu item.
///
/// ```ignore
/// DropdownItem::new(div().child("GET"))
///     .active(current_method == HttpMethod::Get)
///     .on_select(move |_, cx| { view.update(...); })
/// ```
pub struct DropdownItem {
    active: bool,
    element: AnyElement,
    on_select: Option<Box<dyn Fn(&mut Window, &mut gpui::App) + 'static>>,
}

impl DropdownItem {
    pub fn new(element: impl IntoElement) -> Self {
        Self {
            active: false,
            element: element.into_any_element(),
            on_select: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn on_select(
        mut self,
        f: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
}

/// Render a styled dropdown container with the given items.
///
/// Items are rendered with rounded highlights on hover and a subtle
/// accent background for the active item. The list has a border, shadow,
/// and consistent padding.
pub fn dropdown_list(
    items: Vec<DropdownItem>,
    accent: Hsla,
    bg: Hsla,
    border: Hsla,
) -> impl IntoElement {
    div()
        .py(px(4.0))
        .border_1()
        .border_color(border)
        .bg(bg)
        .rounded(px(8.0))
        .shadow_md()
        .overflow_hidden()
        .flex()
        .flex_col()
        .children(items.into_iter().map(move |item| {
            let active = item.active;
            let element = item.element;
            let on_select = item.on_select;

            div()
                .px(px(10.0))
                .py(px(6.0))
                .mx(px(4.0))
                .my(px(1.0))
                .rounded(px(5.0))
                .bg(if active {
                    theme::rgba_with_alpha(accent.into(), 0.08)
                } else {
                    Hsla::default()
                })
                .hover(move |s| {
                    s.bg(theme::rgba_with_alpha(accent.into(), 0.05))
                        .cursor_pointer()
                })
                .child(element)
                .when_some(on_select, |this, f| {
                    this.on_mouse_down(MouseButton::Left, move |_, window, cx| f(window, cx))
                })
        }))
}
