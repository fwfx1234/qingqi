//! 终端输入：Zed 风格 try_keystroke + 鼠标/粘贴辅助。

use std::borrow::Cow;

use gpui::*;

use crate::mappings::keys::to_esc_str;
use crate::terminal::TerminalModes;

pub fn paste_text_to_bytes(text: &str) -> Vec<u8> {
    text.bytes()
        .map(|b| if b == b'\n' { b'\r' } else { b })
        .collect()
}

pub fn wrap_bracketed_paste(bytes: Vec<u8>, modes: &TerminalModes) -> Vec<u8> {
    if !modes.bracketed_paste || bytes.is_empty() {
        return bytes;
    }
    let mut out = b"\x1b[200~".to_vec();
    out.extend(bytes);
    out.extend(b"\x1b[201~");
    out
}

/// SGR 鼠标序列：`\x1b[<btn;col;rowM/m`
pub fn encode_sgr_mouse(button: u8, col: usize, row: usize, release: bool) -> Vec<u8> {
    format!(
        "\x1b[{};{};{}{}",
        button,
        col + 1,
        row + 1,
        if release { 'm' } else { 'M' }
    )
    .into_bytes()
}

pub fn encode_sgr_wheel(up: bool, col: usize, row: usize) -> Vec<u8> {
    encode_sgr_mouse(if up { 64 } else { 65 }, col, row, false)
}

pub fn mouse_button_code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        _ => 0,
    }
}

/// 对齐 Zed `Terminal::try_keystroke`：可打印字符返回 `None`，交给 IME。
pub fn keystroke_to_bytes(keystroke: &Keystroke, modes: &TerminalModes) -> Option<Vec<u8>> {
    let esc = to_esc_str(keystroke, modes.app_cursor, false)?;
    Some(match esc {
        Cow::Borrowed(s) => s.as_bytes().to_vec(),
        Cow::Owned(s) => s.into_bytes(),
    })
}

pub fn terminal_editing_key(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        b"\t"
            | b"\x1b"
            | b"\x1b[A"
            | b"\x1b[B"
            | b"\x1b[C"
            | b"\x1b[D"
            | b"\x1b[H"
            | b"\x1b[F"
            | b"\x1b[5~"
            | b"\x1b[6~"
            | b"\x1b[3~"
    ) || bytes.first() == Some(&0x01)
        || bytes.starts_with(b"\x1b[1;")
        || bytes.starts_with(b"\x1b[2")
        || bytes.starts_with(b"\x1b[1")
        || bytes.starts_with(b"\x1bO")
}
