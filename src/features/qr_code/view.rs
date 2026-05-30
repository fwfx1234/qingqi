use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use gpui::{
    App, AppContext, AsyncApp, Component, Entity, ExternalPaths, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, hsla, px,
};

use crate::{
    app::{
        text_input::{TextInput, TextInputStyle},
        theme,
        ui::{self, components},
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
    pending_action: Arc<Mutex<Option<QrBackgroundResult>>>,
}

enum QrBackgroundResult {
    Preview(std::result::Result<crate::features::qr_code::service::QrMatrix, String>),
    History(std::result::Result<Vec<QrHistoryRecord>, String>),
    Save {
        text: String,
        result: std::result::Result<PathBuf, String>,
    },
    Scan(std::result::Result<(String, String), String>),
    Export(std::result::Result<PathBuf, String>),
    ClearHistory(std::result::Result<(), String>),
    RemoveHistory(std::result::Result<bool, String>),
    RecordCopy {
        success_message: String,
        result: std::result::Result<(), String>,
    },
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
            pending_action: Arc::new(Mutex::new(None)),
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
        self.generate_from_text(text, cx.to_async());
    }

    pub fn generate_from_text(&mut self, text: &str, async_cx: AsyncApp) {
        self.message = String::from("正在生成二维码...");
        self.tone = StatusTone::Neutral;
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        let text = text.to_string();
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move { service.preview(&text).map_err(|error| error.to_string()) })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::Preview(result));
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn refresh_history(&mut self, cx: &App, async_cx: AsyncApp) {
        self.message = String::from("正在读取历史...");
        self.tone = StatusTone::Neutral;
        let query = self.history_query(cx);
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        service
                            .list_history(&query)
                            .map_err(|error| error.to_string())
                    })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::History(result));
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn toggle_history(&mut self, cx: &App) {
        self.show_history = !self.show_history;
        if self.show_history {
            self.show_scan = false;
        }
        if self.show_history {
            self.refresh_history(cx, cx.to_async());
        }
    }

    pub fn toggle_scan(&mut self) {
        self.show_scan = !self.show_scan;
        if self.show_scan {
            self.show_history = false;
        }
    }

    pub fn save_current(&mut self, cx: &App, async_cx: AsyncApp) {
        let text = self.input_text(cx);
        let target_dir = match platform::shell::choose_directory("选择保存文件夹") {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.message = String::from("已取消保存");
                self.tone = StatusTone::Neutral;
                return;
            }
            Err(error) => {
                self.message = format!("打开文件夹选择失败: {error}");
                self.tone = StatusTone::Error;
                return;
            }
        };

        self.message = String::from("正在保存二维码...");
        self.tone = StatusTone::Neutral;
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        async_cx
            .spawn(async move |async_cx| {
                let save_text = text.clone();
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        service
                            .save_to_dir(&save_text, &target_dir)
                            .map_err(|error| error.to_string())
                    })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::Save { text, result });
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn copy_current(&mut self, cx: &mut App) {
        let text = self.input_text(cx);
        if text.trim().is_empty() {
            self.message = String::from("无可复制内容");
            self.tone = StatusTone::Error;
            return;
        }
        platform::clipboard::write_text(cx, text.clone());
        self.record_copy_async(text, String::from("已复制内容"), cx.to_async());
    }

    pub fn fill_from_clipboard(&mut self, cx: &mut App) {
        let text = platform::clipboard::read_text(cx).unwrap_or_default();
        if text.trim().is_empty() {
            self.message = String::from("剪贴板没有可用文本");
            self.tone = StatusTone::Error;
            return;
        }
        self.set_input_text(text.clone(), cx);
        self.generate_from_text(&text, cx.to_async());
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

        self.scan_path_text(path, cx.to_async());
    }

    pub fn scan_path_text(&mut self, path: String, async_cx: AsyncApp) {
        self.message = String::from("正在扫描二维码...");
        self.tone = StatusTone::Neutral;
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        service
                            .scan_image_input(&path)
                            .map(|(text, normalized_path)| {
                                (text, normalized_path.to_string_lossy().to_string())
                            })
                            .map_err(|error| error.to_string())
                    })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::Scan(result));
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn copy_scan_result(&mut self, cx: &mut App) {
        if self.scan_result.trim().is_empty() {
            self.message = String::from("暂无扫描结果");
            self.tone = StatusTone::Error;
            return;
        }

        platform::clipboard::write_text(cx, self.scan_result.clone());
        self.record_copy_async(
            self.scan_result.clone(),
            String::from("已复制扫描结果"),
            cx.to_async(),
        );
    }

    pub fn use_scan_result(&mut self, cx: &mut App) {
        if self.scan_result.trim().is_empty() {
            self.message = String::from("暂无扫描结果");
            self.tone = StatusTone::Error;
            return;
        }
        let text = self.scan_result.clone();
        self.set_input_text(text.clone(), cx);
        self.generate_from_text(&text, cx.to_async());
        self.message = String::from("已将扫描结果用作生成内容");
        self.tone = StatusTone::Success;
    }

    pub fn export_history(&mut self, _cx: &App, async_cx: AsyncApp) {
        let target_dir = match platform::shell::choose_directory("选择导出文件夹") {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.message = String::from("已取消导出");
                self.tone = StatusTone::Neutral;
                return;
            }
            Err(error) => {
                self.message = format!("打开文件夹选择失败: {error}");
                self.tone = StatusTone::Error;
                return;
            }
        };

        self.message = String::from("正在导出历史...");
        self.tone = StatusTone::Neutral;
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        service
                            .export_history_to_dir(&target_dir)
                            .map_err(|error| error.to_string())
                    })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::Export(result));
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn clear_history(&mut self, async_cx: AsyncApp) {
        self.message = String::from("正在清空历史...");
        self.tone = StatusTone::Neutral;
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(
                        async move { service.clear_history().map_err(|error| error.to_string()) },
                    )
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::ClearHistory(result));
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn remove_history_item(&mut self, id: &str, async_cx: AsyncApp) {
        self.message = String::from("正在删除历史...");
        self.tone = StatusTone::Neutral;
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        let id = id.to_string();
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        service
                            .remove_history(&id)
                            .map_err(|error| error.to_string())
                    })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::RemoveHistory(result));
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn copy_history_item(&mut self, item: &QrHistoryRecord, cx: &mut App) {
        platform::clipboard::write_text(cx, item.content.clone());
        self.record_copy_async(
            item.content.clone(),
            String::from("已复制历史内容"),
            cx.to_async(),
        );
    }

    pub fn use_history_item(&mut self, item: &QrHistoryRecord, cx: &mut App) {
        self.set_input_text(item.content.clone(), cx);
        self.generate_from_text(&item.content, cx.to_async());
        self.message = format!("已载入{}记录", item.kind.label());
        self.tone = StatusTone::Success;
    }

    fn record_copy_async(&mut self, text: String, success_message: String, async_cx: AsyncApp) {
        self.message = String::from("正在写入历史...");
        self.tone = StatusTone::Neutral;
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        service
                            .record_copy(&text)
                            .map(|_| ())
                            .map_err(|error| error.to_string())
                    })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(QrBackgroundResult::RecordCopy {
                        success_message,
                        result,
                    });
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    fn collect_pending(&mut self, cx: &mut App) {
        let pending = self
            .pending_action
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        let Some(action) = pending else {
            return;
        };

        match action {
            QrBackgroundResult::Preview(result) => match result {
                Ok(matrix) => {
                    self.qr_size = matrix.size;
                    self.qr_matrix = matrix.cells;
                    self.message = format!("二维码已生成 ({}x{})", self.qr_size, self.qr_size);
                    self.tone = StatusTone::Success;
                }
                Err(error) => {
                    self.message = error;
                    self.tone = StatusTone::Error;
                    self.qr_matrix.clear();
                    self.qr_size = 0;
                }
            },
            QrBackgroundResult::History(result) => match result {
                Ok(history) => self.history = history,
                Err(error) => {
                    self.history.clear();
                    self.message = format!("读取历史失败: {error}");
                    self.tone = StatusTone::Error;
                }
            },
            QrBackgroundResult::Save { text, result } => match result {
                Ok(path) => {
                    self.message = format!("已保存到: {}", path.display());
                    self.tone = StatusTone::Success;
                    self.generate_from_text(&text, cx.to_async());
                    self.refresh_history(cx, cx.to_async());
                }
                Err(error) => {
                    self.message = format!("保存失败: {error}");
                    self.tone = StatusTone::Error;
                }
            },
            QrBackgroundResult::Scan(result) => match result {
                Ok((text, normalized)) => {
                    self.set_scan_path(normalized, cx);
                    self.scan_result = text;
                    self.scan_error.clear();
                    self.message = String::from("扫描成功");
                    self.tone = StatusTone::Success;
                    self.refresh_history(cx, cx.to_async());
                }
                Err(error) => {
                    self.scan_result.clear();
                    self.scan_error = error.clone();
                    self.message = format!("扫描失败: {error}");
                    self.tone = StatusTone::Error;
                }
            },
            QrBackgroundResult::Export(result) => match result {
                Ok(path) => {
                    self.message = format!("已导出到: {}", path.display());
                    self.tone = StatusTone::Success;
                    self.refresh_history(cx, cx.to_async());
                }
                Err(error) => {
                    self.message = format!("导出失败: {error}");
                    self.tone = StatusTone::Error;
                }
            },
            QrBackgroundResult::ClearHistory(result) => match result {
                Ok(()) => {
                    self.history.clear();
                    self.message = String::from("历史记录已清空");
                    self.tone = StatusTone::Success;
                }
                Err(error) => {
                    self.message = format!("清空历史失败: {error}");
                    self.tone = StatusTone::Error;
                }
            },
            QrBackgroundResult::RemoveHistory(result) => match result {
                Ok(true) => {
                    self.message = String::from("已删除历史记录");
                    self.tone = StatusTone::Success;
                    self.refresh_history(cx, cx.to_async());
                }
                Ok(false) => {
                    self.message = String::from("未找到对应的历史记录");
                    self.tone = StatusTone::Error;
                }
                Err(error) => {
                    self.message = format!("删除历史失败: {error}");
                    self.tone = StatusTone::Error;
                }
            },
            QrBackgroundResult::RecordCopy {
                success_message,
                result,
            } => match result {
                Ok(()) => {
                    self.message = success_message;
                    self.tone = StatusTone::Success;
                    self.refresh_history(cx, cx.to_async());
                }
                Err(error) => {
                    self.message = format!("写入历史失败: {error}");
                    self.tone = StatusTone::Error;
                }
            },
        }
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
        {
            let mut panel = self.panel.borrow_mut();
            panel.collect_pending(cx);
            panel.ensure_inputs(cx);
        }

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
                                                move |_, window, _cx| {
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
                                                theme::semantic().bg_surface,
                                                0.82,
                                            ))
                                            .border_1()
                                            .border_color(ui::border_light())
                                            .child(input),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                primary_action_button("另存为", dark)
                                                    .id("qr-save-btn")
                                                    .on_click({
                                                        let panel = Rc::clone(&self.panel);
                                                        move |_, window, cx| {
                                                            panel
                                                                .borrow_mut()
                                                                .save_current(cx, cx.to_async());
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
                                                            panel.borrow_mut().generate_from_text(
                                                                &text,
                                                                cx.to_async(),
                                                            );
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
                                    .child(status_bar(message, tone, dark)),
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
                .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.88))
                .border_1()
                .border_color(ui::border_light())
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
                            .text_color(ui::text_tertiary())
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
        theme::semantic().danger
    } else if !scan_result.is_empty() {
        theme::semantic().text_primary
    } else {
        theme::semantic().text_secondary
    };

    div()
        .w(px(520.0))
        .rounded(px(10.0))
        .shadow_lg()
        .bg(theme::semantic().bg_surface)
        .border_1()
        .border_color(theme::semantic().border_default)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(42.0))
                .px_3()
                .border_b_1()
                .border_color(theme::semantic().border_default)
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
                            move |_, window, _cx| {
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
                .border_color(theme::semantic().border_default)
                .child(
                    div()
                        .id("qr-scan-drop-zone")
                        .rounded(px(8.0))
                        .bg(theme::semantic().bg_page)
                        .border_1()
                        .border_color(theme::semantic().border_default)
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
                                    panel.borrow_mut().scan_path_text(
                                        path.to_string_lossy().to_string(),
                                        cx.to_async(),
                                    );
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
        .bg(theme::semantic().bg_surface)
        .border_1()
        .border_color(theme::semantic().border_default)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(42.0))
                .px_3()
                .border_b_1()
                .border_color(theme::semantic().border_default)
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
                                .bg(theme::semantic().bg_subtle)
                                .text_size(px(11.0))
                                .text_color(theme::semantic().text_secondary)
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
                                        panel.borrow_mut().export_history(cx, cx.to_async());
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
                                        panel.borrow_mut().clear_history(cx.to_async());
                                        window.refresh();
                                    }
                                }),
                        )
                        .child(
                            action_button("关闭", dark)
                                .id("qr-history-close")
                                .on_click({
                                    let panel = Rc::clone(&panel);
                                    move |_, window, _cx| {
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
                .border_color(theme::semantic().border_default)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .rounded(px(8.0))
                                .bg(theme::semantic().bg_page)
                                .border_1()
                                .border_color(theme::semantic().border_default)
                                .child(history_query),
                        )
                        .child(
                            action_button("筛选", dark)
                                .id("qr-history-filter")
                                .on_click({
                                    let panel = Rc::clone(&panel);
                                    move |_, window, cx| {
                                        panel.borrow_mut().refresh_history(cx, cx.to_async());
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
                .text_color(theme::semantic().text_secondary)
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
                .on_click(move |_, window, _cx| {
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
        QrHistoryKind::Save => theme::semantic().success,
        QrHistoryKind::Copy => theme::semantic().primary_active,
        QrHistoryKind::Scan => theme::semantic().info,
    };

    div()
        .h(px(44.0))
        .px_3()
        .border_b_1()
        .border_color(theme::semantic().border_default)
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
                .text_color(theme::semantic().text_primary)
                .child(item.content.clone()),
        )
        .child(
            div()
                .w(px(76.0))
                .text_size(px(11.0))
                .text_color(theme::semantic().text_secondary)
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
                        panel
                            .borrow_mut()
                            .remove_history_item(&item_id, cx.to_async());
                        window.refresh();
                    }
                }),
        )
}

fn status_bar(message: String, tone: StatusTone, _dark: bool) -> impl IntoElement {
    div()
        .h(px(32.0))
        .rounded(px(10.0))
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.7))
        .border_1()
        .border_color(ui::border_light())
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .text_color(match tone {
                    StatusTone::Neutral => ui::text_secondary(),
                    StatusTone::Success => theme::semantic().success,
                    StatusTone::Error => theme::semantic().danger,
                })
                .child(message),
        )
}

fn section_label(label: &str, _dark: bool) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .text_color(ui::text_secondary())
        .child(label.to_string())
}

fn primary_action_button(label: &str, _dark: bool) -> gpui::Div {
    components::button(
        label.to_string(),
        components::ButtonVariant::Primary,
        Some(crate::core::plugin_spec::PluginAccent::Blue),
        _dark,
    )
}

fn action_button(label: &str, dark: bool) -> gpui::Div {
    components::button(
        label.to_string(),
        components::ButtonVariant::Secondary,
        None,
        dark,
    )
}

fn utility_button(label: &str, active: bool, _dark: bool) -> gpui::Div {
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
            theme::rgba_with_alpha(theme::semantic().bg_surface, 0.8)
        })
        .border_1()
        .border_color(ui::border_light())
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(if active {
            ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue)
        } else {
            theme::semantic().text_primary
        })
        .child(label.to_string())
}

fn icon_button(active: bool, _dark: bool, count: usize) -> gpui::Div {
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
            theme::rgba_with_alpha(theme::semantic().bg_surface, 0.8)
        })
        .border_1()
        .border_color(ui::border_light())
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
                theme::semantic().text_primary
            },
            13.0,
        ))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(if active {
                    ui::accent_color(crate::core::plugin_spec::PluginAccent::Blue)
                } else {
                    theme::semantic().text_primary
                })
                .child(count.to_string()),
        )
}

fn module_title(title: &str, tag: &str, _dark: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_primary)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(ui::text_secondary())
                .child(tag.to_string()),
        )
}

fn ghost_button(label: &str, dark: bool) -> gpui::Div {
    components::button(
        label.to_string(),
        components::ButtonVariant::Ghost,
        None,
        dark,
    )
}
