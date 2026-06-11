//! 文件树面板

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::menu::ContextMenuExt;
use gpui_component::{Icon, IconName};

use super::file_context_menu;
use super::virtual_list;
use qingqi_plugin::plugin_spec::PluginAccent;
use qingqi_ui::text_input::TextInput;
use qingqi_ui::{theme, ui};

use crate::model::SessionId;

use super::{FileEntryRow, FileTreeViewModel};

const ACCENT: PluginAccent = PluginAccent::Cyan;
const MONO: &str = "Menlo";
const COL_SIZE: f32 = 56.0;
const COL_TIME: f32 = 96.0;
const ROW_HEIGHT: f32 = 28.0;

pub fn render_file_tree(
    tree: &FileTreeViewModel,
    session_id: Option<SessionId>,
    list_scroll: UniformListScrollHandle,
    path_input: Entity<TextInput>,
    follow_terminal: bool,
    can_follow_terminal: bool,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let handle = cx.entity().clone();
    let accent = ui::accent_color(ACCENT);
    let tree_key = session_id.map(|id| id.0.as_u128()).unwrap_or(0);
    let tree_id = (tree_key as u64) ^ ((tree_key >> 64) as u64);
    div()
        .id(("ssh-file-tree", tree_id))
        .w(px(360.0))
        .min_w(px(240.0))
        .max_w(px(420.0))
        .h_full()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .bg(ui::bg_surface())
        .border_r_1()
        .border_color(ui::border_light())
        .can_drop(|payload, _, _| payload.is::<ExternalPaths>())
        .drag_over::<ExternalPaths>(move |style, _, _, _| {
            style
                .bg(theme::rgba_with_alpha(accent, 0.08))
                .border_color(accent)
        })
        .on_drop(cx.listener(|view, paths: &ExternalPaths, _, cx| {
            view.upload_local_paths(paths.paths(), cx);
        }))
        .child(render_toolbar(
            path_input,
            follow_terminal,
            can_follow_terminal,
            cx,
        ))
        .child(render_list_header())
        .child(render_entry_list(tree, tree_id, list_scroll, handle))
}

fn render_toolbar(
    path_input: Entity<TextInput>,
    follow_terminal: bool,
    can_follow_terminal: bool,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let accent = ui::accent_color(ACCENT);
    let accent_soft = theme::rgba_with_alpha(accent, 0.14);

    div()
        .h(px(36.0))
        .flex()
        .items_center()
        .px_3()
        .gap(px(6.0))
        .border_b_1()
        .border_color(ui::border_light())
        .bg(ui::bg_surface())
        .child(path_slot(path_input))
        .when(can_follow_terminal, |el| {
            el.child(
                div()
                    .id("btn-follow-terminal")
                    .flex_shrink_0()
                    .px_2()
                    .py(px(3.0))
                    .rounded(px(5.0))
                    .text_size(px(11.0))
                    .cursor_pointer()
                    .bg(if follow_terminal {
                        accent_soft
                    } else {
                        hsla(0.0, 0.0, 0.0, 0.0)
                    })
                    .text_color(if follow_terminal {
                        accent
                    } else {
                        ui::text_secondary()
                    })
                    .hover(|s| s.bg(ui::bg_hover()))
                    .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| {
                        view.toggle_follow_terminal(cx);
                    }))
                    .child("跟随终端"),
            )
        })
        .child(
            div()
                .id("btn-jump-path")
                .flex_shrink_0()
                .px_2()
                .py(px(3.0))
                .rounded(px(5.0))
                .text_size(px(11.0))
                .text_color(ui::text_secondary())
                .cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()).text_color(ui::text_primary()))
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| {
                    view.jump_to_path_in_input(cx);
                }))
                .child("跳转"),
        )
}

fn path_slot(path_input: Entity<TextInput>) -> impl IntoElement {
    let focus_target = path_input.clone();
    div()
        .id("file-path-input")
        .flex_1()
        .min_w(px(0.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            window.focus(&focus_target.read(cx).focus_handle(cx));
        })
        .child(path_input)
}

fn render_list_header() -> impl IntoElement {
    let header_bg = theme::rgba_with_alpha(ui::text_tertiary(), 0.06);
    div()
        .w_full()
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .bg(header_bg)
        .border_b_1()
        .border_color(ui::border_light())
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(10.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ui::text_tertiary())
                .child("名称"),
        )
        .child(meta_group_header())
}

fn meta_group_header() -> impl IntoElement {
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .child(meta_header("大小", COL_SIZE))
        .child(meta_header("修改时间", COL_TIME))
}

fn meta_header(label: &'static str, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .flex()
        .justify_end()
        .text_size(px(10.0))
        .font_weight(FontWeight::MEDIUM)
        .font_family(MONO)
        .text_color(ui::text_tertiary())
        .child(label)
}

fn render_entry_list(
    tree: &FileTreeViewModel,
    tree_id: u64,
    list_scroll: UniformListScrollHandle,
    handle: Entity<super::SshView>,
) -> impl IntoElement {
    if tree.entries.is_empty() {
        return file_list_shell(
            handle,
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .px(px(16.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::text_tertiary())
                        .child(if tree.current_path.is_empty() {
                            "连接后显示远程文件，可拖入本地文件上传".to_string()
                        } else {
                            "目录为空，可拖入本地文件上传".to_string()
                        }),
                ),
        )
        .into_any_element();
    }

    let count = tree.entries.len();
    let entries: Vec<FileEntryRow> = tree.entries.clone();
    let list_handle = handle.clone();

    file_list_shell(
        handle,
        virtual_list::vertical(
            ("ssh-file-list", tree_id),
            count,
            list_scroll,
            move |range, _window, _cx| {
                range
                    .clone()
                    .map(|i| file_row(&entries[i], i, list_handle.clone()))
                    .collect()
            },
        ),
    )
    .into_any_element()
}

/// 文件列表唯一右键菜单入口，避免父子元素重复挂载导致菜单叠影。
fn file_list_shell(handle: Entity<super::SshView>, child: impl IntoElement) -> impl IntoElement {
    let menu_handle = handle.clone();
    div()
        .id("ssh-file-list-area")
        .flex_1()
        .min_h(px(0.0))
        .on_mouse_down(MouseButton::Right, {
            let h = handle.clone();
            move |_, _, cx| {
                h.update(cx, |view, _| view.set_file_context_target(None));
            }
        })
        .context_menu(move |menu, _window, cx| {
            let target = menu_handle.update(cx, |view, _| view.take_file_context_target());
            file_context_menu::build(menu, target, menu_handle.clone())
        })
        .child(child)
}

fn file_row(entry: &FileEntryRow, index: usize, handle: Entity<super::SshView>) -> AnyElement {
    let is_parent = entry.is_parent;
    let is_dir = entry.is_dir;
    let is_selected = entry.is_selected;
    let entry_for_click = entry.clone();
    let entry_for_menu = entry.clone();
    let h = handle.clone();
    let h_select = handle.clone();

    div()
        .id(("ssh-file", index as u64))
        .w_full()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .text_size(px(12.0))
        .cursor_pointer()
        .border_b_1()
        .border_color(theme::rgba_with_alpha(ui::text_tertiary(), 0.08))
        .bg(if is_selected {
            theme::rgba_with_alpha(ui::accent_color(ACCENT), 0.10)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .hover(|s| {
            if !is_selected {
                s.bg(ui::bg_hover())
            } else {
                s
            }
        })
        .on_mouse_down(MouseButton::Right, {
            let entry = entry_for_menu.clone();
            let h = handle.clone();
            move |_, _, cx| {
                h.update(cx, |view, _| {
                    view.set_file_context_target(Some(entry.clone()));
                });
                cx.stop_propagation();
            }
        })
        .on_click({
            let entry = entry_for_click.clone();
            let h_select_click = h_select.clone();
            move |event: &ClickEvent, _: &mut Window, cx: &mut App| {
                if event.click_count() >= 2 {
                    h.update(cx, |view, cx| {
                        view.open_file_entry(&entry, cx);
                    });
                } else if event.click_count() == 1 {
                    h_select_click.update(cx, |view, cx| {
                        view.select_file_entry(&entry, cx);
                    });
                }
            }
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(if is_parent {
                    Icon::new(IconName::ChevronLeft)
                        .size(px(14.0))
                        .text_color(ui::text_secondary())
                        .into_any_element()
                } else if is_dir {
                    Icon::new(IconName::Folder)
                        .size(px(14.0))
                        .text_color(hsla(0.12, 0.7, 0.5, 1.0))
                        .into_any_element()
                } else {
                    Icon::new(IconName::File)
                        .size(px(14.0))
                        .text_color(ui::text_tertiary())
                        .into_any_element()
                })
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_align(TextAlign::Left)
                        .font_weight(if is_parent {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if is_dir || is_parent {
                            ui::text_primary()
                        } else {
                            ui::text_secondary()
                        })
                        .child(entry.name.clone()),
                ),
        )
        .child(meta_group_cells(
            if is_parent { "" } else { &entry.size_text },
            if is_parent { "" } else { &entry.modified_text },
        ))
        .into_any_element()
}

fn meta_group_cells(size: &str, modified: &str) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .child(meta_cell(size, COL_SIZE))
        .child(meta_cell(modified, COL_TIME))
}

fn meta_cell(text: &str, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .flex()
        .justify_end()
        .text_size(px(10.0))
        .font_family(MONO)
        .text_color(ui::text_tertiary())
        .child(text.to_string())
}
