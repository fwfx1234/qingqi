use gpui::{
    App, Entity, IntoElement, InteractiveElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px, prelude::FluentBuilder,
};
use gpui_component::{IconName, Sizable, Size, button::{Button, ButtonVariants}};
use qingqi_ui::{theme, ui};
use crate::view::ApiDebuggerView;
use super::collection_tree::MenuKind;
use super::shared::api_accent;

pub fn context_menu_overlay(
    view: Entity<ApiDebuggerView>,
    title: String,
    position: Option<(f32, f32)>,
    node_id: String,
    kind: MenuKind,
    _dark: bool,
) -> impl IntoElement {
    let (x, y) = position.unwrap_or((248.0, 96.0));
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .id("api-collection-menu-backdrop")
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(gpui::transparent_black())
                .on_click({
                    let view = view.clone();
                    move |_, window, cx| {
                        view.update(cx, |view, _cx| view.close_collection_menu());
                        window.refresh();
                    }
                }),
        )
        .child(
            div()
                .absolute()
                .top(px(y))
                .left(px(x))
                .w(px(230.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::semantic().bg_surface)
                .rounded(px(8.0))
                .shadow_md()
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .flex()
                .flex_col()
                .child(menu_header(title, kind))
                .children(build_menu_items(view.clone(), kind, node_id)),
        )
}

fn menu_header(title: String, kind: MenuKind) -> impl IntoElement {
    let icon = match kind {
        MenuKind::Folder => IconName::Folder,
        MenuKind::Request => IconName::SquareTerminal,
        MenuKind::Scenario => IconName::SquareTerminal,
    };
    div()
        .px(px(12.0))
        .py(px(9.0))
        .border_b_1()
        .border_color(ui::border_light())
        .text_size(px(13.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(api_accent())
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            Button::new("api-collection-menu-icon")
                .ghost()
                .icon(icon)
                .with_size(Size::XSmall),
        )
        .child(title)
}

fn build_menu_items(
    view: Entity<ApiDebuggerView>,
    kind: MenuKind,
    node_id: String,
) -> Vec<gpui::AnyElement> {
    match kind {
        MenuKind::Folder => folder_menu_items(view, node_id),
        MenuKind::Request => request_menu_items(view, node_id),
        MenuKind::Scenario => scenario_menu_items(view, node_id),
    }
}

fn folder_menu_items(
    view: Entity<ApiDebuggerView>,
    _node_id: String,
) -> Vec<gpui::AnyElement> {
    vec![
        menu_item(
            "api-collection-menu-new-folder",
            "新建文件夹",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.create_new_folder());
                }
            },
        ),
        menu_item(
            "api-collection-menu-new-request",
            "新建接口",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.create_new_endpoint());
                }
            },
        ),
        menu_separator(),
        menu_item(
            "api-collection-menu-import-curl",
            "导入 cURL",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| {
                        view.show_curl_import = true;
                    });
                }
            },
        ),
        menu_item(
            "api-collection-menu-import-openapi",
            "导入 OpenAPI",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.import_openapi_file());
                }
            },
        ),
        menu_item(
            "api-collection-menu-import-postman",
            "导入 Postman",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.import_postman_file());
                }
            },
        ),
        menu_item(
            "api-collection-menu-export-openapi",
            "导出为 OpenAPI",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.export_openapi());
                }
            },
        ),
        menu_separator(),
        menu_item(
            "api-collection-menu-rename",
            "重命名",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, cx| view.open_rename(cx));
                }
            },
        ),
        menu_separator(),
        menu_item(
            "api-collection-menu-delete",
            "删除",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.delete_selected_collection_item());
                }
            },
        ),
    ]
}

fn request_menu_items(
    view: Entity<ApiDebuggerView>,
    node_id: String,
) -> Vec<gpui::AnyElement> {
    vec![
        menu_item(
            "api-collection-menu-new-case",
            "新建用例",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.create_new_case());
                }
            },
        ),
        menu_separator(),
        menu_item(
            "api-collection-menu-copy-path",
            "复制路径",
            "",
            {
                let view = view.clone();
                let nid = node_id.clone();
                move |_, cx| {
                    let url = if !nid.is_empty() {
                        let api_view = view.read(cx);
                        if let Ok(Some(node)) =
                            api_view.service.get_collection_node(&nid)
                        {
                            Some(node.url.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(url) = url {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(url.clone()));
                        view.update(cx, |view, _cx| {
                            view.notice = format!("已复制: {}", url);
                            view.close_collection_menu();
                        });
                    } else {
                        view.update(cx, |view, _cx| {
                            view.notice = String::from("接口未找到");
                            view.close_collection_menu();
                        });
                    }
                }
            },
        ),
        menu_separator(),
        menu_item(
            "api-collection-menu-rename",
            "重命名",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, cx| view.open_rename(cx));
                }
            },
        ),
        menu_separator(),
        menu_item(
            "api-collection-menu-delete",
            "删除",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.delete_selected_collection_item());
                }
            },
        ),
    ]
}

fn scenario_menu_items(
    view: Entity<ApiDebuggerView>,
    _node_id: String,
) -> Vec<gpui::AnyElement> {
    vec![
        menu_item(
            "api-collection-menu-rename",
            "重命名",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, cx| view.open_rename(cx));
                }
            },
        ),
        menu_separator(),
        menu_item(
            "api-collection-menu-delete",
            "删除",
            "",
            {
                let view = view.clone();
                move |_, cx| {
                    view.update(cx, |view, _cx| view.delete_selected_collection_item());
                }
            },
        ),
    ]
}

fn menu_item(
    id: &'static str,
    label: &'static str,
    shortcut: &'static str,
    on_click: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> gpui::AnyElement {
    div()
        .id(id)
        .px(px(12.0))
        .py(px(8.0))
        .text_size(px(11.0))
        .text_color(theme::semantic().text_body)
        .hover(move |style| {
            style
                .bg(theme::rgba_with_alpha(api_accent(), 0.06))
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(div().flex_1().child(label))
        .when(!shortcut.is_empty(), |row| {
            row.child(
                div()
                    .text_size(px(10.0))
                    .text_color(ui::text_tertiary())
                    .child(shortcut),
            )
        })
        .on_click(move |event, _window, cx| {
            cx.stop_propagation();
            on_click(event, cx)
        })
        .into_any_element()
}

fn menu_separator() -> gpui::AnyElement {
    div().h(px(1.0)).bg(ui::border_light()).into_any_element()
}
