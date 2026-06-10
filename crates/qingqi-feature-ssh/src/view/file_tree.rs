//! 文件树面板

use gpui::*;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Icon, IconName};
use qingqi_ui::ui;

use super::{FileEntryRow, FileTreeViewModel};

const MONO: &str = "Menlo";
const COL_SIZE: f32 = 56.0;
const COL_TIME: f32 = 96.0;

pub fn render_file_tree(
    tree: &FileTreeViewModel,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let handle = cx.entity().clone();
    div()
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
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |view, event: &MouseDownEvent, _w, cx| {
                view.open_file_context_menu(None, event.position, cx);
            }),
        )
        .child(render_toolbar(tree, cx))
        .child(render_list_header())
        .child(render_entry_list(tree, handle))
}

fn render_toolbar(tree: &FileTreeViewModel, cx: &mut Context<super::SshView>) -> impl IntoElement {
    div()
        .h(px(34.0))
        .flex()
        .items_center()
        .px_3()
        .gap(px(6.0))
        .border_b_1()
        .border_color(ui::border_light())
        .bg(hsla(0.0, 0.0, 0.98, 1.0))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(11.0))
                .font_family(MONO)
                .text_color(ui::text_secondary())
                .truncate()
                .child(tree.current_path.clone()),
        )
        .child(
            div()
                .id("btn-refresh")
                .px_2()
                .py(px(3.0))
                .rounded(px(5.0))
                .text_size(px(11.0))
                .text_color(ui::text_secondary())
                .cursor_pointer()
                .hover(|s| s.bg(ui::bg_hover()).text_color(ui::text_primary()))
                .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| {
                    view.refresh_file_tree(cx);
                }))
                .child("刷新"),
        )
}

fn render_list_header() -> impl IntoElement {
    div()
        .w_full()
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .bg(hsla(0.0, 0.0, 0.96, 1.0))
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
    handle: Entity<super::SshView>,
) -> impl IntoElement {
    let count = tree.entries.len();
    if count == 0 {
        return div()
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
                        "连接后显示远程文件".to_string()
                    } else {
                        "目录为空".to_string()
                    }),
            )
            .into_any_element();
    }
    if count > 50 {
        let entries: Vec<FileEntryRow> = tree.entries.clone();
        let list_handle = handle.clone();
        uniform_list("ssh-file-list", count, move |range, _w, _cx| {
            let start = range.start;
            entries[range]
                .iter()
                .enumerate()
                .map(|(i, e)| file_row(e, start + i, list_handle.clone()))
                .collect::<Vec<_>>()
        })
        .flex_1()
        .into_any_element()
    } else {
        div()
            .flex_1()
            .overflow_y_scrollbar()
            .children(
                tree.entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| file_row(e, i, handle.clone())),
            )
            .into_any_element()
    }
}

fn file_row(entry: &FileEntryRow, index: usize, handle: Entity<super::SshView>) -> AnyElement {
    let is_parent = entry.is_parent;
    let is_dir = entry.is_dir;
    let is_selected = entry.is_selected;
    let entry_for_click = entry.clone();
    let entry_for_menu = entry.clone();
    let h = handle.clone();
    let h_menu = handle;

    div()
        .id(("ssh-file", index as u64))
        .w_full()
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .text_size(px(12.0))
        .cursor_pointer()
        .border_b_1()
        .border_color(hsla(0.0, 0.0, 0.0, 0.04))
        .bg(if is_selected {
            hsla(0.55, 0.35, 0.55, 0.10)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .hover(|s| {
            if !is_selected {
                s.bg(hsla(0.0, 0.0, 0.5, 0.04))
            } else {
                s
            }
        })
        .on_click({
            let entry = entry_for_click.clone();
            move |event: &ClickEvent, _: &mut Window, cx: &mut App| {
                if event.click_count() >= 2 && (is_dir || is_parent) {
                    h.update(cx, |view, cx| {
                        view.open_file_entry(&entry, cx);
                    });
                }
            }
        })
        .on_mouse_down(
            MouseButton::Right,
            move |event: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                h_menu.update(cx, |view, cx| {
                    view.open_file_context_menu(Some(entry_for_menu.clone()), event.position, cx);
                });
            },
        )
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
