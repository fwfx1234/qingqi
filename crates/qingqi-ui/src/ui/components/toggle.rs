use gpui::{div, px, rgb, ParentElement, Styled};

use crate::theme;

pub fn toggle(enabled: bool) -> gpui::Div {
    let track_bg = if enabled {
        theme::blue_500()
    } else {
        gpui::rgba(0x80808040)
    };
    let thumb_x = if enabled { px(20.0) } else { px(2.0) };

    div()
        .w(px(40.0))
        .h(px(22.0))
        .rounded(px(11.0))
        .bg(track_bg)
        .cursor_pointer()
        .flex()
        .items_center()
        .flex_shrink_0()
        .child(
            div()
                .w(px(18.0))
                .h(px(18.0))
                .rounded(px(9.0))
                .bg(rgb(0xFFFFFF))
                .ml(thumb_x)
                .shadow_md(),
        )
}
