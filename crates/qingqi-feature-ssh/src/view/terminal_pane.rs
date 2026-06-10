//! 终端面板

use gpui::*;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::{theme, ui};

use super::TerminalViewModel;

const TERM_FONT: &str = "Menlo";

fn term_bg() -> Hsla {
    hsla(0.0, 0.0, 0.99, 1.0)
}

fn term_text() -> Hsla {
    hsla(0.0, 0.0, 0.12, 1.0)
}

pub fn render_terminal(
    term: &TerminalViewModel,
    font_size: f32,
    focus_handle: &FocusHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(ui::bg_surface())
        .child(render_status_bar(term))
        .child(render_content(term, font_size, focus_handle, cx))
}

fn render_status_bar(term: &TerminalViewModel) -> impl IntoElement {
    div()
        .h(px(30.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_3()
        .bg(hsla(0.0, 0.0, 0.97, 1.0))
        .border_b_1()
        .border_color(ui::border_light())
        .child(
            div()
                .size(px(6.0))
                .rounded_full()
                .bg(ui::success())
                .mr_2(),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family(TERM_FONT)
                .text_color(ui::text_secondary())
                .child(term.status.clone()),
        )
        .child(div().flex_1())
}

fn keystroke_to_bytes(event: &KeyDownEvent) -> Vec<u8> {
    let ks = &event.keystroke;
    if ks.modifiers.platform {
        return Vec::new();
    }
    if ks.modifiers.control {
        if let Some(byte) = ctrl_key_byte(&ks.key) {
            return vec![byte];
        }
    }
    if let Some(c) = &ks.key_char {
        if !c.is_empty() && !ks.modifiers.control {
            return c.as_bytes().to_vec();
        }
    }
    match ks.key.as_str() {
        "enter" | "return" => b"\r".to_vec(),
        "backspace" => b"\x7f".to_vec(),
        "delete" => b"\x1b[3~".to_vec(),
        "tab" => b"\t".to_vec(),
        "escape" => b"\x1b".to_vec(),
        "up" => b"\x1b[A".to_vec(),
        "down" => b"\x1b[B".to_vec(),
        "right" => b"\x1b[C".to_vec(),
        "left" => b"\x1b[D".to_vec(),
        "home" => b"\x1b[H".to_vec(),
        "end" => b"\x1b[F".to_vec(),
        "pageup" => b"\x1b[5~".to_vec(),
        "pagedown" => b"\x1b[6~".to_vec(),
        _ => Vec::new(),
    }
}

fn ctrl_key_byte(key: &str) -> Option<u8> {
    match key {
        "space" => Some(0),
        "@" => Some(0),
        "[" => Some(27),
        "\\" => Some(28),
        "]" => Some(29),
        "^" => Some(30),
        "_" => Some(31),
        _ if key.len() == 1 => {
            let c = key.chars().next()?;
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_alphabetic() {
                Some((lower as u8) - b'a' + 1)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn build_terminal_text(term: &TerminalViewModel) -> String {
    let is_log = matches!(term.terminal_kind, crate::model::TerminalKind::Log);
    let mut out = String::new();
    for (idx, line) in term.lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let mut row = if is_log {
            format!("  {}", line.text)
        } else {
            line.text.clone()
        };
        if term.cursor_visible && !is_log && idx == term.cursor_row {
            let col = term.cursor_col.min(row.chars().count());
            let byte_idx = row
                .char_indices()
                .nth(col)
                .map(|(i, _)| i)
                .unwrap_or(row.len());
            row.insert(byte_idx, '▍');
        }
        out.push_str(&row);
    }
    out
}

fn render_content(
    term: &TerminalViewModel,
    font_size: f32,
    focus_handle: &FocusHandle,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    let fh = focus_handle.clone();
    let line_height = (font_size * 1.4).max(15.0);
    let body = build_terminal_text(term);
    let has_content = !body.is_empty();

    div()
        .flex_1()
        .min_h(px(0.0))
        .min_w(px(0.0))
        .p_2()
        .overflow_hidden()
        .child(
            div()
                .id("terminal-content")
                .size_full()
                .min_h(px(0.0))
                .min_w(px(0.0))
                .rounded(theme::radius_md())
                .border_1()
                .border_color(hsla(0.0, 0.0, 0.88, 1.0))
                .bg(term_bg())
                .overflow_scrollbar()
                .p_3()
                .tab_index(0)
                .cursor_text()
                .track_focus(focus_handle)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _: &MouseDownEvent, window, cx| {
                        window.focus(&fh);
                        view.close_context_menu(cx);
                        view.close_file_context_menu(cx);
                    }),
                )
                .capture_key_down(cx.listener(|view, event: &KeyDownEvent, _w, cx| {
                    if !view.terminal_input_enabled() {
                        return;
                    }
                    let Some(sid) = view.selected_session_id else {
                        return;
                    };
                    let bytes = keystroke_to_bytes(event);
                    if !bytes.is_empty() {
                        let _ = view.service.send_terminal_input(&sid, &bytes);
                        view.refresh_ui_throttled(cx);
                    }
                }))
                .child(
                    div()
                        .font_family(TERM_FONT)
                        .text_size(px(font_size))
                        .line_height(px(line_height))
                        .text_color(term_text())
                        .whitespace_nowrap()
                        .child(if has_content {
                            body
                        } else {
                            "已连接，点击此处开始输入…".to_string()
                        }),
                ),
        )
}
