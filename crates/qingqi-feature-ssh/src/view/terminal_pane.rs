//! 终端面板辅助函数

use gpui::*;
use qingqi_ui::ui;

use super::TerminalViewModel;
use crate::terminal::TerminalLine;

pub(crate) const TERM_FONT: &str = "Menlo";
pub(crate) const TERM_PADDING: f32 = 12.0;
pub(crate) const CHAR_WIDTH_RATIO: f32 = 0.6;
pub(crate) const LINE_HEIGHT_RATIO: f32 = 1.4;
fn term_text() -> Hsla {
    hsla(0.0, 0.0, 0.12, 1.0)
}

fn line_text_color(line: &TerminalLine) -> Hsla {
    line.fg_color
        .map(|[h, s, l, a]| hsla(h, s, l, a))
        .unwrap_or_else(term_text)
}

pub(crate) fn render_status_bar(term: &TerminalViewModel) -> impl IntoElement {
    div()
        .h(px(30.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_3()
        .bg(hsla(0.0, 0.0, 0.97, 1.0))
        .border_b_1()
        .border_color(ui::border_light())
        .child(div().size(px(6.0)).rounded_full().bg(ui::success()).mr_2())
        .child(
            div()
                .text_size(px(11.0))
                .font_family(TERM_FONT)
                .text_color(ui::text_secondary())
                .child(term.status.clone()),
        )
        .child(div().flex_1())
}

pub(crate) fn scroll_delta_from_wheel(event: &ScrollWheelEvent, line_height: f32) -> i32 {
    let line_height_px = px(line_height);
    let pixel_delta = event.delta.pixel_delta(line_height_px);
    if pixel_delta.y == px(0.0) {
        return 0;
    }
    let lines = (f32::from(pixel_delta.y) / f32::from(line_height_px)).round() as i32;
    if lines == 0 {
        if pixel_delta.y > px(0.0) { 1 } else { -1 }
    } else {
        lines
    }
}

fn log_placeholder(term: &TerminalViewModel) -> String {
    if !term.status.is_empty() {
        term.status.clone()
    } else {
        "等待连接日志…".into()
    }
}

pub(crate) fn render_log_body(
    term: &TerminalViewModel,
    font_size: f32,
    line_height: f32,
) -> AnyElement {
    if term.lines.is_empty() {
        return div()
            .font_family(TERM_FONT)
            .text_size(px(font_size))
            .line_height(px(line_height))
            .text_color(ui::text_secondary())
            .child(log_placeholder(term))
            .into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .w_full()
        .children(term.lines.iter().map(|line| {
            div()
                .font_family(TERM_FONT)
                .text_size(px(font_size))
                .line_height(px(line_height))
                .text_color(line_text_color(line))
                .child(line.text.clone())
        }))
        .into_any_element()
}
