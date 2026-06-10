//! 传输记录面板

use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::ui;

use super::TransferPanelViewModel;

pub fn render_transfer_panel(
    transfers: &TransferPanelViewModel,
    expanded: bool,
) -> impl IntoElement {
    div()
        .w_full()
        .border_t_1()
        .border_color(ui::border_light())
        .bg(ui::bg_surface())
        .child(render_control_bar(transfers, expanded))
        .when(expanded, |root| root.child(render_transfer_list(transfers)))
}

fn render_control_bar(
    transfers: &TransferPanelViewModel,
    expanded: bool,
) -> impl IntoElement {
    div()
        .h(px(36.0))
        .flex()
        .items_center()
        .px_3()
        .justify_between()
        .cursor_pointer()
        .hover(|s| s.bg(ui::bg_hover()))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(ui::text_secondary())
                .child(format!(
                    "传输记录 ({} 进行中, {} 已完成, {} 失败)",
                    transfers.active_count,
                    transfers.completed_count,
                    transfers.failed_count,
                )),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(ui::text_secondary())
                .child(if expanded { "收起 ▲" } else { "展开 ▼" }),
        )
}

fn render_transfer_list(transfers: &TransferPanelViewModel) -> impl IntoElement {
    if transfers.rows.is_empty() {
        return div()
            .h(px(200.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .text_color(ui::text_secondary())
            .child("暂无传输记录")
            .into_any_element();
    }

    div()
        .h(px(200.0))
        .overflow_y_scrollbar()
        .children(transfers.rows.iter().map(render_transfer_row))
        .into_any_element()
}

fn render_transfer_row(
    row: &super::TransferRowViewModel,
) -> impl IntoElement {
    div()
        .h(px(32.0))
        .flex()
        .items_center()
        .px_3()
        .text_size(px(12.0))
        .hover(|s| s.bg(ui::bg_hover()))
        .child(
            div()
                .mr_2()
                .text_size(px(14.0))
                .child(row.direction_icon),
        )
        .child(
            div()
                .flex_1()
                .child(row.file_name.clone()),
        )
        .child(render_progress_bar(row))
        .child(
            div()
                .mr_2()
                .ml_2()
                .text_size(px(11.0))
                .text_color(row.status_color)
                .child(row.status_text.clone()),
        )
}

fn render_progress_bar(
    row: &super::TransferRowViewModel,
) -> impl IntoElement {
    div()
        .w(px(120.0))
        .h(px(6.0))
        .rounded_full()
        .bg(hsla(0.0, 0.0, 0.0, 0.1))
        .child(
            div()
                .h_full()
                .rounded_full()
                .bg(row.status_color)
                .w(px(120.0 * row.progress_percent as f32 / 100.0)),
        )
}
