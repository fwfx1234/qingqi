use std::collections::HashSet;

use crate::service::{ApiGroup, HttpMethod};
use crate::view::ApiDebuggerView;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use qingqi_ui::components::input::Input;
use qingqi_ui::components::list::ListItem;
use qingqi_ui::components::theme::Theme;
use qingqi_ui::components::tree::{TreeItem, TreeState, tree};
use qingqi_ui::{icon, theme, ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    Folder,
    Request,
    Scenario,
}

pub fn build_tree_items(
    groups: &[ApiGroup],
    global_req_index: &mut usize,
    collapsed: &HashSet<String>,
) -> Vec<TreeItem> {
    groups
        .iter()
        .map(|group| {
            let gid = group.id.clone().unwrap_or_else(|| group.name.clone());
            let start = *global_req_index;
            *global_req_index += group.requests.len();

            let mut children = Vec::new();

            children.extend(group.requests.iter().enumerate().map(|(offset, req)| {
                let req_idx = start + offset;
                let rid = if !req.node_id.is_empty() {
                    req.node_id.clone()
                } else {
                    format!("_{}", req_idx)
                };
                let mut item = TreeItem::new(
                    format!("r:{}:{}:{}", req_idx, req.method.label(), rid),
                    format!("{}  {}", req.method.label(), req.title),
                );
                if !req.scenarios.is_empty() {
                    let should_expand = !collapsed.contains(&rid);
                    item = item.expanded(should_expand).children(
                        req.scenarios.iter().enumerate().map(|(si, scn)| {
                            let scn_id = if !scn.node_id.is_empty() {
                                scn.node_id.clone()
                            } else {
                                String::new()
                            };
                            TreeItem::new(
                                format!("s:{}:{}:{}", req_idx, si, scn_id),
                                scn.name.clone(),
                            )
                        }),
                    );
                }
                item
            }));

            children.extend(build_tree_items(
                &group.folders,
                global_req_index,
                collapsed,
            ));

            let folder_expanded = !collapsed.contains(&format!("g:{}", gid));
            TreeItem::new(format!("g:{}", gid), group.name.clone())
                .expanded(folder_expanded)
                .children(children)
        })
        .collect()
}

pub fn collection_tree(
    view: Entity<ApiDebuggerView>,
    tree_state: Entity<TreeState>,
    _cx: &App,
) -> impl IntoElement {
    let ts = tree_state.clone();

    div()
        .w(px(260.0))
        .min_h(px(0.0))
        .flex_1()
        .flex()
        .flex_col()
        .child(tree(
            &tree_state,
            move |ix, entry, selected, window, cx| {
                let item = entry.item();
                let id: String = item.id.to_string();
                let label: String = item.label.to_string();
                let depth = entry.depth();
                let id_clone = id.clone();
                let label_clone = label.clone();
                let _ = cx;

                let mut list_item = ListItem::new(ix).pl(px(8.0 + depth as f32 * 16.0));

                if id_clone.starts_with("s:") {
                    let parts: Vec<String> =
                        id_clone.splitn(4, ':').map(|s| s.to_string()).collect();
                    let req_idx: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let scn_idx: usize = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let node_id = parts.get(3).cloned().unwrap_or_default();
                    let v = view.clone();
                    list_item = list_item.child(
                        div()
                            .id(("scn-item", ix))
                            .px(px(6.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .hover(|s| s.bg(ui::glass::hover_bg(cx)))
                            .child(
                                icon!(square_terminal)
                                    .size(px(12.0))
                                    .text_color(ui::text_tertiary(cx)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(if selected {
                                        Theme::global(cx).primary
                                    } else {
                                        ui::text_secondary(cx)
                                    })
                                    .truncate()
                                    .child(label_clone.clone()),
                            )
                            .on_click(move |_, window, cx| {
                                v.update(cx, |view, cx| {
                                    view.select_scenario(req_idx, scn_idx, window, cx);
                                });
                                window.refresh();
                            })
                            .on_mouse_down(
                                MouseButton::Right,
                                {
                                    let v = view.clone();
                                    let nid = node_id.clone();
                                    let lbl = label_clone.clone();
                                    move |event, _window, cx| {
                                        cx.stop_propagation();
                                        v.update(cx, |view, _cx| {
                                            view.collection_menu_node_id = nid.clone();
                                            view.collection_menu_kind = Some(MenuKind::Scenario);
                                            view.collection_menu_title = lbl.clone();
                                                view.collection_menu_position = Some((
                                                    f32::from(event.position.x),
                                                    f32::from(event.position.y),
                                                ));
                                            view.show_collection_menu = true;
                                        });
                                    }
                                },
                            ),
                    );
                } else if id_clone.starts_with("g:") {
                    let group_id = id_clone.strip_prefix("g:").unwrap_or("").to_string();
                    let is_folder = entry.is_folder();
                    let is_expanded = entry.is_expanded();
                    let toggle_id = format!("g:{}", group_id);
                    let is_renaming = {
                        let renaming_id = view.read(cx).renaming_node_id.clone();
                        !renaming_id.is_empty() && group_id == renaming_id
                    };
                    if is_renaming {
                        let rename_input = view.read(cx).rename_inline_input.clone();
                        let v_confirm = view.clone();
                        let v_enter = view.clone();
                        list_item = list_item.child(
                            div()
                                .id(("grp-item", ix))
                                .px(px(6.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .hover(|s| s.bg(ui::glass::hover_bg(cx)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    |_, _, cx| cx.stop_propagation(),
                                )
                                .child(
                                    div()
                                        .size(px(18.0))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            if is_expanded {
                                                icon!(folder_open)
                                            } else {
                                                icon!(folder_closed)
                                            }
                                            .size(px(14.0))
                                            .text_color(Theme::global(cx).primary),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .py(px(2.0))
                                        .child(
                                            Input::new(&rename_input)
                                                .appearance(false)
                                                .bordered(false)
                                                .focus_bordered(false)
                                                .h(px(22.0))
                                                .text_size(px(12.0))
                                                .on_blur(move |_, cx| {
                                                    v_confirm.update(cx, |view, cx| {
                                                        view.confirm_inline_rename(cx);
                                                    });
                                                }),
                                        ),
                                )
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key == "enter" {
                                        v_enter.update(cx, |view, cx| {
                                            view.confirm_inline_rename(cx);
                                        });
                                    }
                                })
                                .on_mouse_down(
                                    MouseButton::Right,
                                    {
                                        let v = view.clone();
                                        let gid = group_id.clone();
                                        let lbl = label_clone.clone();
                                        move |event, _window, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |view, _cx| {
                                                view.collection_menu_node_id = gid.clone();
                                                view.collection_menu_kind =
                                                    Some(MenuKind::Folder);
                                                view.collection_menu_title = lbl.clone();
                                                view.collection_menu_position = Some((
                                                    f32::from(event.position.x),
                                                    f32::from(event.position.y),
                                                ));
                                                view.show_collection_menu = true;
                                            });
                                        }
                                    },
                                ),
                        );
                    } else {
                        let v_toggle = view.clone();
                        let v_right = view.clone();
                        list_item = list_item.child(
                            div()
                                .id(("grp-item", ix))
                                .px(px(6.0))
                                .py(px(5.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .hover(|s| s.bg(ui::glass::hover_bg(cx)))
                                .child(if is_folder {
                                    (if is_expanded {
                                        icon!(chevron_down)
                                    } else {
                                        icon!(chevron_right)
                                    })
                                    .size(px(12.0))
                                    .text_color(ui::text_tertiary(cx))
                                    .into_any_element()
                                } else {
                                    icon!(folder)
                                        .size(px(12.0))
                                        .text_color(ui::text_tertiary(cx))
                                        .into_any_element()
                                })
                                .child(
                                    if is_expanded {
                                        icon!(folder_open)
                                    } else {
                                        icon!(folder_closed)
                                    }
                                    .size(px(14.0))
                                    .text_color(Theme::global(cx).primary),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(ui::text_secondary(cx))
                                        .truncate()
                                        .child(label_clone.clone()),
                                )
                                .on_click({
                                    let gid = group_id.clone();
                                    move |_, _, cx| {
                                        if is_folder {
                                            cx.stop_propagation();
                                            v_toggle.update(cx, |view, cx| {
                                                view.toggle_expansion(
                                                    format!("g:{}", gid.clone()),
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                })
                                .on_mouse_down(
                                    MouseButton::Right,
                                    {
                                        let gid = group_id.clone();
                                        let lbl = label_clone.clone();
                                        move |event, _window, cx| {
                                            cx.stop_propagation();
                                            v_right.update(cx, |view, _cx| {
                                                view.collection_menu_node_id = gid.clone();
                                                view.collection_menu_kind =
                                                    Some(MenuKind::Folder);
                                                view.collection_menu_title = lbl.clone();
                                                view.collection_menu_position = Some((
                                                    f32::from(event.position.x),
                                                    f32::from(event.position.y),
                                                ));
                                                view.show_collection_menu = true;
                                            });
                                        }
                                    },
                                ),
                        );
                    }
                } else {
                    let method_str = id_clone.split(':').nth(2).unwrap_or("GET").to_string();
                    let method = match method_str.as_str() {
                        "DELETE" => HttpMethod::Delete,
                        "PATCH" => HttpMethod::Patch,
                        "POST" => HttpMethod::Post,
                        "PUT" => HttpMethod::Put,
                        "HEAD" => HttpMethod::Head,
                        "OPTIONS" => HttpMethod::Options,
                        _ => HttpMethod::Get,
                    };
                    let method_color =
                        theme::http_method_color(method.label(), Theme::global(cx).is_dark());
                    let display_name = label_clone
                        .splitn(2, "  ")
                        .nth(1)
                        .unwrap_or(&label_clone)
                        .to_string();

                    let req_idx: usize = id_clone
                        .splitn(4, ':')
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let node_id = id_clone.splitn(4, ':').nth(3).unwrap_or("").to_string();
                    let v = view.clone();
                    let v_confirm = view.clone();
                    let v_cancel = view.clone();
                    let v_enter = view.clone();
                    let has_scenarios = entry.is_folder();

                    let is_renaming = {
                        let renaming_id = view.read(cx).renaming_node_id.clone();
                        !renaming_id.is_empty() && node_id == renaming_id
                    };

                    if is_renaming {
                        let rename_input = view.read(cx).rename_inline_input.clone();
                        let renaming_icon = if has_scenarios {
                            (if entry.is_expanded() {
                                icon!(chevron_down)
                            } else {
                                icon!(chevron_right)
                            })
                            .size(px(12.0))
                            .text_color(ui::text_tertiary(cx))
                        } else {
                            icon!(square_terminal)
                                .size(px(12.0))
                                .text_color(ui::text_tertiary(cx))
                        };
                        list_item = list_item.child(
                            div()
                                .id(("req-item", ix))
                                .px(px(6.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .border_1()
                                .border_color(Theme::global(cx).primary)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    |_, _, cx| cx.stop_propagation(),
                                )
                                .child(
                                    div()
                                        .size(px(18.0))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(renaming_icon),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .py(px(2.0))
                                        .child(
                                            Input::new(&rename_input)
                                                .appearance(false)
                                                .bordered(false)
                                                .focus_bordered(false)
                                                .h(px(22.0))
                                                .text_size(px(12.0))
                                                .on_blur(move |_, cx| {
                                                    v_confirm.update(cx, |view, cx| {
                                                        view.confirm_inline_rename(cx);
                                                    });
                                                }),
                                        ),
                                )
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key == "enter" {
                                        v_enter.update(cx, |view, cx| {
                                            view.confirm_inline_rename(cx);
                                        });
                                    } else if event.keystroke.key == "escape" {
                                        v_cancel.update(cx, |view, _| {
                                            view.cancel_inline_rename();
                                        });
                                    }
                                })
                                .on_mouse_down(
                                    MouseButton::Right,
                                    {
                                        let v = view.clone();
                                        let nid = node_id.clone();
                                        let lbl = label_clone.clone();
                                        move |event, _window, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |view, _cx| {
                                                view.collection_menu_node_id = nid.clone();
                                                view.collection_menu_kind =
                                                    Some(MenuKind::Request);
                                                view.collection_menu_title = lbl.clone();
                                                view.collection_menu_position = Some((
                                                    f32::from(event.position.x),
                                                    f32::from(event.position.y),
                                                ));
                                                view.show_collection_menu = true;
                                            });
                                        }
                                    },
                                ),
                        );
                    } else {
                        let req_icon: gpui::AnyElement = if has_scenarios {
                            (if entry.is_expanded() {
                                icon!(chevron_down)
                            } else {
                                icon!(chevron_right)
                            })
                            .size(px(12.0))
                            .text_color(ui::text_tertiary(cx))
                            .into_any_element()
                        } else {
                            icon!(square_terminal)
                                .size(px(12.0))
                                .text_color(ui::text_tertiary(cx))
                                .into_any_element()
                        };
                        if has_scenarios {
                            let nid = node_id.clone();
                            list_item = list_item.child(
                                div()
                                    .id(("req-item", ix))
                                    .px(px(6.0))
                                    .py(px(4.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .hover(|s| s.bg(ui::glass::hover_bg(cx)))
                                    .child(
                                        div()
                                            .size(px(18.0))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(req_icon),
                                    )
                                    .child({
                                        let ts2 = ts.clone();
                                        div()
                                            .id(("req-content", ix))
                                            .flex_1()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .pl(px(2.0))
                                            .child(
                                                div()
                                                    .min_w(px(42.0))
                                                    .flex_shrink_0()
                                                    .font_family("SF Mono")
                                                    .text_size(px(11.0))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(method_color)
                                                    .whitespace_nowrap()
                                                    .child(method_str.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_size(px(12.0))
                                                    .truncate()
                                                    .child(display_name),
                                            )
                                            .on_click(move |_, _, cx| {
                                                cx.stop_propagation();
                                                ts2.update(cx, |tree, cx| {
                                                    tree.set_selected_index(Some(ix), cx);
                                                });
                                                v.update(cx, |view, cx| {
                                                    view.toggle_expansion(nid.clone(), cx);
                                                });
                                            })
                                    })
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        {
                                            let v = view.clone();
                                            let nid = node_id.clone();
                                            let lbl = label_clone.clone();
                                            move |event, _window, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |view, _cx| {
                                                    view.collection_menu_node_id = nid.clone();
                                                    view.collection_menu_kind =
                                                        Some(MenuKind::Request);
                                                    view.collection_menu_title = lbl.clone();
                                                    view.collection_menu_position = Some((
                                                        f32::from(event.position.x),
                                                        f32::from(event.position.y),
                                                    ));
                                                    view.show_collection_menu = true;
                                                });
                                            }
                                        },
                                    ),
                            );
                        } else {
                            list_item = list_item.child(
                                div()
                                    .id(("req-item", ix))
                                    .px(px(6.0))
                                    .py(px(4.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .hover(|s| s.bg(ui::glass::hover_bg(cx)))
                                    .child(
                                        div()
                                            .size(px(18.0))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(req_icon),
                                    )
                                    .child({
                                        let ts2 = ts.clone();
                                        div()
                                            .id(("req-content", ix))
                                            .flex_1()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .pl(px(2.0))
                                            .child(
                                                div()
                                                    .min_w(px(42.0))
                                                    .flex_shrink_0()
                                                    .font_family("SF Mono")
                                                    .text_size(px(11.0))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(method_color)
                                                    .whitespace_nowrap()
                                                    .child(method_str.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_size(px(12.0))
                                                    .truncate()
                                                    .child(display_name),
                                            )
                                            .on_click(move |_, window, cx| {
                                                cx.stop_propagation();
                                                ts2.update(cx, |tree, cx| {
                                                    tree.set_selected_index(Some(ix), cx);
                                                });
                                                v.update(cx, |view, cx| {
                                                    view.select_request(req_idx, window, cx)
                                                });
                                                window.refresh();
                                            })
                                    })
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        {
                                            let v = view.clone();
                                            let nid = node_id.clone();
                                            let lbl = label_clone.clone();
                                            move |event, _window, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |view, _cx| {
                                                    view.collection_menu_node_id = nid.clone();
                                                    view.collection_menu_kind =
                                                        Some(MenuKind::Request);
                                                    view.collection_menu_title = lbl.clone();
                                                    view.collection_menu_position = Some((
                                                        f32::from(event.position.x),
                                                        f32::from(event.position.y),
                                                    ));
                                                    view.show_collection_menu = true;
                                                });
                                            }
                                        },
                                    ),
                            );
                        }
                    }
                }

                list_item
            },
            _cx,
        ))
}
