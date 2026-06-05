use std::sync::{Arc, Mutex};

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::Dimensions,
    term::{
        Config as AlacrittyConfig, TermMode,
        cell::{Cell, Flags},
    },
    vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb},
};

use crate::model::TerminalId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalMode {
    Primary,
    Alternate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalCursorShape {
    Block,
    Underline,
    Beam,
    HollowBlock,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCursor {
    pub row: usize,
    pub column: usize,
    pub visible: bool,
    pub shape: TerminalCursorShape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalCellStyle {
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub inverse: bool,
    pub hidden: bool,
    pub strike: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCell {
    pub text: String,
    pub style: TerminalCellStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalStyledRow {
    pub cells: Vec<TerminalCell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TerminalInputState {
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    pub mouse_reporting: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalFrame {
    pub terminal_id: TerminalId,
    pub title: String,
    pub rows: Vec<String>,
    pub styled_rows: Vec<TerminalStyledRow>,
    pub cursor: TerminalCursor,
    pub mode: TerminalMode,
    pub input: TerminalInputState,
    pub columns: usize,
    pub screen_lines: usize,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalInput {
    Text(String),
    Paste(String),
    Enter,
    Tab,
    ShiftTab,
    Backspace,
    Delete,
    Insert,
    Escape,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Function(u8),
    Ctrl(char),
    Alt(char),
    CtrlAlt(char),
    MouseButton {
        button: TerminalMouseButton,
        column: u16,
        row: u16,
        kind: TerminalMouseEventKind,
        modifiers: TerminalMouseModifiers,
    },
    MouseMove {
        button: Option<TerminalMouseButton>,
        column: u16,
        row: u16,
        modifiers: TerminalMouseModifiers,
    },
    MouseScroll {
        direction: TerminalMouseScrollDirection,
        column: u16,
        row: u16,
        modifiers: TerminalMouseModifiers,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalMouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalMouseEventKind {
    Press,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalMouseScrollDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TerminalMouseModifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

#[derive(Clone, Default)]
struct ListenerState {
    title: Option<String>,
    pty_writes: Vec<String>,
}

#[derive(Clone, Default)]
struct TerminalEvents {
    state: Arc<Mutex<ListenerState>>,
}

impl TerminalEvents {
    fn title(&self) -> Option<String> {
        self.state.lock().ok().and_then(|state| state.title.clone())
    }

    fn drain_pty_writes(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.pty_writes))
            .unwrap_or_default()
    }
}

impl EventListener for TerminalEvents {
    fn send_event(&self, event: Event) {
        if let Ok(mut state) = self.state.lock() {
            match event {
                Event::Title(title) => state.title = Some(title),
                Event::ResetTitle => state.title = None,
                Event::PtyWrite(bytes) => state.pty_writes.push(bytes),
                _ => {}
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalSize {
    columns: usize,
    screen_lines: usize,
}

impl TerminalSize {
    fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns: columns.max(2),
            screen_lines: screen_lines.max(1),
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

pub struct TerminalEngine {
    terminal_id: TerminalId,
    fallback_title: String,
    events: TerminalEvents,
    parser: Processor,
    term: Term<TerminalEvents>,
    size: TerminalSize,
    revision: u64,
}

impl TerminalEngine {
    pub fn new(title: impl Into<String>) -> Self {
        Self::with_size(title, 96, 28)
    }

    pub fn with_size(title: impl Into<String>, columns: usize, screen_lines: usize) -> Self {
        let fallback_title = title.into();
        let events = TerminalEvents::default();
        let size = TerminalSize::new(columns, screen_lines);
        let mut config = AlacrittyConfig {
            scrolling_history: 10_000,
            ..AlacrittyConfig::default()
        };
        config.kitty_keyboard = true;
        let term = Term::new(config, &size, events.clone());
        Self {
            terminal_id: TerminalId::new(),
            fallback_title,
            events,
            parser: Processor::new(),
            term,
            size,
            revision: 1,
        }
    }

    pub fn demo(title: impl Into<String>, endpoint: impl AsRef<str>) -> Self {
        let title = title.into();
        let endpoint = endpoint.as_ref().to_string();
        let mut engine = Self::new(title.clone());
        let banner = format!(
            "\x1b[1;36m{title}\x1b[0m  connected to \x1b[32m{endpoint}\x1b[0m\r\n\
             \x1b[90mRemote terminal core: Alacritty parser/runtime active.\x1b[0m\r\n\
             $ "
        );
        engine.feed_bytes(banner.as_bytes());
        engine
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) -> TerminalFrame {
        self.parser.advance(&mut self.term, bytes);
        self.revision += 1;
        self.frame()
    }

    pub fn resize(&mut self, columns: usize, screen_lines: usize) -> TerminalFrame {
        self.size = TerminalSize::new(columns, screen_lines);
        self.term.resize(self.size);
        self.revision += 1;
        self.frame()
    }

    pub fn encode_input(&self, input: TerminalInput) -> Vec<u8> {
        encode_input(self.input_state(), input)
    }

    pub fn input_state(&self) -> TerminalInputState {
        let mode = self.term.mode();
        TerminalInputState {
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            mouse_reporting: mode.contains(TermMode::MOUSE_MODE),
        }
    }

    pub fn drain_pty_writes(&self) -> Vec<String> {
        self.events.drain_pty_writes()
    }

    /// 当前帧版本号，每次 feed_bytes/resize 自增。供 UI 侧去重，避免无变化重绘。
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn frame(&self) -> TerminalFrame {
        let display = self.term.renderable_content();
        let display_offset = display.display_offset as i32;
        let mut plain_rows = vec![vec![' '; self.size.columns()]; self.size.screen_lines()];
        let mut styled_rows = vec![
            TerminalStyledRow {
                cells: Vec::with_capacity(self.size.columns()),
            };
            self.size.screen_lines()
        ];

        for indexed in display.display_iter {
            let viewport_row = indexed.point.line.0 + display_offset;
            if viewport_row < 0 || viewport_row as usize >= self.size.screen_lines() {
                continue;
            }
            let row_index = viewport_row as usize;
            let column_index = indexed.point.column.0;
            let cell = indexed.cell;
            let text = display_text_for_cell(cell);
            let ch = text.chars().next().unwrap_or(' ');
            if column_index < self.size.columns() {
                plain_rows[row_index][column_index] = ch;
            }
            styled_rows[row_index].cells.push(TerminalCell {
                text,
                style: style_for_cell(cell, self.term.colors()),
            });
        }

        let rows = plain_rows
            .into_iter()
            .map(|row| {
                let line: String = row.into_iter().collect();
                line.trim_end().to_string()
            })
            .collect();

        let cursor = display.cursor;
        let cursor_viewport_row = cursor.point.line.0 + display_offset;
        let cursor_visible = cursor.shape != CursorShape::Hidden
            && cursor_viewport_row >= 0
            && (cursor_viewport_row as usize) < self.size.screen_lines();

        TerminalFrame {
            terminal_id: self.terminal_id.clone(),
            title: self
                .events
                .title()
                .unwrap_or_else(|| self.fallback_title.clone()),
            rows,
            styled_rows,
            cursor: TerminalCursor {
                row: cursor_viewport_row.max(0) as usize,
                column: cursor.point.column.0,
                visible: cursor_visible,
                shape: map_cursor_shape(cursor.shape),
            },
            mode: if display.mode.contains(TermMode::ALT_SCREEN) {
                TerminalMode::Alternate
            } else {
                TerminalMode::Primary
            },
            input: self.input_state(),
            columns: self.size.columns(),
            screen_lines: self.size.screen_lines(),
            revision: self.revision,
        }
    }
}

pub fn encode_input(state: TerminalInputState, input: TerminalInput) -> Vec<u8> {
    match input {
        TerminalInput::Text(text) => text.into_bytes(),
        TerminalInput::Paste(text) => {
            if state.bracketed_paste {
                format!("\x1b[200~{text}\x1b[201~").into_bytes()
            } else {
                text.into_bytes()
            }
        }
        TerminalInput::Enter => b"\r".to_vec(),
        TerminalInput::Tab => b"\t".to_vec(),
        TerminalInput::ShiftTab => b"\x1b[Z".to_vec(),
        TerminalInput::Backspace => vec![0x7f],
        TerminalInput::Delete => b"\x1b[3~".to_vec(),
        TerminalInput::Insert => b"\x1b[2~".to_vec(),
        TerminalInput::Escape => vec![0x1b],
        TerminalInput::Home => {
            if state.app_cursor {
                b"\x1bOH".to_vec()
            } else {
                b"\x1b[H".to_vec()
            }
        }
        TerminalInput::End => {
            if state.app_cursor {
                b"\x1bOF".to_vec()
            } else {
                b"\x1b[F".to_vec()
            }
        }
        TerminalInput::PageUp => b"\x1b[5~".to_vec(),
        TerminalInput::PageDown => b"\x1b[6~".to_vec(),
        TerminalInput::ArrowUp => arrow_seq(state.app_cursor, 'A'),
        TerminalInput::ArrowDown => arrow_seq(state.app_cursor, 'B'),
        TerminalInput::ArrowRight => arrow_seq(state.app_cursor, 'C'),
        TerminalInput::ArrowLeft => arrow_seq(state.app_cursor, 'D'),
        TerminalInput::Function(index) => function_key(index),
        TerminalInput::Ctrl(ch) => ctrl_seq(ch),
        TerminalInput::Alt(ch) => alt_seq(ch),
        TerminalInput::CtrlAlt(ch) => {
            let mut bytes = vec![0x1b];
            bytes.extend(ctrl_seq(ch));
            bytes
        }
        TerminalInput::MouseButton {
            button,
            column,
            row,
            kind,
            modifiers,
        } => {
            if state.mouse_reporting {
                sgr_mouse_button(button, column, row, kind, modifiers)
            } else {
                Vec::new()
            }
        }
        TerminalInput::MouseMove {
            button,
            column,
            row,
            modifiers,
        } => {
            if state.mouse_reporting {
                sgr_mouse_move(button, column, row, modifiers)
            } else {
                Vec::new()
            }
        }
        TerminalInput::MouseScroll {
            direction,
            column,
            row,
            modifiers,
        } => {
            if state.mouse_reporting {
                sgr_mouse_scroll(direction, column, row, modifiers)
            } else {
                Vec::new()
            }
        }
    }
}

fn arrow_seq(app_cursor: bool, code: char) -> Vec<u8> {
    if app_cursor {
        format!("\x1bO{code}").into_bytes()
    } else {
        format!("\x1b[{code}").into_bytes()
    }
}

fn function_key(index: u8) -> Vec<u8> {
    match index {
        1 => b"\x1bOP".to_vec(),
        2 => b"\x1bOQ".to_vec(),
        3 => b"\x1bOR".to_vec(),
        4 => b"\x1bOS".to_vec(),
        5 => b"\x1b[15~".to_vec(),
        6 => b"\x1b[17~".to_vec(),
        7 => b"\x1b[18~".to_vec(),
        8 => b"\x1b[19~".to_vec(),
        9 => b"\x1b[20~".to_vec(),
        10 => b"\x1b[21~".to_vec(),
        11 => b"\x1b[23~".to_vec(),
        12 => b"\x1b[24~".to_vec(),
        _ => Vec::new(),
    }
}

fn ctrl_seq(ch: char) -> Vec<u8> {
    match ch {
        '@' | ' ' => vec![0x00],
        '[' => vec![0x1b],
        '\\' => vec![0x1c],
        ']' => vec![0x1d],
        '^' => vec![0x1e],
        '_' => vec![0x1f],
        other => {
            let upper = other.to_ascii_uppercase() as u8;
            if upper.is_ascii_uppercase() {
                vec![upper - b'@']
            } else {
                vec![upper]
            }
        }
    }
}

fn alt_seq(ch: char) -> Vec<u8> {
    let mut bytes = vec![0x1b];
    let mut utf8 = [0u8; 4];
    bytes.extend(ch.encode_utf8(&mut utf8).as_bytes());
    bytes
}

fn sgr_mouse_button(
    button: TerminalMouseButton,
    column: u16,
    row: u16,
    kind: TerminalMouseEventKind,
    modifiers: TerminalMouseModifiers,
) -> Vec<u8> {
    let base = match kind {
        TerminalMouseEventKind::Press => mouse_button_code(button),
        TerminalMouseEventKind::Release => 3,
    } + mouse_modifier_code(modifiers);
    format!(
        "\x1b[<{};{};{}{}",
        base,
        column,
        row,
        match kind {
            TerminalMouseEventKind::Press => 'M',
            TerminalMouseEventKind::Release => 'm',
        }
    )
    .into_bytes()
}

fn sgr_mouse_move(
    button: Option<TerminalMouseButton>,
    column: u16,
    row: u16,
    modifiers: TerminalMouseModifiers,
) -> Vec<u8> {
    let base = button.map(mouse_button_code).unwrap_or(3) + 32 + mouse_modifier_code(modifiers);
    format!("\x1b[<{};{};{}M", base, column, row).into_bytes()
}

fn sgr_mouse_scroll(
    direction: TerminalMouseScrollDirection,
    column: u16,
    row: u16,
    modifiers: TerminalMouseModifiers,
) -> Vec<u8> {
    let base = match direction {
        TerminalMouseScrollDirection::Up => 64,
        TerminalMouseScrollDirection::Down => 65,
    } + mouse_modifier_code(modifiers);
    format!("\x1b[<{};{};{}M", base, column, row).into_bytes()
}

fn mouse_button_code(button: TerminalMouseButton) -> u8 {
    match button {
        TerminalMouseButton::Left => 0,
        TerminalMouseButton::Middle => 1,
        TerminalMouseButton::Right => 2,
    }
}

fn mouse_modifier_code(modifiers: TerminalMouseModifiers) -> u8 {
    (if modifiers.shift { 4 } else { 0 })
        + (if modifiers.alt { 8 } else { 0 })
        + (if modifiers.ctrl { 16 } else { 0 })
}

fn display_text_for_cell(cell: &Cell) -> String {
    if cell
        .flags
        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
    {
        String::from(" ")
    } else if cell.flags.contains(Flags::HIDDEN) {
        String::from(" ")
    } else {
        cell.c.to_string()
    }
}

fn style_for_cell(
    cell: &Cell,
    colors: &alacritty_terminal::term::color::Colors,
) -> TerminalCellStyle {
    TerminalCellStyle {
        fg: color_to_rgb_u32(cell.fg, colors),
        bg: color_to_rgb_u32(cell.bg, colors),
        bold: cell.flags.contains(Flags::BOLD),
        italic: cell.flags.contains(Flags::ITALIC),
        underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        dim: cell.flags.contains(Flags::DIM),
        inverse: cell.flags.contains(Flags::INVERSE),
        hidden: cell.flags.contains(Flags::HIDDEN),
        strike: cell.flags.contains(Flags::STRIKEOUT),
    }
}

fn color_to_rgb_u32(color: Color, colors: &alacritty_terminal::term::color::Colors) -> u32 {
    let rgb = match color {
        Color::Spec(rgb) => rgb,
        Color::Named(named) => colors[named].unwrap_or_else(|| named_color_rgb(named)),
        Color::Indexed(index) => indexed_color_rgb(index),
    };
    ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | (rgb.b as u32)
}

fn named_color_rgb(named: NamedColor) -> Rgb {
    match named {
        NamedColor::Black => rgb(0x1d, 0x1f, 0x21),
        NamedColor::Red => rgb(0xcc, 0x66, 0x66),
        NamedColor::Green => rgb(0xb5, 0xbd, 0x68),
        NamedColor::Yellow => rgb(0xf0, 0xc6, 0x74),
        NamedColor::Blue => rgb(0x81, 0xa2, 0xbe),
        NamedColor::Magenta => rgb(0xb2, 0x94, 0xbb),
        NamedColor::Cyan => rgb(0x8a, 0xbe, 0xb7),
        NamedColor::White => rgb(0xc5, 0xc8, 0xc6),
        NamedColor::BrightBlack => rgb(0x66, 0x66, 0x66),
        NamedColor::BrightRed => rgb(0xff, 0x33, 0x34),
        NamedColor::BrightGreen => rgb(0x9e, 0xe4, 0x93),
        NamedColor::BrightYellow => rgb(0xff, 0xff, 0x66),
        NamedColor::BrightBlue => rgb(0x72, 0x9f, 0xcf),
        NamedColor::BrightMagenta => rgb(0xad, 0x7f, 0xa8),
        NamedColor::BrightCyan => rgb(0x34, 0xe2, 0xe2),
        NamedColor::BrightWhite => rgb(0xee, 0xee, 0xec),
        NamedColor::Foreground => rgb(0xea, 0xea, 0xea),
        NamedColor::Background => rgb(0x11, 0x13, 0x17),
        NamedColor::Cursor => rgb(0xea, 0xea, 0xea),
        NamedColor::DimBlack => rgb(0x13, 0x14, 0x16),
        NamedColor::DimRed => rgb(0x88, 0x44, 0x44),
        NamedColor::DimGreen => rgb(0x76, 0x7d, 0x45),
        NamedColor::DimYellow => rgb(0x9e, 0x84, 0x4f),
        NamedColor::DimBlue => rgb(0x56, 0x6f, 0x84),
        NamedColor::DimMagenta => rgb(0x7b, 0x62, 0x82),
        NamedColor::DimCyan => rgb(0x5f, 0x82, 0x7d),
        NamedColor::DimWhite => rgb(0x8f, 0x93, 0x91),
        NamedColor::BrightForeground => rgb(0xff, 0xff, 0xff),
        NamedColor::DimForeground => rgb(0xaa, 0xaa, 0xaa),
    }
}

fn indexed_color_rgb(index: u8) -> Rgb {
    match index {
        0 => named_color_rgb(NamedColor::Black),
        1 => named_color_rgb(NamedColor::Red),
        2 => named_color_rgb(NamedColor::Green),
        3 => named_color_rgb(NamedColor::Yellow),
        4 => named_color_rgb(NamedColor::Blue),
        5 => named_color_rgb(NamedColor::Magenta),
        6 => named_color_rgb(NamedColor::Cyan),
        7 => named_color_rgb(NamedColor::White),
        8 => named_color_rgb(NamedColor::BrightBlack),
        9 => named_color_rgb(NamedColor::BrightRed),
        10 => named_color_rgb(NamedColor::BrightGreen),
        11 => named_color_rgb(NamedColor::BrightYellow),
        12 => named_color_rgb(NamedColor::BrightBlue),
        13 => named_color_rgb(NamedColor::BrightMagenta),
        14 => named_color_rgb(NamedColor::BrightCyan),
        15 => named_color_rgb(NamedColor::BrightWhite),
        16..=231 => {
            let index = index - 16;
            let r = index / 36;
            let g = (index % 36) / 6;
            let b = index % 6;
            rgb(
                color_cube_value(r),
                color_cube_value(g),
                color_cube_value(b),
            )
        }
        232..=255 => {
            let level = 8 + (index - 232) * 10;
            rgb(level, level, level)
        }
    }
}

fn color_cube_value(index: u8) -> u8 {
    match index {
        0 => 0,
        _ => 55 + index * 40,
    }
}

fn map_cursor_shape(shape: CursorShape) -> TerminalCursorShape {
    match shape {
        CursorShape::Block => TerminalCursorShape::Block,
        CursorShape::Underline => TerminalCursorShape::Underline,
        CursorShape::Beam => TerminalCursorShape::Beam,
        CursorShape::HollowBlock => TerminalCursorShape::HollowBlock,
        CursorShape::Hidden => TerminalCursorShape::Hidden,
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

#[cfg(test)]
mod tests {
    use super::{
        TerminalEngine, TerminalInput, TerminalInputState, TerminalMode,
        TerminalMode as ScreenMode, TerminalMouseButton, TerminalMouseEventKind,
        TerminalMouseModifiers, TerminalMouseScrollDirection, encode_input,
    };

    #[test]
    fn parses_ansi_colors_into_styled_rows() {
        let mut engine = TerminalEngine::with_size("ansi", 12, 4);
        let frame = engine.feed_bytes(b"\x1b[31mred\x1b[0m");
        assert_eq!(frame.rows[0], "red");
        assert_eq!(frame.styled_rows[0].cells[0].style.fg, 0xcc6666);
    }

    #[test]
    fn clear_screen_and_cursor_move_work() {
        let mut engine = TerminalEngine::with_size("clear", 12, 4);
        engine.feed_bytes(b"hello");
        let frame = engine.feed_bytes(b"\x1b[2J\x1b[Hok");
        assert_eq!(frame.rows[0], "ok");
        assert!(frame.rows.iter().skip(1).all(|row| row.is_empty()));
    }

    #[test]
    fn alternate_screen_and_resize_are_tracked() {
        let mut engine = TerminalEngine::with_size("alt", 8, 3);
        engine.feed_bytes(b"main");
        let alt_frame = engine.feed_bytes(b"\x1b[?1049h\x1b[HALT");
        assert_eq!(alt_frame.mode, TerminalMode::Alternate);
        assert_eq!(alt_frame.rows[0], "ALT");

        let resized = engine.resize(5, 2);
        assert_eq!(resized.columns, 5);
        assert_eq!(resized.screen_lines, 2);

        let restored = engine.feed_bytes(b"\x1b[?1049l");
        assert_eq!(restored.mode, ScreenMode::Primary);
        assert_eq!(restored.rows[0], "main");
    }

    #[test]
    fn input_mapping_covers_common_terminal_sequences() {
        let state = TerminalInputState {
            app_cursor: true,
            bracketed_paste: true,
            mouse_reporting: true,
        };

        assert_eq!(encode_input(state, TerminalInput::ArrowUp), b"\x1bOA");
        assert_eq!(encode_input(state, TerminalInput::Home), b"\x1bOH");
        assert_eq!(encode_input(state, TerminalInput::Function(5)), b"\x1b[15~");
        assert_eq!(encode_input(state, TerminalInput::Ctrl('c')), vec![3]);
        assert_eq!(encode_input(state, TerminalInput::Alt('x')), b"\x1bx");
        assert_eq!(
            encode_input(state, TerminalInput::Paste(String::from("abc"))),
            b"\x1b[200~abc\x1b[201~"
        );
        assert_eq!(
            encode_input(
                state,
                TerminalInput::MouseButton {
                    button: TerminalMouseButton::Left,
                    column: 12,
                    row: 4,
                    kind: TerminalMouseEventKind::Press,
                    modifiers: TerminalMouseModifiers::default(),
                }
            ),
            b"\x1b[<0;12;4M"
        );
        assert_eq!(
            encode_input(
                state,
                TerminalInput::MouseScroll {
                    direction: TerminalMouseScrollDirection::Down,
                    column: 9,
                    row: 3,
                    modifiers: TerminalMouseModifiers {
                        shift: false,
                        alt: false,
                        ctrl: true,
                    },
                }
            ),
            b"\x1b[<81;9;3M"
        );
    }
}
