use std::collections::HashSet;

use crate::service::{ApiGroup, HttpMethod};
use crate::view::ApiDebuggerView;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use qingqi_ui::components::input::{Input, InputState};
use qingqi_ui::components::list::ListItem;
use qingqi_ui::components::theme::Theme;
use qingqi_ui::components::tree::{TreeEntry, TreeItem, TreeState, tree};
use qingqi_ui::{icon, theme, ui};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    Folder,
    Request,
    Scenario,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestClickAction {
    Select,
    ToggleExpansion,
    None,
}

fn request_click_action(selected: bool, has_scenarios: bool) -> RequestClickAction {
    match (selected, has_scenarios) {
        (false, _) => RequestClickAction::Select,
        (true, true) => RequestClickAction::ToggleExpansion,
        (true, false) => RequestClickAction::None,
    }
}

fn handle_tree_click(
    view: Entity<ApiDebuggerView>,
    tree_state: Entity<TreeState>,
    ix: usize,
    entry: &TreeEntry,
    selected: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let id = entry.item().id.to_string();
    if let Some(parts) = id.strip_prefix("s:") {
        let parts: Vec<&str> = parts.splitn(3, ':').collect();
        let request_index = parts
            .first()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let scenario_index = parts
            .get(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        tree_state.update(cx, |tree, cx| tree.set_selected_index(Some(ix), cx));
        view.update(cx, |view, cx| {
            view.select_scenario(request_index, scenario_index, window, cx);
        });
        window.refresh();
        return;
    }

    if let Some(group_id) = id.strip_prefix("g:") {
        if entry.is_folder() {
            let group_id = group_id.to_string();
            view.update(cx, |view, cx| {
                view.toggle_expansion(format!("g:{group_id}"), cx)
            });
        } else {
            tree_state.update(cx, |tree, cx| tree.set_selected_index(Some(ix), cx));
        }
        return;
    }

    let parts: Vec<&str> = id.splitn(4, ':').collect();
    let request_index = parts
        .get(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let node_id = parts.get(3).copied().unwrap_or_default();
    match request_click_action(selected, entry.is_folder()) {
        RequestClickAction::Select => {
            tree_state.update(cx, |tree, cx| tree.set_selected_index(Some(ix), cx));
            view.update(cx, |view, cx| {
                view.select_request(request_index, window, cx)
            });
            window.refresh();
        }
        RequestClickAction::ToggleExpansion => {
            view.update(cx, |view, cx| {
                view.toggle_expansion(node_id.to_string(), cx)
            });
        }
        RequestClickAction::None => {}
    }
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
                    let should_expand = collapsed.contains(&rid);
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

fn collection_tree_row(
    id: impl Into<gpui::ElementId>,
    depth: usize,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .h_full()
        .pl(px(14.0 + depth as f32 * 16.0))
        .pr(px(18.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .hover(|style| {
            style.bg(theme::rgba_with_alpha(
                Theme::global(cx).foreground.into(),
                if Theme::global(cx).is_dark() {
                    0.06
                } else {
                    0.08
                },
            ))
        })
}

fn schedule_inline_rename_focus(
    view: Entity<ApiDebuggerView>,
    input: Entity<InputState>,
    window: &mut Window,
    cx: &mut App,
) {
    let scheduled_at = Instant::now();
    tracing::info!(
        target: "qingqi::api_debugger::rename",
        step = "ui_focus_scheduled",
        "API 调试器重命名：输入框聚焦任务已调度"
    );
    window.defer(cx, move |window, cx| {
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            queue_duration_ms = scheduled_at.elapsed().as_millis(),
            step = "ui_focus_started",
            "API 调试器重命名：输入框聚焦任务开始执行"
        );
        let step_started = Instant::now();
        input.update(cx, |input, input_cx| input.focus(window, input_cx));
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            step_duration_ms = step_started.elapsed().as_millis(),
            total_duration_ms = scheduled_at.elapsed().as_millis(),
            step = "ui_input_focused",
            "API 调试器重命名：输入框聚焦完成"
        );
        view.update(cx, |view, _| view.focus_inline_rename = false);
        tracing::info!(
            target: "qingqi::api_debugger::rename",
            total_duration_ms = scheduled_at.elapsed().as_millis(),
            step = "ui_focus_completed",
            "API 调试器重命名：输入框聚焦流程完成"
        );
    });
}

pub fn collection_tree(
    view: Entity<ApiDebuggerView>,
    tree_state: Entity<TreeState>,
    focus_inline_rename: bool,
    cx: &App,
) -> impl IntoElement {
    let handler_view = view.clone();
    let handler_tree_state = tree_state.clone();
    tree_state
        .read(cx)
        .set_entry_click_handler(move |ix, entry, selected, window, cx| {
            handle_tree_click(
                handler_view.clone(),
                handler_tree_state.clone(),
                ix,
                entry,
                selected,
                window,
                cx,
            );
        });

    div()
        .w(px(260.0))
        .min_h(px(80.0))
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

                let mut list_item = ListItem::new(ix).w_full().p(px(0.0));

                if id_clone.starts_with("s:") {
                    let node_id = id_clone.splitn(4, ':').nth(3).unwrap_or("").to_string();
                    let is_renaming = {
                        let renaming_id = view.read(cx).renaming_node_id.clone();
                        !renaming_id.is_empty() && node_id == renaming_id
                    };
                    if is_renaming {
                        let rename_input = view.read(cx).rename_inline_input.clone();
                        if focus_inline_rename {
                            schedule_inline_rename_focus(
                                view.clone(),
                                rename_input.clone(),
                                window,
                                cx,
                            );
                        }
                        let v_confirm = view.clone();
                        let v_mouse_down_out = view.clone();
                        let v_cancel = view.clone();
                        list_item = list_item.child(
                            collection_tree_row(("scn-item", ix), depth, cx)
                                .border_1()
                                .border_color(Theme::global(cx).primary)
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .size(px(18.0))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            icon!(square_terminal)
                                                .size(px(12.0))
                                                .text_color(ui::text_tertiary(cx)),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(("scn-rename-input", ix))
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .py(px(2.0))
                                        .on_mouse_down_out(move |_, _, cx| {
                                            v_mouse_down_out.update(cx, |view, cx| {
                                                view.confirm_inline_rename(cx);
                                            });
                                        })
                                        .child(
                                            Input::new(&rename_input)
                                                .appearance(false)
                                                .bordered(false)
                                                .focus_bordered(false)
                                                .w_full()
                                                .h(px(22.0))
                                                .text_size(px(11.0))
                                                .on_submit({
                                                    let view = view.clone();
                                                    move |_, _, cx| {
                                                        let view = view.clone();
                                                        cx.defer(move |cx| {
                                                            view.update(cx, |view, cx| {
                                                                view.confirm_inline_rename(cx);
                                                            });
                                                        });
                                                    }
                                                })
                                                .on_blur(move |_, cx| {
                                                    let view = v_confirm.clone();
                                                    cx.defer(move |cx| {
                                                        view.update(cx, |view, cx| {
                                                            view.confirm_inline_rename(cx);
                                                        });
                                                    });
                                                }),
                                        ),
                                )
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key == "escape" {
                                        v_cancel.update(cx, |view, _| view.cancel_inline_rename());
                                    }
                                }),
                        );
                    } else {
                        list_item = list_item.child(
                            collection_tree_row(("scn-item", ix), depth, cx)
                                .gap(px(4.0))
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
                                .on_mouse_down(MouseButton::Right, {
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
                                }),
                        );
                    }
                } else if id_clone.starts_with("g:") {
                    let group_id = id_clone.strip_prefix("g:").unwrap_or("").to_string();
                    let is_folder = entry.is_folder();
                    let is_expanded = entry.is_expanded();
                    let is_renaming = {
                        let renaming_id = view.read(cx).renaming_node_id.clone();
                        !renaming_id.is_empty() && group_id == renaming_id
                    };
                    if is_renaming {
                        let rename_input = view.read(cx).rename_inline_input.clone();
                        if focus_inline_rename {
                            schedule_inline_rename_focus(
                                view.clone(),
                                rename_input.clone(),
                                window,
                                cx,
                            );
                        }
                        let v_confirm = view.clone();
                        let v_mouse_down_out = view.clone();
                        list_item = list_item.child(
                            collection_tree_row(("grp-item", ix), depth, cx)
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(|_, _, cx| cx.stop_propagation())
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
                                        .id(("grp-rename-input", ix))
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .py(px(2.0))
                                        .on_mouse_down_out(move |_, _, cx| {
                                            v_mouse_down_out.update(cx, |view, cx| {
                                                view.confirm_inline_rename(cx);
                                            });
                                        })
                                        .child(
                                            Input::new(&rename_input)
                                                .appearance(false)
                                                .bordered(false)
                                                .focus_bordered(false)
                                                .w_full()
                                                .h(px(22.0))
                                                .text_size(px(12.0))
                                                .on_submit({
                                                    let view = view.clone();
                                                    move |_, _, cx| {
                                                        let view = view.clone();
                                                        cx.defer(move |cx| {
                                                            view.update(cx, |view, cx| {
                                                                view.confirm_inline_rename(cx);
                                                            });
                                                        });
                                                    }
                                                })
                                                .on_blur(move |_, cx| {
                                                    let view = v_confirm.clone();
                                                    cx.defer(move |cx| {
                                                        view.update(cx, |view, cx| {
                                                            view.confirm_inline_rename(cx);
                                                        });
                                                    });
                                                }),
                                        ),
                                )
                                .on_mouse_down(MouseButton::Right, {
                                    let v = view.clone();
                                    let gid = group_id.clone();
                                    let lbl = label_clone.clone();
                                    move |event, _window, cx| {
                                        cx.stop_propagation();
                                        v.update(cx, |view, _cx| {
                                            view.collection_menu_node_id = gid.clone();
                                            view.collection_menu_kind = Some(MenuKind::Folder);
                                            view.collection_menu_title = lbl.clone();
                                            view.collection_menu_position = Some((
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ));
                                            view.show_collection_menu = true;
                                        });
                                    }
                                }),
                        );
                    } else {
                        let v_right = view.clone();
                        list_item = list_item.child(
                            collection_tree_row(("grp-item", ix), depth, cx)
                                .gap(px(4.0))
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
                                .on_mouse_down(MouseButton::Right, {
                                    let gid = group_id.clone();
                                    let lbl = label_clone.clone();
                                    move |event, _window, cx| {
                                        cx.stop_propagation();
                                        v_right.update(cx, |view, _cx| {
                                            view.collection_menu_node_id = gid.clone();
                                            view.collection_menu_kind = Some(MenuKind::Folder);
                                            view.collection_menu_title = lbl.clone();
                                            view.collection_menu_position = Some((
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ));
                                            view.show_collection_menu = true;
                                        });
                                    }
                                }),
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

                    let node_id = id_clone.splitn(4, ':').nth(3).unwrap_or("").to_string();
                    let v_confirm = view.clone();
                    let v_cancel = view.clone();
                    let has_scenarios = entry.is_folder();

                    let is_renaming = {
                        let renaming_id = view.read(cx).renaming_node_id.clone();
                        !renaming_id.is_empty() && node_id == renaming_id
                    };

                    if is_renaming {
                        let rename_input = view.read(cx).rename_inline_input.clone();
                        if focus_inline_rename {
                            schedule_inline_rename_focus(
                                view.clone(),
                                rename_input.clone(),
                                window,
                                cx,
                            );
                        }
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
                        let v_mouse_down_out = view.clone();
                        list_item = list_item.child(
                            collection_tree_row(("req-item", ix), depth, cx)
                                .border_1()
                                .border_color(Theme::global(cx).primary)
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(|_, _, cx| cx.stop_propagation())
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
                                        .id(("req-rename-input", ix))
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .py(px(2.0))
                                        .on_mouse_down_out(move |_, _, cx| {
                                            v_mouse_down_out.update(cx, |view, cx| {
                                                view.confirm_inline_rename(cx);
                                            });
                                        })
                                        .child(
                                            Input::new(&rename_input)
                                                .appearance(false)
                                                .bordered(false)
                                                .focus_bordered(false)
                                                .w_full()
                                                .h(px(22.0))
                                                .text_size(px(12.0))
                                                .on_submit({
                                                    let view = view.clone();
                                                    move |_, _, cx| {
                                                        let view = view.clone();
                                                        cx.defer(move |cx| {
                                                            view.update(cx, |view, cx| {
                                                                view.confirm_inline_rename(cx);
                                                            });
                                                        });
                                                    }
                                                })
                                                .on_blur(move |_, cx| {
                                                    let view = v_confirm.clone();
                                                    cx.defer(move |cx| {
                                                        view.update(cx, |view, cx| {
                                                            view.confirm_inline_rename(cx);
                                                        });
                                                    });
                                                }),
                                        ),
                                )
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key == "escape" {
                                        v_cancel.update(cx, |view, _| {
                                            view.cancel_inline_rename();
                                        });
                                    }
                                })
                                .on_mouse_down(MouseButton::Right, {
                                    let v = view.clone();
                                    let nid = node_id.clone();
                                    let lbl = label_clone.clone();
                                    move |event, _window, cx| {
                                        cx.stop_propagation();
                                        v.update(cx, |view, _cx| {
                                            view.collection_menu_node_id = nid.clone();
                                            view.collection_menu_kind = Some(MenuKind::Request);
                                            view.collection_menu_title = lbl.clone();
                                            view.collection_menu_position = Some((
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ));
                                            view.show_collection_menu = true;
                                        });
                                    }
                                }),
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
                        let request_row = collection_tree_row(("req-item", ix), depth, cx);
                        let request_row = if selected {
                            request_row.bg(theme::rgba_with_alpha(
                                Theme::global(cx).primary.into(),
                                if Theme::global(cx).is_dark() {
                                    0.16
                                } else {
                                    0.1
                                },
                            ))
                        } else {
                            request_row
                        };
                        list_item = list_item.child(
                            request_row
                                .child(
                                    div()
                                        .size(px(18.0))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(req_icon),
                                )
                                .child(
                                    div()
                                        .id(("req-content", ix))
                                        .h_full()
                                        .min_w(px(0.0))
                                        .flex_1()
                                        .flex()
                                        .items_center()
                                        .pl(px(2.0))
                                        .child(
                                            div()
                                                .h_full()
                                                .flex_none()
                                                .flex()
                                                .items_center()
                                                .mr(px(8.0))
                                                .font_family("SF Mono")
                                                .text_size(px(11.0))
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(method_color)
                                                .whitespace_nowrap()
                                                .child(method_str.clone()),
                                        )
                                        .child(
                                            div()
                                                .h_full()
                                                .min_w(px(0.0))
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .text_size(px(12.0))
                                                .text_color(if selected {
                                                    Theme::global(cx).primary
                                                } else {
                                                    ui::text_secondary(cx)
                                                })
                                                .truncate()
                                                .child(display_name),
                                        ),
                                )
                                .on_mouse_down(MouseButton::Right, {
                                    let v = view.clone();
                                    let nid = node_id.clone();
                                    let lbl = label_clone.clone();
                                    move |event, _window, cx| {
                                        cx.stop_propagation();
                                        v.update(cx, |view, _cx| {
                                            view.collection_menu_node_id = nid.clone();
                                            view.collection_menu_kind = Some(MenuKind::Request);
                                            view.collection_menu_title = lbl.clone();
                                            view.collection_menu_position = Some((
                                                f32::from(event.position.x),
                                                f32::from(event.position.y),
                                            ));
                                            view.show_collection_menu = true;
                                        });
                                    }
                                }),
                        );
                    }
                }

                list_item
            },
            cx,
        ))
}

#[cfg(test)]
mod tests {
    use super::{RequestClickAction, request_click_action};

    #[test]
    fn request_click_selects_before_toggling_scenarios() {
        assert_eq!(
            request_click_action(false, true),
            RequestClickAction::Select
        );
        assert_eq!(
            request_click_action(true, true),
            RequestClickAction::ToggleExpansion
        );
        assert_eq!(
            request_click_action(false, false),
            RequestClickAction::Select
        );
        assert_eq!(request_click_action(true, false), RequestClickAction::None);
    }
}
