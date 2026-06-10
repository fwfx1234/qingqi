//! 终端面板

use gpui::*;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::ui;

use super::TerminalViewModel;

pub fn render_terminal(
    term: &TerminalViewModel,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    div()
        .flex_1().flex().flex_col().bg(ui::bg_surface())
        .child(render_status_bar(term))
        .child(render_content(term, cx))
}

fn render_status_bar(term: &TerminalViewModel) -> impl IntoElement {
    div()
        .h(px(28.0)).flex().items_center().px_2()
        .border_b_1().border_color(ui::border_light())
        .child(div().flex().items_center().gap(px(6.0))
            .child(div().text_size(px(11.0)).text_color(ui::text_secondary()).child(term.status.clone())))
        .child(div().flex_1())
}

fn render_content(
    term: &TerminalViewModel,
    cx: &mut Context<super::SshView>,
) -> impl IntoElement {
    div()
        .id("terminal-content")
        .flex_1().overflow_y_scrollbar().p_2().font_family("Menlo")
        .track_focus(&cx.focus_handle())
        .on_key_down(cx.listener(|view, event: &KeyDownEvent, _w, cx| {
            if let Some(sid) = view.selected_session_id {
                // 将击键转为字节发送到终端
                let bytes: Vec<u8> = match &event.keystroke.key_char {
                    Some(c) if !c.is_empty() => c.as_bytes().to_vec(),
                    Some(_) | None => {
                        match event.keystroke.key.as_str() {
                            "enter" => b"\r".to_vec(),
                            "backspace" => b"\x7f".to_vec(),
                            "tab" => b"\t".to_vec(),
                            "escape" => b"\x1b".to_vec(),
                            "up" => b"\x1b[A".to_vec(),
                            "down" => b"\x1b[B".to_vec(),
                            "right" => b"\x1b[C".to_vec(),
                            "left" => b"\x1b[D".to_vec(),
                            _ => vec![],
                        }
                    }
                };
                if !bytes.is_empty() {
                    let _ = view.service.send_terminal_input(&sid, &bytes);
                }
            }
            cx.notify();
        }))
        .children(term.lines.iter().map(|line| {
            let is_log = matches!(term.terminal_kind, crate::model::TerminalKind::Log);
            let mut el = div().text_size(px(12.0)).h(px(18.0))
                .child(if is_log { format!("  {}", line.text.clone()) } else { line.text.clone() });
            if let Some(color) = line.fg_color {
                el = el.text_color(hsla(color[0], color[1], color[2], color[3]));
            }
            el
        }))
}
