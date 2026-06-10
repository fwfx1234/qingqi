//! 文件树面板

use gpui::*;
use gpui::prelude::*;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::ui;

use super::FileTreeViewModel;

pub fn render_file_tree(tree: &FileTreeViewModel) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(ui::border_light())
        .child(render_toolbar(tree))
        .child(render_entry_list(tree))
}

fn render_toolbar(tree: &FileTreeViewModel) -> impl IntoElement {
    div()
        .h(px(36.0))
        .flex()
        .items_center()
        .px_2()
        .gap(px(4.0))
        .border_b_1()
        .border_color(ui::border_light())
        .child(
            div()
                .text_size(px(12.0))
                .text_color(ui::text_secondary())
                .child(tree.current_path.clone()),
        )
        .child(div().flex_1())
        .child(render_tool_button("上传"))
        .child(render_tool_button("刷新"))
        .child(render_tool_button("新建文件夹"))
}

fn render_tool_button(label: &str) -> impl IntoElement {
    div()
        .px_2()
        .py(px(2.0))
        .rounded_sm()
        .text_size(px(11.0))
        .text_color(ui::text_secondary())
        .cursor_pointer()
        .hover(|s| s.bg(ui::bg_hover()).text_color(ui::text_primary()))
        .child(label.to_string())
}

fn render_entry_list(tree: &FileTreeViewModel) -> impl IntoElement {
    let count = tree.entries.len();
    if count > 50 {
        let entries: Vec<_> = tree
            .entries
            .iter()
            .map(|e| {
                (
                    e.name.clone(),
                    e.is_dir,
                    e.size_text.clone(),
                    e.is_selected,
                )
            })
            .collect();
        uniform_list("ssh-file-list", count, move |range, _w, _cx| {
            entries[range]
                .iter()
                .map(|(name, is_dir, size_text, is_selected)| {
                    div()
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .px_2()
                        .text_size(px(12.0))
                        .cursor_pointer()
                        .bg(if *is_selected {
                            hsla(0.55, 0.3, 0.5, 0.15)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(|s| {
                            s.bg(hsla(0.55, 0.1, 0.5, 0.1))
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(if *is_dir { "📁" } else { "📄" })
                                .child(name.clone()),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(ui::text_secondary())
                                .child(size_text.clone()),
                        )
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        })
        .flex_1()
        .into_any_element()
    } else {
        div()
            .flex_1()
            .overflow_y_scrollbar()
            .children(tree.entries.iter().map(|e| {
                div()
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .px_2()
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .bg(if e.is_selected {
                        hsla(0.55, 0.3, 0.5, 0.15)
                    } else {
                        hsla(0.0, 0.0, 0.0, 0.0)
                    })
                    .hover(|s| {
                        s.bg(hsla(0.55, 0.1, 0.5, 0.1))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(if e.is_dir { "📁" } else { "📄" })
                            .child(e.name.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(ui::text_secondary())
                            .child(e.size_text.clone()),
                    )
            }))
            .into_any_element()
    }
}
