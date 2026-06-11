//! 终端引擎（SSH: alacritty_terminal / FTP: 日志模式）

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::viewport_to_point;
use alacritty_terminal::term::cell::{Cell, Flags, LineLength};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};
use tracing::debug;

use crate::model::TerminalKind;
use crate::protocol::{LogLevel, PtyOutputHub, TerminalOutput, TerminalOutputSource};

pub const DEFAULT_TERM_COLS: usize = 80;
pub const DEFAULT_TERM_ROWS: usize = 24;

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalCell {
    pub ch: char,
    pub fg: Option<[f32; 4]>,
    pub bg: Option<[f32; 4]>,
    pub bold: bool,
    pub inverse: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalModes {
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    pub mouse_click: bool,
    pub mouse_drag: bool,
    pub mouse_motion: bool,
    pub sgr_mouse: bool,
}

impl TerminalModes {
    pub fn mouse_active(&self) -> bool {
        self.mouse_click || self.mouse_drag || self.mouse_motion
    }
}

#[derive(Clone, Debug)]
pub struct TerminalFrame {
    pub lines: Vec<TerminalLine>,
    pub grid: Vec<Vec<TerminalCell>>,
    pub cols: usize,
    pub rows: usize,
    pub display_offset: usize,
    pub max_display_offset: usize,
    pub cursor_visible: bool,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub modes: TerminalModes,
    pub status_text: String,
    pub terminal_kind: TerminalKind,
}

#[derive(Clone, Debug)]
pub struct TerminalLine {
    pub text: String,
    pub fg_color: Option<[f32; 4]>,
    pub bg_color: Option<[f32; 4]>,
    pub bold: bool,
}

struct ShellTerm {
    term: Term<VoidListener>,
    parser: Processor,
}

impl ShellTerm {
    fn new(cols: usize, rows: usize) -> Self {
        let mut config = Config::default();
        config.scrolling_history = 10_000;
        let size = TermSize::new(cols, rows);
        Self {
            term: Term::new(config, &size, VoidListener),
            parser: Processor::new(),
        }
    }

    fn write(&mut self, data: &[u8]) {
        self.parser.advance(&mut self.term, data);
    }

    fn resize(&mut self, cols: usize, rows: usize) {
        if cols == 0 || rows == 0 {
            return;
        }
        let size = TermSize::new(cols, rows);
        self.term.resize(size);
    }

    fn scroll_display(&mut self, scroll: Scroll) {
        self.term.scroll_display(scroll);
    }

    fn cols(&self) -> usize {
        self.term.columns()
    }

    fn rows(&self) -> usize {
        self.term.screen_lines()
    }

    fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    fn max_display_offset(&self) -> usize {
        self.term.grid().history_size()
    }

    fn cell_to_terminal_cell(&self, cell: &Cell) -> TerminalCell {
        let inverse = cell.flags.contains(Flags::INVERSE);
        let (mut fg, mut bg) = (cell.fg, cell.bg);
        if inverse {
            std::mem::swap(&mut fg, &mut bg);
        }
        TerminalCell {
            ch: cell.c,
            fg: resolve_color(fg, self.term.colors()),
            bg: resolve_color(bg, self.term.colors()),
            bold: cell.flags.intersects(Flags::BOLD | Flags::BOLD_ITALIC),
            inverse,
        }
    }

    fn extract_grid_row(&self, line: Line) -> Vec<TerminalCell> {
        let cols = self.term.columns();
        let grid_row = &self.term.grid()[line];
        let line_length = grid_row.line_length();
        let end = line_length.0.min(cols);
        let mut cells = Vec::with_capacity(end.max(1));

        for col in 0..end {
            let column = Column(col);
            let cell = &grid_row[column];

            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }

            let mut term_cell = if cell.flags.contains(Flags::HIDDEN) {
                empty_terminal_cell()
            } else {
                self.cell_to_terminal_cell(cell)
            };

            if term_cell.ch == '\t' {
                term_cell.ch = ' ';
            } else if term_cell.ch.is_control() {
                term_cell.ch = ' ';
            }
            cells.push(term_cell);
        }

        if cells.is_empty() {
            cells.push(empty_terminal_cell());
        }
        cells
    }

    fn terminal_modes(&self) -> TerminalModes {
        let mode = self.term.mode();
        TerminalModes {
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            mouse_click: mode.contains(TermMode::MOUSE_REPORT_CLICK),
            mouse_drag: mode.contains(TermMode::MOUSE_DRAG),
            mouse_motion: mode.contains(TermMode::MOUSE_MOTION),
            sgr_mouse: mode.contains(TermMode::SGR_MOUSE),
        }
    }

    fn snapshot(&self) -> (Vec<Vec<TerminalCell>>, usize, usize, bool, TerminalModes) {
        let offset = self.term.grid().display_offset();
        let screen_lines = self.term.screen_lines();
        let cursor = &self.term.grid().cursor;
        let mut grid = Vec::with_capacity(screen_lines);
        let mut cursor_row = 0usize;
        let mut cursor_in_view = false;

        for viewport_row in 0..screen_lines {
            let grid_line = viewport_to_point(offset, Point::new(viewport_row, Column(0))).line;
            let row_cells = self.extract_grid_row(grid_line);
            if grid_line == cursor.point.line {
                cursor_row = viewport_row;
                cursor_in_view = true;
            }
            grid.push(row_cells);
        }

        let cursor_visible = self
            .term
            .mode()
            .contains(TermMode::SHOW_CURSOR)
            && cursor_in_view;

        let cursor_col = cursor.point.column.0;
        (
            grid,
            cursor_row,
            cursor_col,
            cursor_visible,
            self.terminal_modes(),
        )
    }

    fn cursor_viewport(&self) -> (usize, usize, bool) {
        let offset = self.term.grid().display_offset();
        let cursor = &self.term.grid().cursor;
        let mut cursor_row = 0usize;
        let mut cursor_in_view = false;
        for viewport_row in 0..self.term.screen_lines() {
            let grid_line = viewport_to_point(offset, Point::new(viewport_row, Column(0))).line;
            if grid_line == cursor.point.line {
                cursor_row = viewport_row;
                cursor_in_view = true;
                break;
            }
        }
        let cursor_visible = self
            .term
            .mode()
            .contains(alacritty_terminal::term::TermMode::SHOW_CURSOR)
            && cursor_in_view;
        (cursor_row, cursor.point.column.0, cursor_visible)
    }
}

fn empty_terminal_cell() -> TerminalCell {
    TerminalCell {
        ch: ' ',
        fg: None,
        bg: None,
        bold: false,
        inverse: false,
    }
}

fn resolve_color(color: Color, palette: &Colors) -> Option<[f32; 4]> {
    let rgb = match color {
        Color::Spec(rgb) => rgb,
        Color::Named(named) => palette[named].or_else(|| default_named_rgb(named))?,
        Color::Indexed(index) => palette[index as usize].or_else(|| indexed_rgb(index))?,
    };
    Some(rgb_to_hsla(rgb))
}

fn rgb_to_hsla(rgb: Rgb) -> [f32; 4] {
    let r = rgb.r as f32 / 255.0;
    let g = rgb.g as f32 / 255.0;
    let b = rgb.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return [0.0, 0.0, l, 1.0];
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        (g - b) / d + (if g < b { 6.0 } else { 0.0 })
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    [h, s, l, 1.0]
}

fn default_named_rgb(named: NamedColor) -> Option<Rgb> {
    Some(match named {
        NamedColor::Black => Rgb { r: 0, g: 0, b: 0 },
        NamedColor::Red => Rgb { r: 0xcd, g: 0x00, b: 0x00 },
        NamedColor::Green => Rgb { r: 0x00, g: 0xcd, b: 0x00 },
        NamedColor::Yellow => Rgb { r: 0xcd, g: 0xcd, b: 0x00 },
        NamedColor::Blue => Rgb { r: 0x00, g: 0x00, b: 0xcd },
        NamedColor::Magenta => Rgb { r: 0xcd, g: 0x00, b: 0xcd },
        NamedColor::Cyan => Rgb { r: 0x00, g: 0xcd, b: 0xcd },
        NamedColor::White => Rgb { r: 0xe5, g: 0xe5, b: 0xe5 },
        NamedColor::BrightBlack => Rgb { r: 0x4d, g: 0x4d, b: 0x4d },
        NamedColor::BrightRed => Rgb { r: 0xff, g: 0x00, b: 0x00 },
        NamedColor::BrightGreen => Rgb { r: 0x00, g: 0xff, b: 0x00 },
        NamedColor::BrightYellow => Rgb { r: 0xff, g: 0xff, b: 0x00 },
        NamedColor::BrightBlue => Rgb { r: 0x46, g: 0x6d, b: 0xff },
        NamedColor::BrightMagenta => Rgb { r: 0xff, g: 0x00, b: 0xff },
        NamedColor::BrightCyan => Rgb { r: 0x00, g: 0xff, b: 0xff },
        NamedColor::BrightWhite => Rgb { r: 0xff, g: 0xff, b: 0xff },
        NamedColor::Foreground => Rgb { r: 0x1e, g: 0x1e, b: 0x1e },
        NamedColor::Background => Rgb { r: 0xfc, g: 0xfc, b: 0xfc },
        NamedColor::Cursor => Rgb { r: 0x1e, g: 0x1e, b: 0x1e },
        _ => return None,
    })
}

fn indexed_rgb(index: u8) -> Option<Rgb> {
    if index < 16 {
        return default_named_rgb(match index {
            0 => NamedColor::Black,
            1 => NamedColor::Red,
            2 => NamedColor::Green,
            3 => NamedColor::Yellow,
            4 => NamedColor::Blue,
            5 => NamedColor::Magenta,
            6 => NamedColor::Cyan,
            7 => NamedColor::White,
            8 => NamedColor::BrightBlack,
            9 => NamedColor::BrightRed,
            10 => NamedColor::BrightGreen,
            11 => NamedColor::BrightYellow,
            12 => NamedColor::BrightBlue,
            13 => NamedColor::BrightMagenta,
            14 => NamedColor::BrightCyan,
            15 => NamedColor::BrightWhite,
            _ => return None,
        });
    }
    if index < 232 {
        let index = index - 16;
        let r = index / 36;
        let g = (index % 36) / 6;
        let b = index % 6;
        let ramp = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
        return Some(Rgb {
            r: ramp[r as usize],
            g: ramp[g as usize],
            b: ramp[b as usize],
        });
    }
    let level = index - 232;
    let gray = 0x08 + level * 10;
    Some(Rgb {
        r: gray,
        g: gray,
        b: gray,
    })
}

pub struct TerminalEngine {
    kind: TerminalKind,
    shell: Mutex<Option<ShellTerm>>,
    log_lines: Mutex<Vec<TerminalLine>>,
    cursor_row: Mutex<usize>,
    cursor_col: Mutex<usize>,
    cursor_visible: Mutex<bool>,
    status_text: Mutex<String>,
    max_lines: usize,
}

impl TerminalEngine {
    pub fn new(kind: TerminalKind) -> Self {
        let shell = matches!(kind, TerminalKind::Shell)
            .then(|| ShellTerm::new(DEFAULT_TERM_COLS, DEFAULT_TERM_ROWS));
        Self {
            kind,
            shell: Mutex::new(shell),
            log_lines: Mutex::new(Vec::new()),
            cursor_row: Mutex::new(0),
            cursor_col: Mutex::new(0),
            cursor_visible: Mutex::new(true),
            status_text: Mutex::new(String::new()),
            max_lines: 5000,
        }
    }

    fn sync_cursor_from_shell(&self, term: &ShellTerm) {
        let (row, col, visible) = term.cursor_viewport();
        if let Ok(mut cursor_row) = self.cursor_row.lock() {
            *cursor_row = row;
        }
        if let Ok(mut cursor_col) = self.cursor_col.lock() {
            *cursor_col = col;
        }
        if let Ok(mut cursor_visible) = self.cursor_visible.lock() {
            *cursor_visible = visible;
        }
    }

    pub fn start_processing(engine: Arc<Self>, source: TerminalOutputSource) {
        Self::start_processing_with_notify(engine, source, None);
    }

    pub fn start_processing_with_notify(
        engine: Arc<Self>,
        source: TerminalOutputSource,
        on_update: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        match source {
            TerminalOutputSource::Channel(rx) => {
                Self::spawn_channel_loop(engine, rx, on_update);
            }
            TerminalOutputSource::PtyHub(hub) => {
                Self::spawn_pty_hub_loop(engine, hub, on_update);
            }
        }
    }

    fn spawn_channel_loop(
        engine: Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<TerminalOutput>,
        on_update: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        tokio::spawn(async move {
            let mut pending_flush: Option<tokio::task::JoinHandle<()>> = None;
            let mut debounce_aborts: u32 = 0;
            let mut pty_packets: u64 = 0;
            const DEBOUNCE: Duration = Duration::from_millis(16);

            while let Some(output) = rx.recv().await {
                match output {
                    TerminalOutput::PtyOutput(data) => {
                        pty_packets += 1;
                        Self::apply_pty_output(
                            &engine,
                            &data,
                            pty_packets,
                            &mut pending_flush,
                            &mut debounce_aborts,
                            on_update.as_ref(),
                            DEBOUNCE,
                        );
                    }
                    TerminalOutput::LogLine { level, text } => {
                        Self::apply_log_line(
                            &engine,
                            level,
                            &text,
                            &mut pending_flush,
                            &mut debounce_aborts,
                            on_update.as_ref(),
                        );
                    }
                }
            }

            debug!(
                target: "qingqi_ssh",
                pty_packets,
                debounce_aborts,
                "term_diag: PtyOutput 接收循环结束"
            );
            Self::flush_pending_notify(pending_flush, on_update.as_ref()).await;
        });
    }

    fn spawn_pty_hub_loop(
        engine: Arc<Self>,
        hub: Arc<PtyOutputHub>,
        on_update: Option<Arc<dyn Fn() + Send + Sync>>,
    ) {
        tokio::spawn(async move {
            let mut pending_flush: Option<tokio::task::JoinHandle<()>> = None;
            let mut debounce_aborts: u32 = 0;
            let mut pty_packets: u64 = 0;
            const DEBOUNCE: Duration = Duration::from_millis(16);
            let notify = hub.notify();

            loop {
                let chunks = hub.drain();
                if chunks.is_empty() {
                    notify.notified().await;
                    continue;
                }
                for data in chunks {
                    pty_packets += 1;
                    Self::apply_pty_output(
                        &engine,
                        &data,
                        pty_packets,
                        &mut pending_flush,
                        &mut debounce_aborts,
                        on_update.as_ref(),
                        DEBOUNCE,
                    );
                }
            }
        });
    }

    fn apply_pty_output(
        engine: &Arc<Self>,
        data: &[u8],
        pty_packets: u64,
        pending_flush: &mut Option<tokio::task::JoinHandle<()>>,
        debounce_aborts: &mut u32,
        on_update: Option<&Arc<dyn Fn() + Send + Sync>>,
        debounce: Duration,
    ) {
        let offset_before = engine
            .shell
            .lock()
            .ok()
            .and_then(|shell| shell.as_ref().map(|term| term.display_offset()))
            .unwrap_or(0);
        let (rows, cols, cursor_row, cursor_col, offset_after) =
            if let Ok(mut shell) = engine.shell.lock() {
                if let Some(term) = shell.as_mut() {
                    term.write(data);
                    engine.sync_cursor_from_shell(term);
                    let (row, col, _) = term.cursor_viewport();
                    (
                        term.rows(),
                        term.cols(),
                        row,
                        col,
                        term.display_offset(),
                    )
                } else {
                    debug!(
                        target: "qingqi_ssh",
                        bytes = data.len(),
                        "term_diag: PtyOutput 但 shell 未初始化"
                    );
                    (0, 0, 0, 0, offset_before)
                }
            } else {
                (0, 0, 0, 0, offset_before)
            };
        debug!(
            target: "qingqi_ssh",
            packet = pty_packets,
            bytes = data.len(),
            offset_before,
            offset_after,
            rows,
            cols,
            cursor_row,
            cursor_col,
            debounce_aborts = *debounce_aborts,
            "term_diag: PtyOutput 已写入 alacritty"
        );
        if let Some(callback) = on_update {
            if pending_flush.take().is_some() {
                *debounce_aborts += 1;
            }
            let callback = Arc::clone(callback);
            *pending_flush = Some(tokio::spawn(async move {
                sleep(debounce).await;
                debug!(target: "qingqi_ssh", "term_diag: debounce 触发 UI notify");
                callback();
            }));
        }
    }

    fn apply_log_line(
        engine: &Arc<Self>,
        level: LogLevel,
        text: &str,
        pending_flush: &mut Option<tokio::task::JoinHandle<()>>,
        debounce_aborts: &mut u32,
        on_update: Option<&Arc<dyn Fn() + Send + Sync>>,
    ) {
        engine.push_log_line(level, text);
        if let Some(callback) = on_update {
            if pending_flush.take().is_some() {
                *debounce_aborts += 1;
            }
            debug!(target: "qingqi_ssh", "term_diag: LogLine 立即 notify");
            callback();
        }
    }

    async fn flush_pending_notify(
        pending_flush: Option<tokio::task::JoinHandle<()>>,
        on_update: Option<&Arc<dyn Fn() + Send + Sync>>,
    ) {
        if let Some(handle) = pending_flush {
            let _ = handle.await;
        } else if let Some(callback) = on_update {
            callback();
        }
    }

    pub fn scroll_display(&self, scroll: Scroll) {
        if let Ok(mut shell) = self.shell.lock() {
            if let Some(term) = shell.as_mut() {
                term.scroll_display(scroll);
            }
        }
    }

    pub fn scroll_to_bottom(&self) {
        if let Ok(mut shell) = self.shell.lock() {
            if let Some(term) = shell.as_mut() {
                term.scroll_display(Scroll::Bottom);
            }
        }
    }

    pub fn resize(&self, cols: usize, rows: usize) {
        if let Ok(mut shell) = self.shell.lock() {
            if let Some(term) = shell.as_mut() {
                term.resize(cols, rows);
            }
        }
    }

    fn push_log_line(&self, level: LogLevel, text: &str) {
        let color = match level {
            LogLevel::Sent => Some([0.0, 0.8, 1.0, 1.0]),
            LogLevel::Received => Some([0.55, 0.55, 0.55, 1.0]),
            LogLevel::Error => Some([1.0, 0.3, 0.3, 1.0]),
            LogLevel::Info => None,
        };
        let prefix = match level {
            LogLevel::Sent => "> ",
            LogLevel::Received => "< ",
            LogLevel::Error => "! ",
            LogLevel::Info => "  ",
        };
        let mut lines = self.log_lines.lock().unwrap_or_else(|e| e.into_inner());
        lines.push(TerminalLine {
            text: format!("{prefix}{text}"),
            fg_color: color,
            bg_color: None,
            bold: false,
        });
        while lines.len() > self.max_lines {
            lines.remove(0);
        }
    }

    pub fn append_log(&self, level: LogLevel, text: &str) {
        self.push_log_line(level, text);
    }

    pub fn snapshot(&self) -> TerminalFrame {
        if let Ok(shell) = self.shell.lock() {
            if let Some(term) = shell.as_ref() {
                let (grid, row, col, visible, modes) = term.snapshot();
                return TerminalFrame {
                    grid,
                    lines: Vec::new(),
                    cols: term.cols(),
                    rows: term.rows(),
                    display_offset: term.display_offset(),
                    max_display_offset: term.max_display_offset(),
                    cursor_visible: visible,
                    cursor_row: row,
                    cursor_col: col,
                    modes,
                    status_text: self
                        .status_text
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone(),
                    terminal_kind: self.kind.clone(),
                };
            }
        }

        let lines = self.log_lines.lock().unwrap_or_else(|e| e.into_inner());
        TerminalFrame {
            lines: lines.clone(),
            grid: Vec::new(),
            cols: 0,
            rows: 0,
            display_offset: 0,
            max_display_offset: 0,
            cursor_visible: *self
                .cursor_visible
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
            cursor_row: *self.cursor_row.lock().unwrap_or_else(|e| e.into_inner()),
            cursor_col: *self.cursor_col.lock().unwrap_or_else(|e| e.into_inner()),
            modes: TerminalModes::default(),
            status_text: self
                .status_text
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            terminal_kind: self.kind.clone(),
        }
    }

    pub fn set_status(&self, text: &str) {
        let mut s = self.status_text.lock().unwrap_or_else(|e| e.into_inner());
        *s = text.to_string();
    }

    pub fn kind(&self) -> TerminalKind {
        self.kind.clone()
    }
}
