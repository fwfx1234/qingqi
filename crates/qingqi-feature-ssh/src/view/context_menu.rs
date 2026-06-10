//! Profile 右键菜单

use gpui::prelude::*;
use gpui::*;
use qingqi_ui::{theme, theme_mode, ui};
use qingqi_ui::ui::glass;

pub fn render_profile_context_menu(
    handle: Entity<super::SshView>,
    profile_id: i64,
    position: Point<Pixels>,
) -> impl IntoElement {
    let dark = theme_mode::is_dark();
    let backdrop = handle.clone();
    let menu_x = position.x.max(px(8.0));
    let menu_y = position.y.max(px(8.0));

    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .id("profile-menu-backdrop")
                .size_full()
                .absolute()
                .bg(hsla(0.0, 0.0, 0.0, 0.001))
                .on_click({
                    let h = backdrop.clone();
                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        h.update(cx, |v, cx| v.close_context_menu(cx));
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top(menu_y)
                .left(menu_x)
                .w(px(168.0))
                .rounded(theme::radius_md())
                .border_1()
                .border_color(glass::border(dark))
                .bg(theme::semantic().bg_elevated)
                .shadow_lg()
                .overflow_hidden()
                .flex()
                .flex_col()
                .py(px(4.0))
                .child(menu_item(
                    handle.clone(),
                    profile_id,
                    "connect",
                    "连接",
                    false,
                    dark,
                ))
                .child(menu_item(
                    handle.clone(),
                    profile_id,
                    "edit",
                    "编辑",
                    false,
                    dark,
                ))
                .child(menu_item(
                    handle,
                    profile_id,
                    "delete",
                    "删除",
                    true,
                    dark,
                )),
        )
}

fn menu_item(
    handle: Entity<super::SshView>,
    profile_id: i64,
    action: &'static str,
    label: &'static str,
    danger: bool,
    dark: bool,
) -> impl IntoElement {
    let h = handle.clone();
    div()
        .id((action, profile_id as u64))
        .h(px(28.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .text_size(theme::font_size_body())
        .text_color(if danger {
            ui::danger()
        } else {
            ui::text_primary()
        })
        .cursor_pointer()
        .hover(|s| s.bg(glass::hover_bg(dark)))
        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            h.update(cx, |v, cx| {
                match action {
                    "connect" => v.connect_profile(profile_id, cx),
                    "edit" => v.open_profile_editor(Some(profile_id), cx),
                    "delete" => v.delete_profile(profile_id, cx),
                    _ => {}
                }
                v.close_context_menu(cx);
            });
        })
        .child(label)
}
