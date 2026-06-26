use gpui::ElementId;
use gpui_component::{Sizable, Size, button::Button, switch::Switch};

use super::*;

pub(super) fn pill_button(
    label: &'static str,
    _cx: &App,
    handler: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    Button::new(ElementId::Name(format!("clipboard-pill-{label}").into()))
        .label(label)
        .compact()
        .with_size(Size::Small)
        .on_click(move |event, _window, cx| handler(event, cx))
}

pub(super) fn toggle_control(
    id: impl Into<gpui::ElementId>,
    enabled: bool,
    handler: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    Switch::new(id)
        .checked(enabled)
        .on_click(move |_checked, _window, cx| handler(cx))
}
