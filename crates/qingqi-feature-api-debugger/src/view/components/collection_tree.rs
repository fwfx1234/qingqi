use std::collections::HashSet;

use gpui::{
    App, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::list::ListItem;
use gpui_component::theme::Theme;
use gpui_component::tree::{tree, TreeItem, TreeState};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::{Icon, IconName};
use qingqi_ui::{theme, ui};
use crate::service::{ApiGroup, HttpMethod};
use crate::view::ApiDebuggerView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    Folder,
    Request,
    Scenario,
}

pub fn build_tree_items(groups: &[ApiGroup], global_req_index: &mut usize, collapsed: &HashSet<String>) -> Vec<TreeItem> {
    groups.iter().map(|group| {
        let gid = group.id.clone().unwrap_or_else(|| group.name.clone());
        let start = *global_req_index;
        *global_req_index += group.requests.len();

        let mut children = Vec::new();

        children.extend(
            group.requests.iter().enumerate().map(|(offset, req)| {
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
            }),
        );

        children.extend(build_tree_items(&group.folders, global_req_index, collapsed));

        TreeItem::new(format!("g:{}", gid), group.name.clone())
            .expanded(true)
            .children(children)
    }).collect()
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
        .child(
            tree(&tree_state, move |ix, entry, selected, _window, cx| {
                let item = entry.item();
                let id: String = item.id.to_string();
                let label: String = item.label.to_string();
                let depth = entry.depth();
                let id_clone = id.clone();
                let label_clone = label.clone();

                let mut list_item = ListItem::new(ix)
                    .pl(px(8.0 + depth as f32 * 16.0));

                if id_clone.starts_with("s:") {
                    let parts: Vec<String> = id_clone.splitn(4, ':').map(|s| s.to_string()).collect();
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
                            .child(
                                Icon::new(IconName::SquareTerminal)
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
                                    view.select_scenario(req_idx, scn_idx, cx);
                                });
                                window.refresh();
                            })
                            .context_menu({
                                let v = view.clone();
                                let nid = node_id.clone();
                                move |menu, _window, _| {
                                    let v1 = v.clone(); let n1 = nid.clone();
                                    let v2 = v.clone(); let n2 = nid.clone();
                                    menu
                                        .item(PopupMenuItem::new("重命名")
                                            .on_click(move |_, _, cx| {
                                                v1.update(cx, |view, cx| {
                                                    view.collection_menu_node_id = n1.clone();
                                                    view.open_rename(cx);
                                                });
                                            }))
                                        .item(PopupMenuItem::new("删除")
                                            .on_click(move |_, _, cx| {
                                                v2.update(cx, |view, _cx| {
                                                    view.collection_menu_node_id = n2.clone();
                                                    view.delete_selected_collection_item();
                                                });
                                            }))
                                }
                            }),
                    );
                } else if id_clone.starts_with("g:") {
                    let group_id = id_clone.strip_prefix("g:").unwrap_or("").to_string();
                    let is_folder = entry.is_folder();
                    let is_expanded = entry.is_expanded();
                    list_item = list_item.child(
                        div()
                            .id(("grp-item", ix))
                            .px(px(6.0))
                            .py(px(5.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                if is_folder {
                                    Icon::new(
                                        if is_expanded { IconName::ChevronDown } else { IconName::ChevronRight },
                                    )
                                    .size(px(12.0))
                                    .text_color(ui::text_tertiary(cx))
                                } else {
                                    Icon::new(IconName::Folder)
                                        .size(px(12.0))
                                        .text_color(ui::text_tertiary(cx))
                                },
                            )
                            .child(
                                Icon::new(
                                    if is_expanded { IconName::FolderOpen } else { IconName::FolderClosed },
                                )
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
                            .context_menu({
                                let v = view.clone();
                                let gid = group_id.clone();
                                move |menu, _window, _| {
                                    let v1 = v.clone(); let v2 = v.clone(); let v3 = v.clone();
                                    let v4 = v.clone(); let v5 = v.clone(); let v6 = v.clone();
                                    let v7 = v.clone(); let v8 = v.clone();
                                    let n1 = gid.clone(); let n2 = gid.clone(); let n3 = gid.clone();
                                    let n4 = gid.clone();
                                    menu
                                        .item(PopupMenuItem::new("新建文件夹")
                                            .on_click(move |_, _, cx| {
                                                v1.update(cx, |view, _cx| {
                                                    view.collection_menu_node_id = n1.clone();
                                                    view.create_new_folder();
                                                });
                                            }))
                                        .item(PopupMenuItem::new("新建接口")
                                            .on_click(move |_, _, cx| {
                                                v2.update(cx, |view, _cx| {
                                                    view.collection_menu_node_id = n2.clone();
                                                    view.create_new_endpoint();
                                                });
                                            }))
                                        .item(PopupMenuItem::new("导入 cURL")
                                            .on_click(move |_, _, cx| {
                                                v3.update(cx, |view, _cx| {
                                                    view.show_curl_import = true;
                                                });
                                            }))
                                        .item(PopupMenuItem::new("导入 OpenAPI")
                                            .on_click(move |_, _, cx| {
                                                v4.update(cx, |view, _cx| view.import_openapi_file());
                                            }))
                                        .item(PopupMenuItem::new("导入 Postman")
                                            .on_click(move |_, _, cx| {
                                                v5.update(cx, |view, _cx| view.import_postman_file());
                                            }))
                                        .item(PopupMenuItem::new("导出为 OpenAPI")
                                            .on_click(move |_, _, cx| {
                                                v6.update(cx, |view, _cx| view.export_openapi());
                                            }))
                                        .item(PopupMenuItem::new("重命名")
                                            .on_click(move |_, _, cx| {
                                                v7.update(cx, |view, cx| {
                                                    view.collection_menu_node_id = n3.clone();
                                                    view.open_rename(cx);
                                                });
                                            }))
                                        .item(PopupMenuItem::new("删除")
                                            .on_click(move |_, _, cx| {
                                                v8.update(cx, |view, _cx| {
                                                    view.collection_menu_node_id = n4.clone();
                                                    view.delete_selected_collection_item();
                                                });
                                            }))
                                }
                            }),
                    );
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
                    let method_color = theme::http_method_color(method.label(), Theme::global(cx).is_dark());
                    let display_name = label_clone.splitn(2, "  ").nth(1).unwrap_or(&label_clone).to_string();

                    let req_idx: usize = id_clone.splitn(4, ':').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let node_id = id_clone.splitn(4, ':').nth(3).unwrap_or("").to_string();
                    let v = view.clone();
                    let has_scenarios = entry.is_folder();

                    if has_scenarios {
                        let is_expanded = entry.is_expanded();
                        let currently_collapsed = view.read(cx).collapsed_nodes.borrow().contains(&node_id);
                        if currently_collapsed == is_expanded {
                            let nid = node_id.clone();
                            view.update(cx, |view, _cx| {
                                let mut c = view.collapsed_nodes.borrow_mut();
                                if is_expanded {
                                    c.remove(&nid);
                                } else {
                                    c.insert(nid);
                                }
                            });
                        }
                    }

                    let is_renaming = {
                        let renaming_id = view.read(cx).renaming_node_id.clone();
                        !renaming_id.is_empty() && node_id == renaming_id
                    };

                    if is_renaming {
                        let rename_input = view.read(cx).rename_inline_input.clone();
                        let v_confirm = view.clone();
                        let v_cancel = view.clone();
                        let renaming_icon = if has_scenarios {
                            Icon::new(
                                if entry.is_expanded() { IconName::ChevronDown } else { IconName::ChevronRight },
                            )
                            .size(px(12.0))
                            .text_color(ui::text_tertiary(cx))
                            .into_any_element()
                        } else {
                            Icon::new(IconName::SquareTerminal)
                                .size(px(12.0))
                                .text_color(ui::text_tertiary(cx))
                                .into_any_element()
                        };
                        list_item = list_item.child(
                            div()
                                .id(("req-item", ix))
                                .px(px(6.0))
                                .py(px(4.0))
                                .h(px(30.0))
                                .rounded(px(4.0))
                                .bg(Theme::global(cx).popover)
                                .flex()
                                .items_center()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                                        .min_w(px(42.0))
                                        .flex_shrink_0()
                                        .font_family("SF Mono")
                                        .text_size(px(11.0))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(method_color)
                                        .whitespace_nowrap()
                                        .child(method_str),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(rename_input.clone()),
                                )
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key == "enter" {
                                        v_confirm.update(cx, |view, cx| {
                                            view.confirm_inline_rename(cx);
                                        });
                                    } else if event.keystroke.key == "escape" {
                                        v_cancel.update(cx, |view, _| {
                                            view.cancel_inline_rename();
                                        });
                                    }
                                })
                                .context_menu({
                                    let v = view.clone();
                                    let nid = node_id.clone();
                                    move |menu, _window, _| {
                                        let v1 = v.clone(); let v2 = v.clone();
                                        let v3 = v.clone(); let v4 = v.clone();
                                        let n1 = nid.clone(); let n2 = nid.clone();
                                        let n3 = nid.clone(); let n4 = nid.clone();
                                        menu
                                            .item(PopupMenuItem::new("新建用例")
                                                .on_click(move |_, _, cx| {
                                                    v1.update(cx, |view, _cx| {
                                                        view.collection_menu_node_id = n1.clone();
                                                        view.create_new_case();
                                                    });
                                                }))
                                            .item(PopupMenuItem::new("复制路径")
                                                .on_click(move |_, _, cx| {
                                                    let nd = n2.clone();
                                                    v2.update(cx, |view, cx| {
                                                        if !nd.is_empty() {
                                                            if let Ok(Some(node)) = view.service.get_collection_node(&nd) {
                                                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(node.url.clone()));
                                                                view.notice = format!("已复制: {}", node.url);
                                                            }
                                                        }
                                                    });
                                                }))
                                            .item(PopupMenuItem::new("重命名")
                                                .on_click(move |_, _, cx| {
                                                    v3.update(cx, |view, cx| {
                                                        view.collection_menu_node_id = n3.clone();
                                                        view.open_rename(cx);
                                                    });
                                                }))
                                            .item(PopupMenuItem::new("删除")
                                                .on_click(move |_, _, cx| {
                                                    v4.update(cx, |view, _cx| {
                                                        view.collection_menu_node_id = n4.clone();
                                                        view.delete_selected_collection_item();
                                                    });
                                                }))
                                    }
                                }),
                        );
                    } else {
                        let req_icon = if has_scenarios {
                            Icon::new(
                                if entry.is_expanded() { IconName::ChevronDown } else { IconName::ChevronRight },
                            )
                            .size(px(12.0))
                            .text_color(ui::text_tertiary(cx))
                            .into_any_element()
                        } else {
                            Icon::new(IconName::SquareTerminal)
                                .size(px(12.0))
                                .text_color(ui::text_tertiary(cx))
                                .into_any_element()
                        };
                        list_item = list_item.child(
                            div()
                                .id(("req-item", ix))
                                .px(px(6.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
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
                                        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                            cx.stop_propagation();
                                            ts2.update(cx, |tree, cx| {
                                                tree.set_selected_index(Some(ix), cx);
                                            });
                                            v.update(cx, |view, cx| view.select_request(req_idx, cx));
                                            window.refresh();
                                        })
                                })
                                .context_menu({
                                    let v = view.clone();
                                    let nid = node_id.clone();
                                    move |menu, _window, _| {
                                        let v1 = v.clone(); let v2 = v.clone();
                                        let v3 = v.clone(); let v4 = v.clone();
                                        let n1 = nid.clone(); let n2 = nid.clone();
                                        let n3 = nid.clone(); let n4 = nid.clone();
                                        menu
                                            .item(PopupMenuItem::new("新建用例")
                                                .on_click(move |_, _, cx| {
                                                    v1.update(cx, |view, _cx| {
                                                        view.collection_menu_node_id = n1.clone();
                                                        view.create_new_case();
                                                    });
                                                }))
                                            .item(PopupMenuItem::new("复制路径")
                                                .on_click(move |_, _, cx| {
                                                    let nd = n2.clone();
                                                    v2.update(cx, |view, cx| {
                                                        if !nd.is_empty() {
                                                            if let Ok(Some(node)) = view.service.get_collection_node(&nd) {
                                                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(node.url.clone()));
                                                                view.notice = format!("已复制: {}", node.url);
                                                            }
                                                        }
                                                    });
                                                }))
                                            .item(PopupMenuItem::new("重命名")
                                                .on_click(move |_, _, cx| {
                                                    v3.update(cx, |view, cx| {
                                                        view.collection_menu_node_id = n3.clone();
                                                        view.open_rename(cx);
                                                    });
                                                }))
                                            .item(PopupMenuItem::new("删除")
                                                .on_click(move |_, _, cx| {
                                                    v4.update(cx, |view, _cx| {
                                                        view.collection_menu_node_id = n4.clone();
                                                        view.delete_selected_collection_item();
                                                    });
                                                }))
                                    }
                                }),
                        );
                    }
                }

                list_item
            }),
        )
}