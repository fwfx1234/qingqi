use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use gpui::{
    App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ObjectFit, ParentElement, Render, Styled, StyledImage, Subscription, Window, div, hsla, img,
    px,
};

use crate::service::QrCodeService;
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    theme::Theme,
};
use qingqi_plugin::storage::AppPaths;
use qingqi_ui::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusTone {
    Neutral,
    Success,
    Error,
}

pub struct QrView {
    input: Option<Entity<InputState>>,
    service: QrCodeService,
    qr_matrix: Vec<bool>,
    qr_size: usize,
    message: String,
    tone: StatusTone,
    scanned_image_path: Option<PathBuf>,
    pending_action: Arc<Mutex<Vec<QrBackgroundResult>>>,
    input_snapshot: String,
    preview_generation: u64,
    subscriptions: Vec<Subscription>,
}

enum QrBackgroundResult {
    Preview {
        generation: u64,
        result: std::result::Result<crate::service::QrMatrix, String>,
    },
    Save {
        result: std::result::Result<PathBuf, String>,
    },
    Scan(std::result::Result<(String, PathBuf), String>),
    RecordCopy {
        success_message: String,
        result: std::result::Result<(), String>,
    },
}

impl QrView {
    pub fn new(paths: AppPaths) -> Result<Self> {
        let service = QrCodeService::new(paths)?;
        Ok(Self {
            input: None,
            service,
            qr_matrix: Vec::new(),
            qr_size: 0,
            message: String::from("输入文本后点击生成"),
            tone: StatusTone::Neutral,
            scanned_image_path: None,
            pending_action: Arc::new(Mutex::new(Vec::new())),
            input_snapshot: String::new(),
            preview_generation: 0,
            subscriptions: Vec::new(),
        })
    }

    pub fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.is_none() {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("输入文本或粘贴图片...")
                    .multi_line(true)
            });
            // 自动聚焦输入框，确保 dispatch path 包含本视图，
            // 从而使 capture_key_down 能响应 Cmd+V 粘贴图片
            window.focus(&input.read(cx).focus_handle(cx));
            self.input = Some(input);
        }
        self.observe_input(cx);
    }

    fn observe_input(&mut self, cx: &mut Context<Self>) {
        if !self.subscriptions.is_empty() {
            return;
        }
        let Some(input) = self.input.clone() else {
            return;
        };
        let subscription = cx.observe(&input, |this, _, cx| {
            this.sync_from_input(cx);
        });
        self.subscriptions.push(subscription);
    }

    fn input_text(&self, cx: &App) -> String {
        self.input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default()
    }

    pub fn set_input_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        self.input_snapshot = text.clone();
        if let Some(input) = self.input.as_ref() {
            input.update(cx, |input, cx| input.reset_value(text, cx));
        }
    }

    fn sync_from_input(&mut self, cx: &mut Context<Self>) {
        let text = self.input_text(cx);
        if text == self.input_snapshot {
            return;
        }

        self.input_snapshot = text.clone();
        self.scanned_image_path = None;
        if text.trim().is_empty() {
            self.invalidate_preview();
            self.qr_matrix.clear();
            self.qr_size = 0;
            self.message = "输入文本后生成二维码".into();
            self.tone = StatusTone::Neutral;
            cx.notify();
            return;
        }

        self.generate_from_text(&text, cx);
    }

    pub fn set_launch_input(&mut self, text: &str, cx: &mut Context<Self>) {
        self.set_input_text(text, cx);
        if !text.trim().is_empty() {
            self.generate_from_text(text, cx);
        }
    }

    pub fn generate_from_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input_snapshot = text.to_string();
        let generation = self.next_preview_generation();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.qr_matrix.clear();
            self.qr_size = 0;
            self.scanned_image_path = None;
            self.message = "请先输入文本".into();
            self.tone = StatusTone::Error;
            cx.notify();
            return;
        }
        self.scanned_image_path = None;
        let service = self.service.clone();
        let text = trimmed.to_string();
        self.message = "正在生成...".into();
        self.tone = StatusTone::Neutral;
        let pending = Arc::clone(&self.pending_action);
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move { service.preview(&text).map_err(|e| e.to_string()) })
                .await;
            if let Ok(mut s) = pending.lock() {
                s.push(QrBackgroundResult::Preview {
                    generation,
                    result: r,
                });
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
        cx.notify();
    }

    pub fn save_current(&mut self, cx: &mut Context<Self>) {
        let text = self.input_text(cx);
        if text.trim().is_empty() {
            self.message = "无可保存内容".into();
            self.tone = StatusTone::Error;
            return;
        }
        let dir = match qingqi_platform::shell::choose_directory("选择保存位置") {
            Ok(Some(p)) => p,
            Ok(None) => {
                self.message = "已取消".into();
                return;
            }
            Err(e) => {
                self.message = format!("{e}");
                self.tone = StatusTone::Error;
                return;
            }
        };
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        let save_text = text.clone();
        self.message = "正在保存...".into();
        self.tone = StatusTone::Neutral;
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move {
                    service
                        .save_to_dir(&save_text, &dir)
                        .map_err(|e| e.to_string())
                })
                .await;
            if let Ok(mut s) = pending.lock() {
                s.push(QrBackgroundResult::Save { result: r });
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    pub fn copy_current(&mut self, cx: &mut Context<Self>) {
        let text = self.input_text(cx);
        if text.trim().is_empty() {
            self.message = "无可复制内容".into();
            self.tone = StatusTone::Error;
            return;
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
        self.message = "已复制".into();
        self.tone = StatusTone::Success;
        self.record_copy(text, cx);
    }

    pub fn fill_from_clipboard(&mut self, cx: &mut Context<Self>) {
        if let Some(image) = qingqi_platform::clipboard::read_image(cx) {
            self.scan_clipboard_image(image, cx);
            return;
        }

        let text = qingqi_platform::clipboard::read_text(cx).unwrap_or_default();
        if text.trim().is_empty() {
            self.message = "剪贴板无可用内容".into();
            self.tone = StatusTone::Error;
            return;
        }
        self.set_input_text(text.clone(), cx);
        self.generate_from_text(&text, cx);
    }

    fn scan_clipboard_image(
        &mut self,
        image: qingqi_platform::clipboard::ClipboardImage,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_preview();
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        self.message = "正在识别...".into();
        self.tone = StatusTone::Neutral;
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move {
                    let tmp = std::env::temp_dir().join(format!(
                        "qr_{}_{}.{}",
                        std::process::id(),
                        image.id,
                        qingqi_platform::clipboard::image_format_extension(image.format)
                    ));
                    std::fs::write(&tmp, &image.bytes).map_err(|e| format!("{e}"))?;
                    let text = service.scan_image(&tmp).map_err(|e| format!("{e}"))?;
                    Ok((text, tmp))
                })
                .await;
            if let Ok(mut s) = pending.lock() {
                s.push(QrBackgroundResult::Scan(r));
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
        cx.notify();
    }

    pub fn choose_scan_image(&mut self, cx: &mut Context<Self>) {
        self.invalidate_preview();
        cx.spawn(async move |this, cx| {
            let result = rfd::AsyncFileDialog::new()
                .set_title("选择二维码图片")
                .pick_file()
                .await
                .map(|file| file.path().to_path_buf());
            let _ = this.update(cx, |view, cx| {
                view.handle_scan_image_selection(Ok(result), cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn handle_scan_image_selection(
        &mut self,
        result: anyhow::Result<Option<PathBuf>>,
        cx: &mut Context<Self>,
    ) {
        let path = match result {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.message = "已取消".into();
                self.tone = StatusTone::Neutral;
                cx.notify();
                return;
            }
            Err(error) => {
                self.message = format!("{error}");
                self.tone = StatusTone::Error;
                cx.notify();
                return;
            }
        };
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        self.message = "正在识别...".into();
        self.tone = StatusTone::Neutral;
        cx.spawn(async move |this, cx| {
            let scan_path = path.clone();
            let r = cx
                .background_executor()
                .spawn(async move {
                    let text = service
                        .scan_image(&scan_path)
                        .map_err(|error| format!("{error}"))?;
                    Ok((text, scan_path))
                })
                .await;
            if let Ok(mut s) = pending.lock() {
                s.push(QrBackgroundResult::Scan(r));
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    pub fn clear_input(&mut self, cx: &mut Context<Self>) {
        self.input_snapshot.clear();
        if let Some(i) = self.input.as_ref() {
            i.update(cx, |i, cx| i.reset_value("", cx));
        }
        self.invalidate_preview();
        self.qr_matrix.clear();
        self.qr_size = 0;
        self.scanned_image_path = None;
        self.message = "已清空".into();
        self.tone = StatusTone::Neutral;
    }

    fn record_copy(&mut self, text: String, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let pending = Arc::clone(&self.pending_action);
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move {
                    service
                        .record_copy(&text)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                })
                .await;
            if let Ok(mut s) = pending.lock() {
                s.push(QrBackgroundResult::RecordCopy {
                    success_message: "已复制".into(),
                    result: r,
                });
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    fn collect_pending(&mut self, cx: &mut Context<Self>) {
        let actions = self
            .pending_action
            .lock()
            .map(|mut s| std::mem::take(&mut *s))
            .unwrap_or_default();

        for action in actions {
            match action {
                QrBackgroundResult::Preview { generation, result } => {
                    if generation != self.preview_generation {
                        continue;
                    }
                    match result {
                        Ok(m) => {
                            self.qr_size = m.size;
                            self.qr_matrix = m.cells;
                            self.message = format!("已生成 ({}x{})", m.size, m.size);
                            self.tone = StatusTone::Success;
                        }
                        Err(e) => {
                            self.qr_matrix.clear();
                            self.qr_size = 0;
                            self.message = e;
                            self.tone = StatusTone::Error;
                        }
                    }
                }
                QrBackgroundResult::Save { result } => match result {
                    Ok(p) => {
                        self.message = format!("已保存: {}", p.display());
                        self.tone = StatusTone::Success;
                    }
                    Err(e) => {
                        self.message = e;
                        self.tone = StatusTone::Error;
                    }
                },
                QrBackgroundResult::Scan(r) => match r {
                    Ok((text, path)) => {
                        self.set_input_text(text, cx);
                        self.scanned_image_path = Some(path);
                        self.qr_matrix.clear();
                        self.qr_size = 0;
                        self.message = "已识别".into();
                        self.tone = StatusTone::Success;
                    }
                    Err(e) => {
                        self.message = e;
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
                    }
                    Err(e) => {
                        self.message = e;
                        self.tone = StatusTone::Error;
                    }
                },
            }
        }
    }

    fn next_preview_generation(&mut self) -> u64 {
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.preview_generation
    }

    fn invalidate_preview(&mut self) {
        self.preview_generation = self.preview_generation.wrapping_add(1);
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        let primary = modifiers.platform || modifiers.control;
        if !primary || !event.keystroke.key.eq_ignore_ascii_case("v") {
            return;
        }

        if let Some(image) = qingqi_platform::clipboard::read_image(cx) {
            self.scan_clipboard_image(image, cx);
            cx.stop_propagation();
        }
    }

    pub fn clear_view_state(&mut self) {
        self.input_snapshot.clear();
        self.invalidate_preview();
        self.qr_matrix.clear();
        self.qr_size = 0;
        self.scanned_image_path = None;
        self.message = "输入文本后点击生成".into();
        self.tone = StatusTone::Neutral;
    }
}

impl Render for QrView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.collect_pending(cx);
        self.ensure_inputs(window, cx);

        let entity = cx.entity();
        let t = Theme::global(cx);
        let input = self.input.clone().expect("qr input missing");
        let qr_matrix = self.qr_matrix.clone();
        let qr_size = self.qr_size;
        let message = self.message.clone();
        let tone = self.tone;
        let scanned = self.scanned_image_path.clone();

        ui::plugin_surface(cx).child(
            ui::plugin_scroll_content()
                .capture_key_down(cx.listener(Self::handle_key_down))
                .flex()
                .flex_col()
                .gap_2()
                // Header
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(t.foreground)
                                .child("二维码"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("qr-choose-img")
                                        .label("选择图片")
                                        .small()
                                        .on_click({
                                            let e = entity.clone();
                                            move |_, _, cx| {
                                                e.update(cx, |t, cx| {
                                                    t.choose_scan_image(cx);
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("qr-clear")
                                        .label("清空")
                                        .small()
                                        .ghost()
                                        .on_click({
                                            let e = entity.clone();
                                            move |_, _, cx| {
                                                e.update(cx, |t, cx| {
                                                    t.clear_input(cx);
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        ),
                )
                // Content: input (left) | preview (right)
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
                                .child(
                                    div()
                                        .rounded(px(10.0))
                                        .bg(t.list)
                                        .border_1()
                                        .border_color(ui::border_light(cx))
                                        .overflow_hidden()
                                        .child(qr_input(input)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new("qr-save")
                                                .label("另存为")
                                                .small()
                                                .primary()
                                                .on_click({
                                                    let e = entity.clone();
                                                    move |_, _, cx| {
                                                        e.update(cx, |t, cx| {
                                                            t.save_current(cx);
                                                            cx.notify();
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new("qr-copy")
                                                .label("复制")
                                                .small()
                                                .on_click({
                                                    let e = entity.clone();
                                                    move |_, _, cx| {
                                                        e.update(cx, |t, cx| {
                                                            t.copy_current(cx);
                                                            cx.notify();
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new("qr-paste")
                                                .label("粘贴")
                                                .small()
                                                .on_click({
                                                    let e = entity.clone();
                                                    move |_, _, cx| {
                                                        e.update(cx, |t, cx| {
                                                            t.fill_from_clipboard(cx);
                                                            cx.notify();
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(div().flex_1())
                                        .child(
                                            Button::new("qr-gen")
                                                .label("生成")
                                                .small()
                                                .ghost()
                                                .on_click({
                                                    let e = entity.clone();
                                                    move |_, _, cx| {
                                                        e.update(cx, |t, cx| {
                                                            let text = t.input_text(cx);
                                                            t.generate_from_text(&text, cx);
                                                            cx.notify();
                                                        });
                                                    }
                                                }),
                                        ),
                                )
                                .child(status_bar(message, tone, cx)),
                        )
                        .child(preview_panel(qr_matrix, qr_size, scanned, cx)),
                ),
        )
    }
}

fn preview_panel(
    qr_matrix: Vec<bool>,
    qr_size: usize,
    scanned: Option<PathBuf>,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    let dark = t.mode.is_dark();
    let preview_size = 200.0;
    div().w(px(preview_size)).flex_none().child(
        div()
            .size(px(preview_size))
            .rounded(px(10.0))
            .bg(t.list)
            .border_1()
            .border_color(ui::border_light(cx))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .child({
                if let Some(path) = scanned {
                    img(path)
                        .object_fit(ObjectFit::Contain)
                        .size_full()
                        .into_any_element()
                } else if !qr_matrix.is_empty() && qr_size > 0 {
                    let cell_size = ((preview_size - 24.0) / qr_size as f32).floor().max(2.0);
                    let total = qr_size as f32 * cell_size;
                    let dark_c = if dark {
                        hsla(0.0, 0.0, 0.92, 1.0)
                    } else {
                        hsla(0.0, 0.0, 0.0, 1.0)
                    };
                    let light_c = if dark {
                        hsla(0.0, 0.0, 0.18, 1.0)
                    } else {
                        hsla(0.0, 0.0, 1.0, 1.0)
                    };
                    div()
                        .rounded(px(8.0))
                        .bg(light_c)
                        .p(px(12.0))
                        .child(
                            div()
                                .size(px(total))
                                .flex()
                                .flex_col()
                                .children((0..qr_size).map(|row| {
                                    div().flex().children((0..qr_size).map(|col| {
                                        let filled = qr_matrix
                                            .get(row * qr_size + col)
                                            .copied()
                                            .unwrap_or(false);
                                        div().size(px(cell_size)).bg(if filled {
                                            dark_c
                                        } else {
                                            light_c
                                        })
                                    }))
                                })),
                        )
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::text_tertiary(cx))
                        .child("预览")
                        .into_any_element()
                }
            }),
    )
}

fn status_bar(message: String, tone: StatusTone, cx: &App) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .h(px(28.0))
        .rounded(px(8.0))
        .bg(t.list)
        .border_1()
        .border_color(ui::border_light(cx))
        .px_3()
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .text_color(match tone {
                    StatusTone::Neutral => ui::text_secondary(cx),
                    StatusTone::Success => t.success,
                    StatusTone::Error => t.danger,
                })
                .child(message),
        )
}

fn qr_input(state: Entity<InputState>) -> Input {
    Input::new(&state)
        .appearance(false)
        .bordered(false)
        .focus_bordered(false)
        .h(px(200.0))
        .text_size(px(12.0))
}


