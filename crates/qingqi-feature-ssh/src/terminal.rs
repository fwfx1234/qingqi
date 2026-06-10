//! 终端引擎（SSH: alacritty_terminal / FTP: 日志模式）

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::viewport_to_point;
use alacritty_terminal::term::cell::{Flags, LineLength};
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use tokio::sync::mpsc;

use crate::model::TerminalKind;
use crate::protocol::{LogLevel, TerminalOutput};

const TERM_COLS: usize = 120;
const TERM_ROWS: usize = 40;

#[derive(Clone, Debug)]
pub struct TerminalFrame {
    pub lines: Vec<TerminalLine>,
    pub cursor_visible: bool,
    pub cursor_row: usize,
    pub cursor_col: usize,
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
    fn new() -> Self {
        let size = TermSize::new(TERM_COLS, TERM_ROWS);
        Self {
            term: Term::new(Config::default(), &size, VoidListener),
            parser: Processor::new(),
        }
    }

    fn write(&mut self, data: &[u8]) {
        self.parser.advance(&mut self.term, data);
    }

    fn extract_grid_line(&self, line: Line) -> String {
        let cols = self.term.columns();
        let grid_row = &self.term.grid()[line];
        let line_length = grid_row.line_length();
        let mut text = String::with_capacity(cols);

        for col in 0..cols {
            let column = Column(col);
            let cell = &grid_row[column];

            if cell.c == '\t' {
                while text.chars().count() % 8 != 0 {
                    text.push(' ');
                }
                continue;
            }

            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            if cell.flags.contains(Flags::HIDDEN) {
                continue;
            }

            text.push(cell.c);
        }

        // 截断行尾空白，保留有内容的尾部空格
        let end = line_length.0.min(cols);
        let trimmed: String = text
            .chars()
            .take(end)
            .collect::<String>()
            .trim_end()
            .to_string();
        sanitize_terminal_line(&trimmed)
    }

    /// 仅导出当前可见视口（避免 scrollback 残留脏行）
    fn snapshot(&self) -> (Vec<TerminalLine>, usize, usize, bool) {
        let offset = self.term.grid().display_offset();
        let screen_lines = self.term.screen_lines();
        let cursor = &self.term.grid().cursor;
        let mut lines = Vec::with_capacity(screen_lines);
        let mut cursor_row = 0usize;

        for viewport_row in 0..screen_lines {
            let grid_line = viewport_to_point(offset, Point::new(viewport_row, Column(0))).line;
            let text = self.extract_grid_line(grid_line);
            if grid_line == cursor.point.line {
                cursor_row = viewport_row;
            }
            lines.push(TerminalLine {
                text,
                fg_color: None,
                bg_color: None,
                bold: false,
            });
        }

        let cursor_visible = self
            .term
            .mode()
            .contains(alacritty_terminal::term::TermMode::SHOW_CURSOR);

        let cursor_col = cursor.point.column.0;
        (lines, cursor_row, cursor_col, cursor_visible)
    }
}

fn sanitize_terminal_line(text: &str) -> String {
    text.chars()
        .filter(|c| *c == '\t' || !c.is_control())
        .collect()
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
        let shell = matches!(kind, TerminalKind::Shell).then(ShellTerm::new);
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

    pub fn start_processing(engine: Arc<Self>, rx: mpsc::UnboundedReceiver<TerminalOutput>) {
        Self::start_processing_with_notify(engine, rx, None);
    }

    pub fn start_processing_with_notify(
        engine: Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<TerminalOutput>,
        on_update: Option<Box<dyn Fn() + Send + Sync>>,
    ) {
        tokio::spawn(async move {
            let mut last_notify = std::time::Instant::now() - std::time::Duration::from_millis(50);
            while let Some(output) = rx.recv().await {
                match output {
                    TerminalOutput::PtyOutput(data) => {
                        if let Ok(mut shell) = engine.shell.lock() {
                            if let Some(term) = shell.as_mut() {
                                term.write(&data);
                                let (_, row, col, visible) = term.snapshot();
                                if let Ok(mut cursor_row) = engine.cursor_row.lock() {
                                    *cursor_row = row;
                                }
                                if let Ok(mut cursor_col) = engine.cursor_col.lock() {
                                    *cursor_col = col;
                                }
                                if let Ok(mut cursor_visible) = engine.cursor_visible.lock() {
                                    *cursor_visible = visible;
                                }
                            }
                        }
                    }
                    TerminalOutput::LogLine { level, text } => {
                        engine.push_log_line(level, &text);
                    }
                }
                if let Some(notify) = on_update.as_ref() {
                    let now = std::time::Instant::now();
                    if now.duration_since(last_notify) >= std::time::Duration::from_millis(50) {
                        notify();
                        last_notify = now;
                    }
                }
            }
        });
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
                let (lines, row, col, visible) = term.snapshot();
                return TerminalFrame {
                    lines,
                    cursor_visible: visible,
                    cursor_row: row,
                    cursor_col: col,
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
            cursor_visible: *self
                .cursor_visible
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
            cursor_row: *self.cursor_row.lock().unwrap_or_else(|e| e.into_inner()),
            cursor_col: *self.cursor_col.lock().unwrap_or_else(|e| e.into_inner()),
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
}
