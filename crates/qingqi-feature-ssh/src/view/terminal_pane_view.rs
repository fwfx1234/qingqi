//! 终端面板子 Entity：隔离 notify，持有交互状态与行缓存。

use std::sync::Arc;

use gpui::*;
use tracing::debug;
use gpui::prelude::FluentBuilder;
use gpui_component::scroll::ScrollableElement;
use qingqi_ui::{theme, ui};

use super::terminal_element::{
    cell_at, copy_selection_text, render_shell_grid, TerminalRowCache, TerminalSelection,
};
use super::terminal_input::{
    encode_sgr_mouse, encode_sgr_wheel, keystroke_to_bytes, mouse_button_code, paste_text_to_bytes,
    wrap_bracketed_paste,
};
use super::terminal_pane::{
    render_log_body, render_status_bar, scroll_delta_from_wheel,
    CHAR_WIDTH_RATIO, LINE_HEIGHT_RATIO, TERM_PADDING,
};
use super::terminal_shell_element::TerminalShellElement;
use super::{SshTerminalShiftTab, SshTerminalTab, SshView, TerminalViewModel};
use crate::model::{SessionId, TerminalKind};
use crate::service::SshService;

struct ImeState {
    marked_text: String,
}

pub struct TerminalPaneView {
    ssh: Entity<SshView>,
    service: Arc<SshService>,
    session_id: Option<SessionId>,
    vm: TerminalViewModel,
    font_size: f32,
    transfer_panel_expanded: bool,
    focus_handle: FocusHandle,
    selection: Option<TerminalSelection>,
    selecting: bool,
    content_bounds: Option<Bounds<Pixels>>,
    measured_grid_size: Option<Size<Pixels>>,
    last_grid: Option<(usize, usize)>,
    row_cache: TerminalRowCache,
    last_mouse_cell: Option<super::terminal_element::GridPoint>,
    ime_state: Option<ImeState>,
}

impl TerminalPaneView {
    pub fn new(
        ssh: Entity<SshView>,
        service: Arc<SshService>,
        font_size: f32,
        focus_handle: FocusHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let _ = cx;
        Self {
            ssh,
            service,
            session_id: None,
            vm: TerminalViewModel {
                status: "未连接".into(),
                lines: Vec::new(),
                grid: Vec::new(),
                cols: 0,
                rows: 0,
                display_offset: 0,
                max_display_offset: 0,
                cursor_visible: false,
                cursor_row: 0,
                cursor_col: 0,
                modes: crate::terminal::TerminalModes::default(),
                terminal_kind: TerminalKind::Shell,
            },
            font_size,
            transfer_panel_expanded: false,
            focus_handle,
            selection: None,
            selecting: false,
            content_bounds: None,
            measured_grid_size: None,
            last_grid: None,
            row_cache: TerminalRowCache::default(),
            last_mouse_cell: None,
            ime_state: None,
        }
    }

    pub fn sync_from_parent(
        &mut self,
        session_id: Option<SessionId>,
        vm: TerminalViewModel,
        font_size: f32,
        transfer_panel_expanded: bool,
    ) {
        if self.session_id != session_id {
            self.ime_state = None;
            self.measured_grid_size = None;
            self.last_grid = None;
        }
        self.session_id = session_id;
        self.vm = vm;
        self.font_size = font_size;
        self.transfer_panel_expanded = transfer_panel_expanded;
    }

    pub fn refresh_terminal(&mut self, session_id: Option<&SessionId>) {
        self.vm = SshView::build_terminal(session_id, &self.service);
        if let Some(id) = session_id {
            self.session_id = Some(id.clone());
        }
    }

    pub fn input_enabled_for_ime(&self, cx: &App) -> bool {
        self.input_enabled(cx)
    }

    pub fn terminal_kind(&self) -> TerminalKind {
        self.vm.terminal_kind.clone()
    }

    pub fn char_metrics_for_ime(&self) -> (Pixels, Pixels) {
        self.char_metrics()
    }

    pub fn cursor_viewport(&self) -> (usize, usize) {
        (self.vm.cursor_row, self.vm.cursor_col)
    }

    pub fn set_grid_layout(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        let bounds_changed = self.content_bounds != Some(bounds)
            || self.measured_grid_size != Some(bounds.size);
        self.content_bounds = Some(bounds);
        self.measured_grid_size = Some(bounds.size);
        if bounds_changed || self.needs_grid_resize() {
            self.apply_grid_resize(cx);
        }
    }

    fn needs_grid_resize(&self) -> bool {
        let Some(size) = self.measured_grid_size else {
            return false;
        };
        if !matches!(self.vm.terminal_kind, TerminalKind::Shell) {
            return false;
        }
        let (cols, rows, _, _) =
            super::terminal_element::estimate_grid_size(size, self.font_size, 0.0);
        self.vm.rows != rows || self.vm.cols != cols
    }

    fn apply_grid_resize(&mut self, cx: &mut Context<Self>) {
        let Some(size) = self.measured_grid_size else {
            return;
        };
        let Some(sid) = self.session_id.clone() else {
            return;
        };
        if !matches!(self.vm.terminal_kind, TerminalKind::Shell) {
            return;
        }
        let (cols, rows, _, _) =
            super::terminal_element::estimate_grid_size(size, self.font_size, 0.0);
        if self.last_grid == Some((cols, rows))
            && self.vm.rows == rows
            && self.vm.cols == cols
        {
            return;
        }
        if self.service.resize_terminal(&sid, cols, rows).is_ok() {
            self.last_grid = Some((cols, rows));
            debug!(
                target: "qingqi_ssh",
                cols,
                rows,
                width = f32::from(size.width),
                height = f32::from(size.height),
                "term_diag: PTY resize"
            );
            self.refresh_terminal(Some(&sid));
            cx.notify();
        }
    }

    pub fn marked_text(&self) -> Option<&str> {
        self.ime_state.as_ref().map(|s| s.marked_text.as_str())
    }

    pub fn marked_text_range(&self) -> Option<std::ops::Range<usize>> {
        self.ime_state.as_ref().map(|s| 0..s.marked_text.encode_utf16().count())
    }

    pub fn set_marked_text(&mut self, text: String, cx: &mut Context<Self>) {
        if text.is_empty() {
            return self.clear_marked_text(cx);
        }
        self.ime_state = Some(ImeState { marked_text: text });
        cx.notify();
    }

    pub fn clear_marked_text(&mut self, cx: &mut Context<Self>) {
        if self.ime_state.is_some() {
            self.ime_state = None;
            cx.notify();
        }
    }

    pub fn commit_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if !text.is_empty() {
            self.send_terminal_input(text.as_bytes(), cx);
        }
    }

    pub fn try_process_keystroke(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        if let Some(bytes) = keystroke_to_bytes(keystroke, &self.vm.modes) {
            self.send_terminal_input(&bytes, cx);
            true
        } else {
            false
        }
    }

    fn input_enabled(&self, cx: &App) -> bool {
        self.ssh.read(cx).terminal_input_enabled()
    }

    fn send_terminal_input(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if !self.input_enabled(cx) || bytes.is_empty() {
            debug!(
                target: "qingqi_ssh",
                enabled = self.input_enabled(cx),
                bytes = bytes.len(),
                "term_diag: send_terminal_input 跳过"
            );
            return;
        }
        let Some(sid) = self.session_id.clone() else {
            debug!(target: "qingqi_ssh", "term_diag: send_terminal_input 无 session");
            return;
        };
        let before = (
            self.vm.display_offset,
            self.vm.cursor_row,
            self.vm.cursor_col,
        );
        self.service.terminal_scroll_to_bottom(&sid);
        let payload = bytes.to_vec();
        let sid_for_ssh = sid.clone();
        self.ssh.update(cx, |ssh, cx| {
            ssh.track_terminal_input(&sid_for_ssh, &payload, cx);
            let _ = ssh.service.send_terminal_input(&sid_for_ssh, &payload);
        });
        debug!(
            target: "qingqi_ssh",
            bytes = payload.len(),
            display_offset_before = before.0,
            display_offset_after = self.vm.display_offset,
            cursor_row_before = before.1,
            cursor_col_before = before.2,
            cursor_row_after = self.vm.cursor_row,
            cursor_col_after = self.vm.cursor_col,
            grid_rows = self.vm.grid.len(),
            "term_diag: pane send_terminal_input"
        );
        cx.notify();
    }

    fn submit_paste(&mut self, text: &str, cx: &mut Context<Self>) {
        let bytes = paste_text_to_bytes(text);
        if bytes.is_empty() {
            return;
        }
        if !self.input_enabled(cx) {
            return;
        }
        let Some(sid) = self.session_id else {
            return;
        };
        let modes = self.vm.modes;
        let payload = wrap_bracketed_paste(bytes, &modes);
        self.ssh.update(cx, |ssh, cx| {
            ssh.track_terminal_input(&sid, &payload, cx);
            let _ = ssh.service.send_terminal_input(&sid, &payload);
        });
    }

    fn submit_mouse(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.send_terminal_input(&bytes, cx);
    }

    fn mouse_modes(&self) -> crate::terminal::TerminalModes {
        self.vm.modes
    }

    fn char_metrics(&self) -> (Pixels, Pixels) {
        let line_height = px((self.font_size * LINE_HEIGHT_RATIO).max(15.0));
        let char_width = px((self.font_size * CHAR_WIDTH_RATIO).max(7.0));
        (char_width, line_height)
    }

    fn grid_dims(&self) -> (usize, usize) {
        let rows = self.vm.rows.max(self.vm.grid.len()).max(1);
        let cols = self
            .vm
            .cols
            .max(self.vm.grid.first().map(|r| r.len()).unwrap_or(80));
        (rows, cols)
    }

    fn cell_from_event(&self, position: Point<Pixels>) -> Option<super::terminal_element::GridPoint> {
        let origin = self.content_bounds?.origin;
        let (char_width, line_height) = self.char_metrics();
        let (rows, cols) = self.grid_dims();
        cell_at(position, origin, char_width, line_height, rows, cols)
    }

    fn scroll(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id else {
            return;
        };
        let offset_before = self.vm.display_offset;
        self.service.terminal_scroll(&sid, delta);
        self.refresh_terminal(Some(&sid));
        debug!(
            target: "qingqi_ssh",
            delta,
            offset_before,
            offset_after = self.vm.display_offset,
            max_offset = self.vm.max_display_offset,
            "term_diag: pane scroll"
        );
        cx.notify();
    }

}

impl Render for TerminalPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let term = &self.vm;
        let font_size = self.font_size;
        let line_height = (font_size * LINE_HEIGHT_RATIO).max(15.0);
        let is_log = matches!(term.terminal_kind, TerminalKind::Log);
        let (term_rows, term_cols) = self.grid_dims();
        let fh = self.focus_handle.clone();
        let selection = self.selection;
        let modes = self.mouse_modes();
        let mouse_active = modes.mouse_active();
        let pane_entity = cx.entity().clone();

        let body = if is_log {
            render_log_body(term, font_size, line_height).into_any_element()
        } else {
            render_shell_grid(term, font_size, &mut self.row_cache, selection)
        };

        let frame = div()
            .id("terminal-content")
            .flex_1()
            .size_full()
            .min_h(px(0.0))
            .min_w(px(0.0))
            .rounded(theme::radius_md())
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.88, 1.0))
            .bg(hsla(0.0, 0.0, 0.99, 1.0))
            .p(px(TERM_PADDING))
            .key_context("ssh_terminal")
            .cursor_text()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|view, _: &SshTerminalTab, _w, cx| {
                view.send_terminal_input(b"\t", cx);
            }))
            .on_action(cx.listener(|view, _: &SshTerminalShiftTab, _w, cx| {
                view.send_terminal_input(b"\x1b[Z", cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    window.focus(&fh);
                    if let Some(sid) = view.session_id.clone() {
                        view.service.terminal_scroll_to_bottom(&sid);
                        view.refresh_terminal(Some(&sid));
                    }
                    if let Some(point) = view.cell_from_event(event.position) {
                        if mouse_active {
                            let btn = mouse_button_code(MouseButton::Left);
                            let seq = encode_sgr_mouse(btn, point.col, point.row, false);
                            view.last_mouse_cell = Some(point);
                            view.submit_mouse(seq, cx);
                            return;
                        }
                        view.selection = Some(TerminalSelection {
                            anchor: point,
                            head: point,
                        });
                        view.selecting = true;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                    if !mouse_active {
                        return;
                    }
                    if let Some(point) = view.cell_from_event(event.position) {
                        let seq = encode_sgr_mouse(2, point.col, point.row, false);
                        view.submit_mouse(seq, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
                    if mouse_active {
                        if let Some(point) = view.cell_from_event(event.position) {
                            let btn = mouse_button_code(MouseButton::Left);
                            let seq = encode_sgr_mouse(btn | 3, point.col, point.row, true);
                            view.submit_mouse(seq, cx);
                        }
                        view.last_mouse_cell = None;
                        return;
                    }
                    view.selecting = false;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _window, cx| {
                if mouse_active {
                    if modes.mouse_motion || modes.mouse_drag {
                        if let Some(point) = view.cell_from_event(event.position) {
                            if view.last_mouse_cell != Some(point) {
                                let btn = mouse_button_code(MouseButton::Left) | 32;
                                let seq = encode_sgr_mouse(btn, point.col, point.row, false);
                                view.submit_mouse(seq, cx);
                                view.last_mouse_cell = Some(point);
                            }
                        }
                    }
                    return;
                }
                if !view.selecting {
                    return;
                }
                if let Some(point) = view.cell_from_event(event.position) {
                    if let Some(sel) = view.selection.as_mut() {
                        sel.head = point;
                        cx.notify();
                    }
                }
            }))
            .on_scroll_wheel(cx.listener(move |view, event: &ScrollWheelEvent, _window, cx| {
                if is_log {
                    return;
                }
                if mouse_active {
                    let col = (term_cols / 2).min(term_cols.saturating_sub(1));
                    let row = (term_rows / 2).min(term_rows.saturating_sub(1));
                    let up = scroll_delta_from_wheel(event, line_height) < 0;
                    let seq = encode_sgr_wheel(up, col, row);
                    view.submit_mouse(seq, cx);
                    cx.stop_propagation();
                    return;
                }
                let delta = scroll_delta_from_wheel(event, line_height);
                if delta != 0 {
                    cx.stop_propagation();
                    view.scroll(delta, cx);
                }
            }))
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                window.focus(&view.focus_handle);
                let ks = &event.keystroke;
                if ks.modifiers.platform {
                    let key = ks.key.as_str();
                    if key == "c" {
                        if let Some(sel) = view.selection.filter(|s| !s.is_empty()) {
                            let text = copy_selection_text(&view.vm, &sel);
                            if !text.is_empty() {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }
                            cx.stop_propagation();
                        }
                        return;
                    }
                    if key == "v" {
                        if let Some(item) = cx.read_from_clipboard() {
                            if let Some(text) = item.text() {
                                view.submit_paste(text.as_ref(), cx);
                            }
                        }
                        cx.stop_propagation();
                        return;
                    }
                    return;
                }

                if ks.key.as_str() == "tab" {
                    return;
                }

                if view.try_process_keystroke(ks, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .size_full()
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .child(body)
                    .when(!is_log, |layer| {
                        layer.child(TerminalShellElement::new(
                            pane_entity,
                            self.focus_handle.clone(),
                            font_size,
                        ))
                    }),
            );

        let frame = if is_log {
            frame.overflow_scrollbar().into_any_element()
        } else {
            frame.overflow_hidden().flex().flex_col().into_any_element()
        };

        div()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(ui::bg_surface())
            .child(render_status_bar(term))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .p_2()
                    .overflow_hidden()
                    .child(frame),
            )
    }
}
