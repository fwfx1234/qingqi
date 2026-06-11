//! Session Tab 栏

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{Icon, IconName};
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::{theme, ui};

use crate::model::SessionId;

use super::SessionTabItem;

const ACCENT: PluginAccent = PluginAccent::Cyan;
const TAB_BAR_HEIGHT: f32 = 34.0;
pub fn render_session_tabs(
    sessions: &[SessionTabItem],
    scroll: &ScrollHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let border = ui::border_light();

    div()
        .id("ssh-session-tab-bar")
        .w_full()
        .h(px(TAB_BAR_HEIGHT))
        .min_h(px(TAB_BAR_HEIGHT))
        .flex_shrink_0()
        .relative()
        .bg(ui::bg_surface())
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(1.0))
                .bg(border),
        )
        .child(if sessions.is_empty() {
            empty_tab_bar().into_any_element()
        } else {
            tab_strip(sessions, scroll, cx).into_any_element()
        })
}

fn empty_tab_bar() -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .pl(px(14.0))
        .text_size(px(12.0))
        .text_color(ui::text_tertiary())
        .child("双击左侧连接以打开会话")
}

fn tab_strip(
    sessions: &[SessionTabItem],
    scroll: &ScrollHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let accent: Hsla = ui::accent_color(ACCENT).into();
    let handle = cx.entity().clone();

    div()
        .id("ssh-session-tabs")
        .w_full()
        .h_full()
        .overflow_x_scroll()
        .track_scroll(scroll)
        .flex()
        .items_end()
        .children(sessions.iter().map(|s| {
            let sid = s.session_id;
            let sid_close = s.session_id;
            let is_selected = s.is_selected;
            let title = s.title.clone();

            div()
                .id(("ssh-tab", sid.0.as_u128() as u64))
                .group("ssh-session-tab")
                .flex_shrink_0()
                .h(px(TAB_BAR_HEIGHT))
                .max_w(px(168.0))
                .min_w(px(68.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .gap(px(5.0))
                .cursor_pointer()
                .border_b(if is_selected { px(2.0) } else { px(0.0) })
                .border_color(if is_selected {
                    accent
                } else {
                    hsla(0.0, 0.0, 0.0, 0.0)
                })
                .when(!is_selected, |tab| tab.hover(|s| s.bg(ui::bg_hover())))
                .on_click(
                    cx.listener(move |view, _: &ClickEvent, _w, cx| view.select_session(sid, cx)),
                )
                .child(status_dot(s.status_color))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(12.0))
                        .font_weight(if is_selected {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if is_selected {
                            accent
                        } else {
                            ui::text_secondary().into()
                        })
                        .child(title),
                )
                .child(close_button(sid_close, is_selected, handle.clone()))
        }))
}

fn status_dot(color: Hsla) -> impl IntoElement {
    div().flex_shrink_0().size(px(6.0)).rounded_full().bg(color)
}

fn close_button(
    session_id: SessionId,
    is_selected: bool,
    handle: Entity<super::SshView>,
) -> impl IntoElement {
    div()
        .id(("ssh-tab-close", session_id.0.as_u128() as u64))
        .flex_shrink_0()
        .size(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .cursor_pointer()
        .opacity(if is_selected { 0.55 } else { 0.28 })
        .group_hover("ssh-session-tab", |s| {
            s.opacity(1.0)
                .bg(theme::rgba_with_alpha(ui::text_tertiary(), 0.12))
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            handle.update(cx, |view, cx| view.close_session(session_id, cx));
        })
        .child(
            Icon::new(IconName::Close)
                .size(px(10.0))
                .text_color(ui::text_tertiary()),
        )
}
