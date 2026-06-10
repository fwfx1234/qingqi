//! 终端引擎（PTY + 日志双模式）

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::model::TerminalKind;
use crate::protocol::{LogLevel, TerminalOutput};

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

#[derive(Clone, Debug)]
pub enum TerminalInput {
    Key(String),
    Paste(String),
    Resize { cols: u16, rows: u16 },
}

pub struct TerminalEngine {
    kind: TerminalKind,
    lines: Mutex<Vec<TerminalLine>>,
    cursor_row: Mutex<usize>,
    cursor_col: Mutex<usize>,
    cursor_visible: Mutex<bool>,
    status_text: Mutex<String>,
    max_lines: usize,
}

impl TerminalEngine {
    pub fn new(kind: TerminalKind) -> Self {
        Self {
            kind,
            lines: Mutex::new(Vec::new()),
            cursor_row: Mutex::new(0),
            cursor_col: Mutex::new(0),
            cursor_visible: Mutex::new(true),
            status_text: Mutex::new(String::new()),
            max_lines: 5000,
        }
    }

    pub fn start_processing(
        engine: Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<TerminalOutput>,
    ) {
        tokio::spawn(async move {
            while let Some(output) = rx.recv().await {
                match output {
                    TerminalOutput::PtyOutput(data) => {
                        let text = String::from_utf8_lossy(&data);
                        let mut lines =
                            engine.lines.lock().unwrap_or_else(|e| e.into_inner());
                        for line in text.lines() {
                            lines.push(TerminalLine {
                                text: line.to_string(),
                                fg_color: None,
                                bg_color: None,
                                bold: false,
                            });
                        }
                        while lines.len() > engine.max_lines {
                            lines.remove(0);
                        }
                        if let Some(_last) = lines.last() {
                            let mut row = engine
                                .cursor_row
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            *row = lines.len().saturating_sub(1);
                        }
                    }
                    TerminalOutput::LogLine { level, text } => {
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
                        let mut lines =
                            engine.lines.lock().unwrap_or_else(|e| e.into_inner());
                        lines.push(TerminalLine {
                            text: format!("{prefix}{text}"),
                            fg_color: color,
                            bg_color: None,
                            bold: false,
                        });
                        while lines.len() > engine.max_lines {
                            lines.remove(0);
                        }
                    }
                }
            }
        });
    }

    pub fn append_log(&self, level: LogLevel, text: &str) {
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
        let mut lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
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

    pub fn snapshot(&self) -> TerminalFrame {
        let lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        TerminalFrame {
            lines: lines.clone(),
            cursor_visible: *self
                .cursor_visible
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
            cursor_row: *self
                .cursor_row
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
            cursor_col: *self
                .cursor_col
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
            status_text: self
                .status_text
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            terminal_kind: self.kind.clone(),
        }
    }

    pub fn set_status(&self, text: &str) {
        let mut s = self
            .status_text
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *s = text.to_string();
    }
}
