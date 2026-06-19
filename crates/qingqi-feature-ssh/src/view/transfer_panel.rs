//! 传输记录面板

use gpui::prelude::FluentBuilder;
use gpui::*;
use qingqi_ui::ui;

use super::virtual_list;

const ROW_HEIGHT: f32 = 32.0;
const LIST_HEIGHT: f32 = 200.0;

pub fn render_transfer_panel(
    transfers: &super::TransferPanelViewModel,
    expanded: bool,
    list_scroll: UniformListScrollHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    render_transfer_panel_inner(transfers, expanded, list_scroll, cx)
}

fn render_transfer_panel_inner(
    transfers: &super::TransferPanelViewModel,
    expanded: bool,
    list_scroll: UniformListScrollHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let hover_bg = ui::bg_hover(cx);
    div()
        .w_full()
        .border_t_1()
        .border_color(ui::border_light(cx))
        .bg(ui::bg_surface(cx))
        .child(render_control_bar(transfers, expanded, hover_bg, cx))
        .when(expanded, |root| {
            root.child(render_transfer_list(transfers, list_scroll, &*cx))
        })
}

fn render_control_bar(
    transfers: &super::TransferPanelViewModel,
    expanded: bool,
    hover_bg: Hsla,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    div()
        .id("transfer-toggle")
        .h(px(36.0))
        .flex()
        .items_center()
        .px_3()
        .justify_between()
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_click(cx.listener(|view, _: &ClickEvent, _w, cx| view.toggle_transfer_panel(cx)))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(ui::text_secondary(cx))
                .child(format!(
                    "传输记录 ({} 进行中, {} 已完成, {} 失败)",
                    transfers.active_count, transfers.completed_count, transfers.failed_count,
                )),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(ui::text_secondary(cx))
                .child(if expanded { "收起 ▲" } else { "展开 ▼" }),
        )
}

fn render_transfer_list(
    transfers: &super::TransferPanelViewModel,
    list_scroll: UniformListScrollHandle,
    cx: &App,
) -> impl IntoElement {
    if transfers.rows.is_empty() {
        return div()
            .h(px(LIST_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .text_color(ui::text_secondary(cx))
            .child("暂无传输记录")
            .into_any_element();
    }

    let rows: Vec<super::TransferRowViewModel> = transfers.rows.clone();
    let count = rows.len();

    virtual_list::vertical_fixed(
        "ssh-transfer-list",
        count,
        px(LIST_HEIGHT),
        list_scroll,
        move |range, _window, cx| {
            range
                .map(|i| render_transfer_row(&rows[i], cx).into_any_element())
                .collect()
        },
    )
    .into_any_element()
}

fn render_transfer_row(row: &super::TransferRowViewModel, cx: &App) -> impl IntoElement {
    let hover_bg = ui::bg_hover(cx);
    div()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .px_3()
        .text_size(px(12.0))
        .hover(move |s| s.bg(hover_bg))
        .child(div().mr_2().text_size(px(14.0)).child(row.direction_icon))
        .child(div().flex_1().child(row.file_name.clone()))
        .child(render_progress_bar(row))
        .child(
            div()
                .mr_2()
                .ml_2()
                .text_size(px(11.0))
                .text_color(row.status_color)
                .child(row.status_text.clone()),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_tertiary(cx))
                .child(row.speed_text.clone()),
        )
}

fn render_progress_bar(row: &super::TransferRowViewModel) -> impl IntoElement {
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
