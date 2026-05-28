use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use gpui::{
    App, AppContext, Component, Entity, ExternalPaths, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, hsla, px,
};

use crate::{
    app::{
        text_input::{TextInput, TextInputStyle},
        theme, ui,
    },
    core::storage::AppPaths,
    features::qr_code::{
        service::QrCodeService,
        store::{QrHistoryKind, QrHistoryRecord},
    },
    platform,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusTone {
    Neutral,
    Success,
    Error,
}

pub struct QrPanel {
    input: Option<Entity<TextInput>>,
    history_query_input: Option<Entity<TextInput>>,
    scan_path_input: Option<Entity<TextInput>>,
    service: QrCodeService,
    qr_matrix: Vec<bool>,
    qr_size: usize,
    message: String,
    tone: StatusTone,
    show_scan: bool,
    show_history: bool,
    history: Vec<QrHistoryRecord>,
    scan_result: String,
    scan_error: String,
}

impl QrPanel {
    pub fn new(paths: AppPaths) -> Result<Self> {
        let service = QrCodeService::new(paths)?;
        Ok(Self {
            input: None,
            history_query_input: None,
            scan_path_input: None,
            service,
            qr_matrix: Vec::new(),
            qr_size: 0,
            message: String::from("输入文本后点击生成"),
            tone: StatusTone::Neutral,
            show_scan: false,
            show_history: false,
            history: Vec::new(),
            scan_result: String::new(),
            scan_error: String::new(),
        })
    }

    pub fn ensure_inputs(&mut self, cx: &mut App) {
        if self.input.is_none() {
            self.input = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "输入文本或 URL", "");
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 172.0,
                        font_size: 12.0,
                        padding: 10.0,
                    },
                    cx,
                );
                input
            }));
        }

        if self.history_query_input.is_none() {
            self.history_query_input = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "搜索历史内容", "");
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 32.0,
                        font_size: 12.0,
                        padding: 8.0,
                    },
                    cx,
                );
                input
            }));
        }

        if self.scan_path_input.is_none() {
            self.scan_path_input = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "选择或粘贴二维码图片路径", "");
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 32.0,
                        font_size: 12.0,
                        padding: 8.0,
                    },
                    cx,
                );
                input
            }));
        }
    }

    fn input_text(&self, cx: &App) -> String {
        self.input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default()
    }

    fn history_query(&self, cx: &App) -> String {
        self.history_query_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default()
    }

    fn scan_path(&self, cx: &App) -> String {
        self.scan_path_input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default()
    }

    fn set_input_text(&mut self, text: impl Into<String>, cx: &mut App) {
        self.ensure_inputs(cx);
        if let Some(input) = self.input.as_ref() {
            let text = text.into();
            input.update(cx, |input, input_cx| input.set_text(text, input_cx));
        }
    }

    fn set_scan_path(&mut self, text: impl Into<String>, cx: &mut App) {
        self.ensure_inputs(cx);
        if let Some(input) = self.scan_path_input.as_ref() {
            let text = text.into();
            input.update(cx, |input, input_cx| input.set_text(text, input_cx));
        }
    }

    pub fn set_launch_input(&mut self, text: &str, cx: &mut App) {
        self.set_input_text(text.to_string(), cx);
        if text.trim().is_empty() {
            self.qr_matrix.clear();
            self.qr_size = 0;
            self.message = String::from("输入文本后点击生成");
            self.tone = StatusTone::Neutral;
            return;
        }
        self.generate_from_text(text);
    }

    pub fn generate_from_text(&mut self, text: &str) {
        match self.service.preview(text) {
            Ok(matrix) => {
                self.qr_size = matrix.size;
                self.qr_matrix = matrix.cells;
                self.message = format!("二维码已生成 ({}x{})", self.qr_size, self.qr_size);
                self.tone = StatusTone::Success;
            }
            Err(error) => {
                self.message = error.to_string();
                self.tone = StatusTone::Error;
                self.qr_matrix.clear();
                self.qr_size = 0;
            }
        }
    }

    pub fn refresh_history(&mut self, cx: &App) {
        let query = self.history_query(cx);
        match self.service.list_history(&query) {
            Ok(history) => self.history = history,
            Err(error) => {
                self.history.clear();
                self.message = format!("读取历史失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn toggle_history(&mut self, cx: &App) {
        self.show_history = !self.show_history;
        if self.show_history {
            self.show_scan = false;
        }
        if self.show_history {
            self.refresh_history(cx);
        }
    }

    pub fn toggle_scan(&mut self) {
        self.show_scan = !self.show_scan;
        if self.show_scan {
            self.show_history = false;
        }
    }

    pub fn save_current(&mut self, cx: &App) {
        let text = self.input_text(cx);
        match self.service.save(&text) {
            Ok(path) => {
                self.generate_from_text(&text);
                self.message = format!("已保存到: {}", path.display());
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Err(error) => {
                self.message = format!("保存失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn copy_current(&mut self, cx: &mut App) {
        let text = self.input_text(cx);
        if text.trim().is_empty() {
            self.message = String::from("无可复制内容");
            self.tone = StatusTone::Error;
            return;
        }
        platform::clipboard::write_text(cx, text.clone());
        match self.service.record_copy(&text) {
            Ok(_) => {
                self.message = String::from("已复制内容");
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Err(error) => {
                self.message = format!("复制记录失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn fill_from_clipboard(&mut self, cx: &mut App) {
        let text = platform::clipboard::read_text(cx).unwrap_or_default();
        if text.trim().is_empty() {
            self.message = String::from("剪贴板没有可用文本");
            self.tone = StatusTone::Error;
            return;
        }
        self.set_input_text(text.clone(), cx);
        self.generate_from_text(&text);
    }

    pub fn clear_input(&mut self, cx: &mut App) {
        self.ensure_inputs(cx);
        if let Some(input) = self.input.as_ref() {
            input.update(cx, |input, input_cx| input.clear(input_cx));
        }
        self.qr_matrix.clear();
        self.qr_size = 0;
        self.message = String::from("已清空");
        self.tone = StatusTone::Neutral;
    }

    pub fn choose_scan_image(&mut self, cx: &mut App) {
        match platform::shell::choose_file("选择二维码图片") {
            Ok(Some(path)) => {
                let path = path.to_string_lossy().to_string();
                self.set_scan_path(path, cx);
                self.scan_selected_path(cx);
            }
            Ok(None) => {
                self.message = String::from("已取消选择图片");
                self.tone = StatusTone::Neutral;
            }
            Err(error) => {
                self.message = format!("打开文件选择失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn scan_selected_path(&mut self, cx: &mut App) {
        let path = self.scan_path(cx);
        if path.trim().is_empty() {
            self.scan_result.clear();
            self.scan_error = String::from("请先选择二维码图片");
            self.message = self.scan_error.clone();
            self.tone = StatusTone::Error;
            return;
        }

        self.scan_path_text(path, cx);
    }

    pub fn scan_path_text(&mut self, path: String, cx: &mut App) {
        match self.service.scan_image_input(&path) {
            Ok((text, normalized_path)) => {
                let normalized = normalized_path.to_string_lossy().to_string();
                self.set_scan_path(normalized, cx);
                self.scan_result = text.clone();
                self.scan_error.clear();
                self.message = String::from("扫描成功");
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Err(error) => {
                self.scan_result.clear();
                self.scan_error = error.to_string();
                self.message = format!("扫描失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn copy_scan_result(&mut self, cx: &mut App) {
        if self.scan_result.trim().is_empty() {
            self.message = String::from("暂无扫描结果");
            self.tone = StatusTone::Error;
            return;
        }

        platform::clipboard::write_text(cx, self.scan_result.clone());
        match self.service.record_copy(&self.scan_result) {
            Ok(_) => {
                self.message = String::from("已复制扫描结果");
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Err(error) => {
                self.message = format!("复制扫描结果失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn use_scan_result(&mut self, cx: &mut App) {
        if self.scan_result.trim().is_empty() {
            self.message = String::from("暂无扫描结果");
            self.tone = StatusTone::Error;
            return;
        }
        let text = self.scan_result.clone();
        self.set_input_text(text.clone(), cx);
        self.generate_from_text(&text);
        self.message = String::from("已将扫描结果用作生成内容");
        self.tone = StatusTone::Success;
    }

    pub fn reveal_save_root(&mut self) {
        if let Err(error) = std::fs::create_dir_all(self.service.save_root()) {
            self.message = format!("创建目录失败: {error}");
            self.tone = StatusTone::Error;
            return;
        }
        match platform::shell::open_path(self.service.save_root()) {
            Ok(_) => {
                self.message = format!("已打开目录: {}", self.service.save_root().display());
                self.tone = StatusTone::Success;
            }
            Err(error) => {
                self.message = format!("打开目录失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn export_history(&mut self, cx: &App) {
        match self.service.export_history_auto() {
            Ok(path) => {
                self.message = format!("已导出到: {}", path.display());
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Err(error) => {
                self.message = format!("导出失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn clear_history(&mut self, cx: &App) {
        match self.service.clear_history() {
            Ok(_) => {
                self.message = String::from("历史记录已清空");
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Err(error) => {
                self.message = format!("清空历史失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn remove_history_item(&mut self, id: &str, cx: &App) {
        match self.service.remove_history(id) {
            Ok(true) => {
                self.message = String::from("已删除历史记录");
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Ok(false) => {
                self.message = String::from("未找到对应的历史记录");
                self.tone = StatusTone::Error;
            }
            Err(error) => {
                self.message = format!("删除历史失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn copy_history_item(&mut self, item: &QrHistoryRecord, cx: &mut App) {
        platform::clipboard::write_text(cx, item.content.clone());
        match self.service.record_copy(&item.content) {
            Ok(_) => {
                self.message = String::from("已复制历史内容");
                self.tone = StatusTone::Success;
                self.refresh_history(cx);
            }
            Err(error) => {
                self.message = format!("复制历史失败: {error}");
                self.tone = StatusTone::Error;
            }
        }
    }

    pub fn use_history_item(&mut self, item: &QrHistoryRecord, cx: &mut App) {
        self.set_input_text(item.content.clone(), cx);
        self.generate_from_text(&item.content);
        self.message = format!("已载入{}记录", item.kind.label());
        self.tone = StatusTone::Success;
    }

    pub fn clear_view_state(&mut self) {
        self.qr_matrix.clear();
        self.qr_size = 0;
        self.message = String::from("输入文本后点击生成");
        self.tone = StatusTone::Neutral;
        self.show_scan = false;
        self.show_history = false;
        self.scan_result.clear();
        self.scan_error.clear();
    }
}

pub struct QrCodeElement {
    pub panel: Rc<RefCell<QrPanel>>,
}

impl IntoElement for QrCodeElement {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for QrCodeElement {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.panel.borrow_mut().ensure_inputs(cx);

        let panel = self.panel.borrow();
        let dark = crate::app::theme_mode::is_dark();
        let input = panel.input.clone().expect("qr input should exist");
        let history_query = panel
            .history_query_input
            .clone()
            .expect("history query input should exist");
        let scan_path_input = panel
            .scan_path_input
            .clone()
            .expect("scan path input should exist");
        let qr_matrix = panel.qr_matrix.clone();
        let qr_size = panel.qr_size;
        let message = panel.message.clone();
        let tone = panel.tone;
        let show_scan = panel.show_scan;
        let show_history = panel.show_history;
        let history = panel.history.clone();
        let scan_result = panel.scan_result.clone();
        let scan_error = panel.scan_error.clone();
        let input_text = input.read(cx).text();
        let save_root = panel.service.save_root().display().to_string();
        drop(panel);

        ui::plugin_surface(dark)
            .relative()
            .child(
                ui::plugin_scroll_content()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(module_title("📱 二维码", "QR Code", dark))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        utility_button("扫描", show_scan, dark)
                                            .id("qr-scan-btn")
                                            .on_click({
                                                let panel = Rc::clone(&self.panel);
                                                move |_, window, cx| {
                                                    panel.borrow_mut().toggle_scan();
                                                    window.refresh();
                                                }
                                            }),
                                    )
                                    .child(
                                        icon_button(show_history, dark, history.len())
                                            .id("qr-history-btn")
                                            .on_click({
                                                let panel = Rc::clone(&self.panel);
                                                move |_, window, cx| {
                                                    panel.borrow_mut().toggle_history(cx);
                                                    window.refresh();
                                                }
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(section_label("输入内容", dark))
                                    .child(
                                        div()
                                            .rounded(px(12.0))
                                            .bg(theme::rgba_with_alpha(
                                                theme::token("color-bg-surface", dark),
                                                0.82,
                                            ))
                                            .border_1()
                                            .border_color(theme::launcher_soft_line(dark))
                                            .child(input),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                primary_action_button("保存图片", dark)
                                                    .id("qr-save-btn")
                                                    .on_click({
                                                        let panel = Rc::clone(&self.panel);
                                                        move |_, window, cx| {
                                                            panel.borrow_mut().save_current(cx);
                                                            window.refresh();
                                                        }
                                                    }),
                                            )
                                            .child(
                                                action_button("复制内容", dark)
                                                    .id("qr-copy-btn")
                                                    .on_click({
                                                        let panel = Rc::clone(&self.panel);
                                                        move |_, window, cx| {
                                                            panel.borrow_mut().copy_current(cx);
                                                            window.refresh();
                                                        }
                                                    }),
                                            )
                                            .child(
                                                action_button("从剪贴板", dark)
                                                    .id("qr-paste-btn")
                                                    .on_click({
                                                        let panel = Rc::clone(&self.panel);
                                                        move |_, window, cx| {
                                                            panel
                                                                .borrow_mut()
                                                                .fill_from_clipboard(cx);
                                                            window.refresh();
                                                        }
                                                    }),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                ghost_button("生成", dark)
                                                    .id("qr-generate-btn")
                                                    .on_click({
                                                        let panel = Rc::clone(&self.panel);
                                                        move |_, window, cx| {
                                                            let text =
                                                                panel.borrow().input_text(cx);
                                                            panel
                                                                .borrow_mut()
                                                                .generate_from_text(&text);
                                                            window.refresh();
                                                        }
                                                    }),
                                            )
                                            .child(
                                                ghost_button("清空", dark)
                                                    .id("qr-clear-btn")
                                                    .on_click({
                                                        let panel = Rc::clone(&self.panel);
                                                        move |_, window, cx| {
                                                            panel.borrow_mut().clear_input(cx);
                                                            window.refresh();
                                                        }
                                                    }),
                                            ),
                                    )
                                    .child(status_bar(
                                        Rc::clone(&self.panel),
                                        message,
                                        tone,
                                        save_root,
                                        dark,
                                    )),
                            )
                            .child(preview_panel(dark, qr_matrix, qr_size)),
                    ),
            )
            .child(if show_scan {
                overlay_shell(
                    dark,
                    "qr-scan-overlay",
                    Rc::clone(&self.panel),
                    scan_panel(
                        Rc::clone(&self.panel),
                        scan_path_input,
                        scan_result,
                        scan_error,
                        dark,
                    ),
                )
                .into_any_element()
            } else if show_history {
                overlay_shell(
                    dark,
                    "qr-history-overlay",
                    Rc::clone(&self.panel),
                    history_panel(
                        Rc::clone(&self.panel),
                        history_query,
                        history,
                        dark,
                        input_text.is_empty(),
                    ),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
    }
}

fn preview_panel(dark: bool, qr_matrix: Vec<bool>, qr_size: usize) -> impl IntoElement {
    div()
        .w(px(220.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(section_label("预览", dark))
        .child(
            div()
                .size(px(220.0))
                .rounded(px(14.0))
                .bg(theme::rgba_with_alpha(
                    theme::token("color-bg-surface", dark),
                    0.88,
                ))
                .border_1()
                .border_color(theme::launcher_soft_line(dark))
                .flex()
                .items_center()
                .justify_center()
                .child({
                    if !qr_matrix.is_empty() && qr_size > 0 {
                        let max_preview = 176.0;
                        let cell_size = (max_preview / qr_size as f32).floor().max(2.0);
                        let total_px = qr_size as f32 * cell_size;
                        let dark_color = if dark {
                            hsla(0.0, 0.0, 0.92, 1.0)
                        } else {
                            hsla(0.0, 0.0, 0.0, 1.0)
                        };
                        let light_color = if dark {
                            hsla(0.0, 0.0, 0.18, 1.0)
                        } else {
                            hsla(0.0, 0.0, 1.0, 1.0)
                        };

                        div()
                            .rounded(px(10.0))
                            .bg(light_color)
                            .p(px(12.0))
                            .child(div().size(px(total_px)).flex().flex_col().children(
                                (0..qr_size).map(|row| {
                                    let cells: Vec<_> = (0..qr_size)
                                        .map(|col| {
                                            let idx = row * qr_size + col;
                                            let filled =
                                                qr_matrix.get(idx).copied().unwrap_or(false);
                                            div().size(px(cell_size)).bg(if filled {
                                                dark_color
                                            } else {
                                                light_color
                                            })
                                        })
                                        .collect();
                                    div().flex().children(cells)
                                }),
                            ))
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::launcher_faint_text(dark))
                            .child("二维码预览")
                            .into_any_element()
                    }
                }),
        )
}

fn scan_panel(
    panel: Rc<RefCell<QrPanel>>,
    scan_path_input: Entity<TextInput>,
    scan_result: String,
    scan_error: String,
    dark: bool,
) -> impl IntoElement {
    let result_text = if !scan_error.is_empty() {
        scan_error.clone()
    } else if !scan_result.is_empty() {
        scan_result.clone()
    } else {
        String::from("扫描结果将显示在这里")
    };
    let result_tone = if !scan_error.is_empty() {
        theme::token("color-danger", dark)
    } else if !scan_result.is_empty() {
        theme::token("color-text-primary", dark)
    } else {
        theme::token("color-text-secondary", dark)
    };

    div()
        .w(px(520.0))
        .rounded(px(10.0))
        .shadow_lg()
        .bg(theme::token("color-bg-surface", dark))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(42.0))
                .px_3()
                .border_b_1()
                .border_color(theme::token("color-border-default", dark))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("扫描二维码"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            action_button("选择图片", dark)
                                .id("qr-scan-choose")
                                .on_click({
                                    let panel = Rc::clone(&panel);
                                    move |_, window, cx| {
                                        panel.borrow_mut().choose_scan_image(cx);
                                        window.refresh();
                                    }
                                }),
                        )
                        .child(action_button("扫描", dark).id("qr-scan-run").on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, cx| {
                                panel.borrow_mut().scan_selected_path(cx);
                                window.refresh();
                            }
                        }))
                        .child(action_button("关闭", dark).id("qr-scan-close").on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, cx| {
                                panel.borrow_mut().show_scan = false;
                                window.refresh();
                            }
                        })),
                ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme::token("color-border-default", dark))
                .child(
                    div()
                        .id("qr-scan-drop-zone")
                        .rounded(px(8.0))
                        .bg(theme::token("color-bg-page", dark))
                        .border_1()
                        .border_color(theme::token("color-border-default", dark))
                        .drag_over::<ExternalPaths>(move |style, _, _, _| {
                            style.bg(theme::rgba_with_alpha(
                                theme::accent_color(theme::ThemeAccent::Cyan),
                                if dark { 0.18 } else { 0.10 },
                            ))
                        })
                        .on_drop({
                            let panel = Rc::clone(&panel);
                            move |paths: &ExternalPaths, window, cx| {
                                if let Some(path) = paths.paths().first() {
                                    panel
                                        .borrow_mut()
                                        .scan_path_text(path.to_string_lossy().to_string(), cx);
                                    window.refresh();
                                }
                            }
                        })
                        .child(scan_path_input),
                ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .h(px(150.0))
                .id("qr-scan-result-scroll")
                .overflow_y_scroll()
                .child(
                    div()
                        .font_family("SF Mono")
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(result_tone)
                        .child(result_text),
                ),
        )
        .child(
            div()
                .px_3()
                .pb_3()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    action_button("复制结果", dark)
                        .id("qr-scan-copy")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, cx| {
                                panel.borrow_mut().copy_scan_result(cx);
                                window.refresh();
                            }
                        }),
                )
                .child(
                    action_button("用作生成内容", dark)
                        .id("qr-scan-use")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, cx| {
                                panel.borrow_mut().use_scan_result(cx);
                                window.refresh();
                            }
                        }),
                ),
        )
}

fn history_panel(
    panel: Rc<RefCell<QrPanel>>,
    history_query: Entity<TextInput>,
    history: Vec<QrHistoryRecord>,
    dark: bool,
    _input_is_empty: bool,
) -> impl IntoElement {
    let count = history.len();
    div()
        .w(px(560.0))
        .rounded(px(10.0))
        .shadow_lg()
        .bg(theme::token("color-bg-surface", dark))
        .border_1()
        .border_color(theme::token("color-border-default", dark))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(42.0))
                .px_3()
                .border_b_1()
                .border_color(theme::token("color-border-default", dark))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("历史记录"),
                        )
                        .child(
                            div()
                                .h(px(20.0))
                                .px_2()
                                .rounded(px(999.0))
                                .bg(theme::token("color-bg-subtle", dark))
                                .text_size(px(11.0))
                                .text_color(theme::token("color-text-secondary", dark))
                                .flex()
                                .items_center()
                                .child(format!("{count} 条")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            action_button("导出", dark)
                                .id("qr-history-export")
                                .on_click({
                                    let panel = Rc::clone(&panel);
                                    move |_, window, cx| {
                                        panel.borrow_mut().export_history(cx);
                                        window.refresh();
                                    }
                                }),
                        )
                        .child(
                            action_button("清空", dark)
                                .id("qr-history-clear")
                                .on_click({
                                    let panel = Rc::clone(&panel);
                                    move |_, window, cx| {
                                        panel.borrow_mut().clear_history(cx);
                                        window.refresh();
                                    }
                                }),
                        )
                        .child(
                            action_button("关闭", dark)
                                .id("qr-history-close")
                                .on_click({
                                    let panel = Rc::clone(&panel);
                                    move |_, window, cx| {
                                        panel.borrow_mut().show_history = false;
                                        window.refresh();
                                    }
                                }),
                        ),
                ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme::token("color-border-default", dark))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .rounded(px(8.0))
                                .bg(theme::token("color-bg-page", dark))
                                .border_1()
                                .border_color(theme::token("color-border-default", dark))
                                .child(history_query),
                        )
                        .child(
                            action_button("筛选", dark)
                                .id("qr-history-filter")
                                .on_click({
                                    let panel = Rc::clone(&panel);
                                    move |_, window, cx| {
                                        panel.borrow_mut().refresh_history(cx);
                                        window.refresh();
                                    }
                                }),
                        ),
                ),
        )
        .child(if history.is_empty() {
            div()
                .h(px(120.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .text_color(theme::token("color-text-secondary", dark))
                .child("暂无历史记录")
                .into_any_element()
        } else {
            div()
                .h(px(220.0))
                .id("qr-history-scroll")
                .overflow_y_scroll()
                .children(
                    history
                        .into_iter()
                        .enumerate()
                        .map(|(index, item)| history_row(Rc::clone(&panel), item, index, dark)),
                )
                .into_any_element()
        })
}

fn overlay_shell(
    dark: bool,
    backdrop_id: &'static str,
    panel: Rc<RefCell<QrPanel>>,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .size_full()
        .absolute()
        .top_0()
        .left_0()
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .bg(hsla(0.0, 0.0, 0.0, if dark { 0.42 } else { 0.24 }))
                .id(backdrop_id)
                .on_click(move |_, window, cx| {
                    let mut panel = panel.borrow_mut();
                    panel.show_scan = false;
                    panel.show_history = false;
                    window.refresh();
                }),
        )
        .child(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .flex()
                .items_center()
                .justify_center()
                .child(content),
        )
}

fn history_row(
    panel: Rc<RefCell<QrPanel>>,
    item: QrHistoryRecord,
    index: usize,
    dark: bool,
) -> impl IntoElement {
    let tone = match item.kind {
        QrHistoryKind::Save => theme::token("color-success", dark),
        QrHistoryKind::Copy => theme::token("color-primary-active", dark),
        QrHistoryKind::Scan => theme::token("color-info", dark),
    };

    div()
        .h(px(44.0))
        .px_3()
        .border_b_1()
        .border_color(theme::token("color-border-default", dark))
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(36.0))
                .text_size(px(11.0))
                .text_color(tone)
                .child(item.kind.label()),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .font_family("SF Mono")
                .text_color(theme::token("color-text-primary", dark))
                .child(item.content.clone()),
        )
        .child(
            div()
                .w(px(76.0))
                .text_size(px(11.0))
                .text_color(theme::token("color-text-secondary", dark))
                .child(item.created_at.split(' ').last().unwrap_or("").to_string()),
        )
        .child(
            action_button("用", dark)
                .id(("qr-history-use", index))
                .on_click({
                    let panel = Rc::clone(&panel);
                    let item = item.clone();
                    move |_, window, cx| {
                        panel.borrow_mut().use_history_item(&item, cx);
                        window.refresh();
                    }
                }),
        )
        .child(
            action_button("复制", dark)
                .id(("qr-history-copy", index))
                .on_click({
                    let panel = Rc::clone(&panel);
                    let item = item.clone();
                    move |_, window, cx| {
                        panel.borrow_mut().copy_history_item(&item, cx);
                        window.refresh();
                    }
                }),
        )
        .child(
            action_button("删除", dark)
                .id(("qr-history-delete", index))
                .on_click({
                    let panel = Rc::clone(&panel);
                    let item_id = item.id.clone();
                    move |_, window, cx| {
                        panel.borrow_mut().remove_history_item(&item_id, cx);
                        window.refresh();
                    }
                }),
        )
}

fn status_bar(
    panel: Rc<RefCell<QrPanel>>,
    message: String,
    tone: StatusTone,
    save_root: String,
    dark: bool,
) -> impl IntoElement {
    div()
        .h(px(32.0))
        .rounded(px(10.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.7,
        ))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .text_color(match tone {
                    StatusTone::Neutral => theme::launcher_muted_text(dark),
                    StatusTone::Success => theme::token("color-success", dark),
                    StatusTone::Error => theme::token("color-danger", dark),
                })
                .child(message),
        )
        .child(
            div()
                .w(px(320.0))
                .text_size(px(10.0))
                .font_family("SF Mono")
                .text_color(theme::launcher_faint_text(dark))
                .child(save_root),
        )
        .child(
            ghost_button("打开目录", dark)
                .id("qr-open-save-root")
                .on_click(move |_, window, cx| {
                    panel.borrow_mut().reveal_save_root();
                    window.refresh();
                }),
        )
}

fn section_label(label: &str, dark: bool) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .text_color(theme::launcher_muted_text(dark))
        .child(label.to_string())
}

fn primary_action_button(label: &str, _dark: bool) -> gpui::Div {
    div()
        .h(px(34.0))
        .px_3()
        .rounded(px(8.0))
        .bg(ui::accent_color(
            crate::core::plugin_spec::PluginAccent::Blue,
        ))
        .hover(|style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::white())
        .child(label.to_string())
}

fn action_button(label: &str, dark: bool) -> gpui::Div {
    div()
        .h(px(32.0))
        .px_3()
        .rounded(px(8.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.88,
        ))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::token("color-text-primary", dark))
        .child(label.to_string())
}

fn utility_button(label: &str, active: bool, dark: bool) -> gpui::Div {
    div()
        .h(px(28.0))
        .px_2()
        .rounded(px(999.0))
        .bg(if active {
            theme::rgba_with_alpha(
                ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue),
                0.18,
            )
        } else {
            theme::rgba_with_alpha(theme::token("color-bg-surface", dark), 0.8)
        })
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(if active {
            ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue)
        } else {
            theme::token("color-text-primary", dark)
        })
        .child(label.to_string())
}

fn icon_button(active: bool, dark: bool, count: usize) -> gpui::Div {
    div()
        .h(px(28.0))
        .px_2()
        .rounded(px(999.0))
        .bg(if active {
            theme::rgba_with_alpha(
                ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue),
                0.18,
            )
        } else {
            theme::rgba_with_alpha(theme::token("color-bg-surface", dark), 0.8)
        })
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .child(ui::icon_element(
            "qta/mdi6.history.png",
            if active {
                ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue)
            } else {
                theme::token("color-text-primary", dark)
            },
            13.0,
        ))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(if active {
                    ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue)
                } else {
                    theme::token("color-text-primary", dark)
                })
                .child(count.to_string()),
        )
}

fn module_title(title: &str, tag: &str, dark: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::token("color-text-primary", dark))
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::launcher_muted_text(dark))
                .child(tag.to_string()),
        )
}

fn ghost_button(label: &str, dark: bool) -> gpui::Div {
    div()
        .h(px(32.0))
        .px_3()
        .rounded(px(8.0))
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::launcher_muted_text(dark))
        .child(label.to_string())
}
