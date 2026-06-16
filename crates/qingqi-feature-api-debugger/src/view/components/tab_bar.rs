use gpui::{rgb, Entity, IntoElement, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, px, prelude::FluentBuilder};
use gpui_component::popover::Popover;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{Icon, IconName, Sizable, Size, button::{Button, ButtonVariants}};
use qingqi_ui::{theme, ui};
use crate::service::ApiEnvironment;
use crate::view::ApiDebuggerView;
use crate::view::components::env_editor::open_env_editor_window;
use crate::view::types::OpenTab;
use crate::view::TAB_BAR_HEIGHT;
use super::dropdown::{DropdownItem, dropdown_list};
use super::shared::circle_badge;

pub fn open_tabs_bar(
    view: Entity<ApiDebuggerView>,
    tabs: Vec<OpenTab>,
    active_tab: OpenTab,
    titles: Vec<String>,
    environments: Vec<ApiEnvironment>,
    selected_env_index: usize,
    show_env_popover: bool,
    _dark: bool,
) -> impl IntoElement {
    let border = ui::border_light();
    let active_index = tabs.iter().position(|t| *t == active_tab).unwrap_or(0);

    let current_env = environments.get(selected_env_index).cloned()
        .unwrap_or_else(|| ApiEnvironment {
            name: String::from("默认环境"),
            badge: String::from("默"),
            color: 0x338855,
            base_url: String::from("http://127.0.0.1:8000"),
            variables: Vec::new(),
            headers: Vec::new(),
        });

    let env_suffix = {
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .pr(px(8.0))
            .child({
                let view = view.clone();
                let envs = environments.clone();
                let selected_idx = selected_env_index;
                Popover::new("api-env-popover")
                    .anchor(gpui::Corner::BottomRight)
                    .appearance(false)
                    .open(show_env_popover)
                    .on_open_change({
                        let v = view.clone();
                        move |is_open, _, cx| {
                            v.update(cx, |view, _cx| view.show_env_popover = *is_open);
                        }
                    })
                    .trigger({
                        Button::new("api-env-trigger")
                            .ghost()
                            .with_size(Size::XSmall)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .size(px(8.0))
                                            .rounded(px(999.0))
                                            .bg(rgb(current_env.color)),
                                    )
                                    .child(current_env.name.clone())
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(ui::text_tertiary())
                                            .child("\u{25BE}"),
                                    ),
                            )
                    })
                    .content(move |_state, _window, _cx| {
                        let v = view.clone();
                        let envs2 = envs.clone();
                        let idx = selected_idx;
                        dropdown_list(
                            envs2.into_iter().enumerate().map(|(i, env)| {
                                let active = i == idx;
                                DropdownItem::new(
                                    div()
                                        .min_h(px(48.0))
                                        .flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(circle_badge(&env.badge, env.color, 32.0))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .font_weight(gpui::FontWeight::MEDIUM)
                                                        .text_color(theme::semantic().text_primary)
                                                        .truncate()
                                                        .child(env.name.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .font_family("SF Mono")
                                                        .text_size(px(10.0))
                                                        .text_color(ui::text_tertiary())
                                                        .truncate()
                                                        .child(env.base_url.clone()),
                                                ),
                                        )
                                        .when(active, |row| {
                                            row.child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(theme::semantic().primary)
                                                    .child("\u{2713}"),
                                            )
                                        }),
                                )
                                .active(active)
                                .on_select({
                                    let v2 = v.clone();
                                    move |_, cx| {
                                        v2.update(cx, |view, cx| {
                                            view.select_environment(i, cx);
                                            view.show_env_popover = false;
                                        });
                                    }
                                })
                            }).collect(),
                        )
                    })
            })
            .child({
                let view = view.clone();
                div()
                    .id("api-env-edit-btn")
                    .size(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(ui::bg_hover()))
                    .child(
                        Icon::new(IconName::Settings)
                            .size(px(12.0))
                            .text_color(ui::text_tertiary()),
                    )
                    .on_click(move |_, _, cx| {
                        open_env_editor_window(view.clone(), cx);
                    })
            })
    };

    div()
        .id("api-tab-bar-outer")
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
        .child(
            TabBar::new("api-tab-bar")
                .underline()
                .selected_index(active_index)
                .suffix(env_suffix)
                .on_click({
                    let view = view.clone();
                    let tabs = tabs.clone();
                    move |&index, _window, cx| {
                        if let Some(tab) = tabs.get(index) {
                            view.update(cx, |view, cx| {
                                view.select_open_tab(tab.clone(), cx);
                            });
                        }
                    }
                })
                .children(tabs.into_iter().enumerate().map(|(index, _tab)| {
                    let title = titles
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| String::from("请求"));
                    let close_view = view.clone();

                    Tab::new()
                        .label(title)
                        .underline()
                        .suffix(
                            div()
                                .id(("api-tab-close", index))
                                .size(px(18.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(theme::rgba_with_alpha(ui::text_tertiary(), 0.12))
                                })
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    close_view.update(cx, |view, cx| {
                                        view.close_open_tab(index, cx);
                                    });
                                })
                                .child(
                                    Icon::new(IconName::Close)
                                        .size(px(10.0))
                                        .text_color(ui::text_tertiary()),
                                ),
                        )
                })),
        )
}

