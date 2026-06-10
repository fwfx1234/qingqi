//! 文件树面板 — 白底 + SVG 图标

use gpui::*;
use gpui_component::{Icon, IconName};
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::ui;

use super::FileTreeViewModel;

pub fn render_file_tree(tree: &FileTreeViewModel) -> impl IntoElement {
    div()
        .flex_1().flex().flex_col()
        .bg(ui::bg_surface())
        .border_r_1().border_color(ui::border_light())
        .child(render_toolbar(tree))
        .child(render_entry_list(tree))
}

fn render_toolbar(tree: &FileTreeViewModel) -> impl IntoElement {
    div()
        .h(px(32.0)).flex().items_center().px_2().gap(px(4.0))
        .border_b_1().border_color(ui::border_light())
        .bg(ui::bg_surface())
        .child(div().text_size(px(11.0)).text_color(ui::text_secondary()).child(tree.current_path.clone()))
        .child(div().flex_1())
        .child(render_tool_button("上传", "btn-upload"))
        .child(render_tool_button("刷新", "btn-refresh"))
        .child(render_tool_button("新建", "btn-mkdir"))
}

fn render_tool_button(label: &'static str, id: &'static str) -> impl IntoElement {
    div()
        .id(id).px_2().py(px(2.0)).rounded(px(4.0))
        .text_size(px(11.0)).text_color(ui::text_secondary()).cursor_pointer()
        .hover(|s| s.bg(ui::bg_hover()).text_color(ui::text_primary()))
        .child(label)
}

fn render_entry_list(tree: &FileTreeViewModel) -> impl IntoElement {
    let count = tree.entries.len();
    if count > 50 {
        let entries: Vec<_> = tree.entries.iter().map(|e| {
            (e.name.clone(), e.is_dir, e.size_text.clone(), e.is_selected)
        }).collect();
        uniform_list("ssh-file-list", count, move |range, _w, _cx| {
            entries[range].iter().map(|(name, is_dir, size_text, is_selected)| {
                file_row(name, *is_dir, size_text, *is_selected)
            }).collect::<Vec<_>>()
        }).flex_1().into_any_element()
    } else {
        div().flex_1().overflow_y_scrollbar()
            .children(tree.entries.iter().map(|e| {
                file_row(&e.name, e.is_dir, &e.size_text, e.is_selected)
            }))
            .into_any_element()
    }
}

fn file_row(name: &str, is_dir: bool, size_text: &str, is_selected: bool) -> AnyElement {
    div()
        .h(px(26.0)).flex().items_center().px_2().text_size(px(12.0)).cursor_pointer()
        .bg(if is_selected { hsla(0.55, 0.3, 0.4, 0.12) } else { hsla(0.0, 0.0, 0.0, 0.0) })
        .hover(|s| {
            if !is_selected { s.bg(hsla(0.55, 0.1, 0.5, 0.06)) } else { s }
        })
        .child(
            div().flex().items_center().gap(px(6.0))
                .child(if is_dir {
                    Icon::new(IconName::Folder).size(px(14.0)).text_color(ui::text_secondary()).into_any_element()
                } else {
                    Icon::new(IconName::File).size(px(14.0)).text_color(ui::text_secondary()).into_any_element()
                })
                .child(div().child(name.to_string())),
        )
        .child(div().flex_1())
        .child(div().text_size(px(10.0)).text_color(ui::text_secondary()).child(size_text.to_string()))
        .into_any_element()
}
