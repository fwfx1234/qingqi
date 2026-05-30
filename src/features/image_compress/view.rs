use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::Context;

use gpui::{
    App, Component, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, img, px,
};

use crate::{
    app::{
        theme,
        ui::{self, components},
    },
    core::plugin_spec::PluginAccent,
    platform,
};

use super::service::{
    CompressionMode, CompressionResult, ImageCompressService, ImportedImage, QueueStatus,
};

/// Result of a single file compression, written by the background worker.
#[derive(Clone, Debug)]
struct BatchResultItem {
    index: usize,
    result: Result<CompressionResult, String>,
}

/// Shared state between the background compression worker and the UI panel.
#[derive(Default)]
struct SharedBatchState {
    results: Vec<BatchResultItem>,
    message: Option<String>,
    /// Set by the worker when all items have been processed (success or failure).
    batch_done: bool,
    single_results: Vec<SingleActionResult>,
}

/// Thread-safe handle for the background worker to report results back to the UI.
#[derive(Clone, Default)]
struct SharedBatchResults {
    inner: Arc<Mutex<SharedBatchState>>,
    cancel_requested: Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
enum SingleActionKind {
    Retry,
    Overwrite,
}

#[derive(Clone, Debug)]
struct SingleActionResult {
    index: usize,
    kind: SingleActionKind,
    result: Result<Option<CompressionResult>, String>,
}

const THUMB_SIZE: f32 = 42.0;

#[derive(Clone, Debug)]
struct QueueItem {
    source: ImportedImage,
    status: QueueStatus,
    output_size: Option<u64>,
    output_path: Option<PathBuf>,
    reduction_ratio: Option<f32>,
    error_message: String,
    from_clipboard: bool,
}

pub struct ImageCompressPanel {
    service: ImageCompressService,
    items: Vec<QueueItem>,
    mode: CompressionMode,
    quality: u8,
    overwrite_original: bool,
    output_dir: PathBuf,
    message: String,
    /// Whether a batch compression is currently running in the background.
    running: bool,
    /// Number of items in the current batch (set when run_compression starts).
    batch_total: usize,
    /// Shared state handle for the background worker to report per-file results.
    shared: SharedBatchResults,
    /// Foreground drain task — periodically refreshes UI while batch is running.
    drain_task: Option<gpui::Task<()>>,
}

impl ImageCompressPanel {
    pub fn new(service: ImageCompressService) -> Self {
        let output_dir = service.default_output_dir().to_path_buf();
        Self {
            service,
            items: Vec::new(),
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir,
            message: String::from("导入图片后即可开始压缩"),
            running: false,
            batch_total: 0,
            shared: SharedBatchResults::default(),
            drain_task: None,
        }
    }

    pub fn clear_transient_state(&mut self) {
        self.message = String::from("导入图片后即可开始压缩");
    }

    fn pending_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == QueueStatus::Pending)
            .count()
    }

    fn running_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == QueueStatus::Running)
            .count()
    }

    fn batch_completed(&self) -> usize {
        self.batch_total.saturating_sub(self.running_count())
    }

    fn success_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == QueueStatus::Success)
            .count()
    }

    fn average_ratio(&self) -> Option<f32> {
        let ratios = self
            .items
            .iter()
            .filter_map(|item| item.reduction_ratio)
            .collect::<Vec<_>>();
        if ratios.is_empty() {
            None
        } else {
            Some(ratios.iter().sum::<f32>() / ratios.len() as f32)
        }
    }

    fn import_paths(&mut self, paths: Vec<PathBuf>, from_clipboard: bool) {
        let mut imported_count = 0usize;
        let mut last_error = String::new();

        for path in paths {
            if self
                .items
                .iter()
                .any(|item| item.source.path.as_path() == path.as_path())
            {
                continue;
            }

            match self.service.import_path(&path) {
                Ok(source) => {
                    self.items.push(QueueItem {
                        source,
                        status: QueueStatus::Pending,
                        output_size: None,
                        output_path: None,
                        reduction_ratio: None,
                        error_message: String::new(),
                        from_clipboard,
                    });
                    imported_count += 1;
                }
                Err(error) => last_error = error.to_string(),
            }
        }

        self.message = if imported_count > 0 {
            format!("已导入 {imported_count} 张图片")
        } else if !last_error.is_empty() {
            format!("导入失败: {last_error}")
        } else {
            String::from("没有新的可导入图片")
        };
    }

    pub fn choose_images(&mut self) {
        match platform::shell::choose_file("选择一张图片导入") {
            Ok(Some(path)) => self.import_paths(vec![path], false),
            Ok(None) => self.message = String::from("已取消选择图片"),
            Err(error) => self.message = format!("打开文件选择失败: {error}"),
        }
    }

    pub fn paste_from_clipboard(&mut self, cx: &App) {
        // 1) Try reading actual image data from clipboard
        if let Some(clipboard_image) = platform::clipboard::read_image(cx) {
            let ext = platform::clipboard::image_format_extension(clipboard_image.format);
            match self
                .service
                .materialize_clipboard_image(&clipboard_image.bytes, ext)
            {
                Ok(imported) => {
                    let already = self
                        .items
                        .iter()
                        .any(|item| item.source.path.as_path() == imported.path.as_path());
                    if !already {
                        self.items.push(QueueItem {
                            source: imported,
                            status: QueueStatus::Pending,
                            output_size: None,
                            output_path: None,
                            reduction_ratio: None,
                            error_message: String::new(),
                            from_clipboard: true,
                        });
                    }
                    self.message = String::from("已读取剪贴板图片，点击「开始压缩」");
                    return;
                }
                Err(error) => {
                    self.message = format!("读取剪贴板图片失败: {error}");
                    return;
                }
            }
        }

        // 2) Fallback: treat clipboard text as file paths
        let text = platform::clipboard::read_text(cx).unwrap_or_default();
        if text.trim().is_empty() {
            self.message = String::from("剪贴板里没有图片，请先复制一张图片");
            return;
        }

        let paths = image_paths_from_input(&text);
        if paths.is_empty() {
            self.message = String::from("剪贴板内容不是图片路径");
            return;
        }

        self.import_paths(paths, false);
    }

    pub fn import_from_launch_input(&mut self, text: &str) {
        let paths = image_paths_from_input(text);
        if !paths.is_empty() {
            self.import_paths(paths, false);
        }
    }

    pub fn choose_output_dir(&mut self) {
        match platform::shell::choose_directory("选择压缩输出目录") {
            Ok(Some(path)) => {
                self.output_dir = path.clone();
                self.message = format!("输出目录已更新: {}", path.display());
            }
            Ok(None) => self.message = String::from("已取消选择目录"),
            Err(error) => self.message = format!("选择目录失败: {error}"),
        }
    }

    pub fn open_output_dir(&mut self) {
        let dir = self.output_dir.clone();
        thread::spawn(move || {
            let _ = std::fs::create_dir_all(&dir);
            let _ = platform::shell::open_path(&dir);
        });
        self.message = format!("正在打开目录: {}", self.output_dir.display());
    }

    pub fn clear_items(&mut self) {
        if self.running {
            self.message = String::from("压缩进行中，无法清空");
            return;
        }
        self.items.clear();
        self.message = String::from("已清空队列");
    }

    pub fn remove_item(&mut self, index: usize) {
        if self.running {
            self.message = String::from("压缩进行中，无法移除");
            return;
        }
        if index < self.items.len() {
            self.items.remove(index);
            self.message = String::from("已移除图片");
        }
    }

    pub fn retry_entry_background(&mut self, index: usize, async_cx: gpui::AsyncApp) {
        let (source_path, output_path_opt) = match self.items.get(index) {
            Some(item) if item.status == QueueStatus::Failed => {
                (item.source.path.clone(), item.output_path.clone())
            }
            _ => {
                self.message = String::from("只有失败的条目可以重试");
                return;
            }
        };

        if let Some(item) = self.items.get_mut(index) {
            item.status = QueueStatus::Running;
            item.error_message.clear();
        }
        self.message = String::from("正在重试...");

        let service = self.service.clone_for_background();
        let output_dir = self.output_dir.clone();
        let mode = self.mode;
        let quality = self.quality;
        let overwrite_original = self.overwrite_original;
        let shared = self.shared.clone();

        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        if let Some(output_path) = &output_path_opt {
                            let _ = std::fs::remove_file(output_path);
                        }
                        service
                            .retry_entry(
                                &output_dir,
                                mode,
                                quality,
                                overwrite_original,
                                &source_path,
                            )
                            .map(Some)
                            .map_err(|error| error.to_string())
                    })
                    .await;

                if let Ok(mut state) = shared.inner.lock() {
                    state.single_results.push(SingleActionResult {
                        index,
                        kind: SingleActionKind::Retry,
                        result,
                    });
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    pub fn overwrite_entry_background(&mut self, index: usize, async_cx: gpui::AsyncApp) {
        let (output, source) = match self.items.get(index) {
            Some(item) if !item.from_clipboard && !item.source.path.as_os_str().is_empty() => {
                match item.output_path.as_ref() {
                    Some(output) => (output.clone(), item.source.path.clone()),
                    None => {
                        self.message = String::from("无输出文件");
                        return;
                    }
                }
            }
            _ => {
                self.message = String::from("剪贴板图片没有源文件，无法覆盖");
                return;
            }
        };

        self.message = String::from("正在覆盖原图...");
        let shared = self.shared.clone();

        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move {
                        std::fs::copy(&output, &source)
                            .with_context(|| format!("无法覆盖原图 {}", source.display()))?;
                        let meta = std::fs::metadata(&source)
                            .with_context(|| format!("无法读取原图 {}", source.display()))?;
                        Ok::<_, anyhow::Error>(Some(CompressionResult {
                            output_path: output,
                            output_size: meta.len(),
                            reduction_ratio: 0.0,
                        }))
                    })
                    .await
                    .map_err(|error| error.to_string());

                if let Ok(mut state) = shared.inner.lock() {
                    state.single_results.push(SingleActionResult {
                        index,
                        kind: SingleActionKind::Overwrite,
                        result,
                    });
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }

    /// Reveal the output file in Finder (macOS).
    pub fn reveal_entry(&mut self, index: usize) {
        let output_path = match self.items.get(index).and_then(|i| i.output_path.as_ref()) {
            Some(p) => p.clone(),
            None => {
                self.message = String::from("无输出文件");
                return;
            }
        };
        match platform::shell::open_path(&output_path) {
            Ok(_) => self.message = format!("已打开 {}", output_path.display()),
            Err(error) => self.message = format!("打开失败: {error}"),
        }
    }

    pub fn set_mode(&mut self, mode: CompressionMode) {
        self.mode = mode;
        self.message = format!("模式已切换为 {}", mode.label());
    }

    pub fn adjust_quality(&mut self, delta: i16) {
        let next = (self.quality as i16 + delta).clamp(30, 100) as u8;
        self.quality = next;
        self.message = format!("压缩质量调整为 {}%", self.quality);
    }

    pub fn toggle_overwrite(&mut self) {
        self.overwrite_original = !self.overwrite_original;
        self.message = if self.overwrite_original {
            String::from("已切换为覆盖原图")
        } else {
            String::from("已切换为输出到单独目录")
        };
    }

    pub fn run_compression(&mut self, cx: &mut App) {
        if self.items.is_empty() {
            self.message = String::from("请先导入图片");
            return;
        }
        if self.running {
            self.message = String::from("压缩任务正在执行中");
            return;
        }

        let pending_indices: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.status == QueueStatus::Pending)
            .map(|(i, _)| i)
            .collect();

        if pending_indices.is_empty() {
            self.message = String::from("没有待压缩的图片");
            return;
        }

        // Snapshot what the worker needs — all cheap clones.
        let items_for_worker: Vec<(PathBuf, usize)> = pending_indices
            .iter()
            .map(|&i| (self.items[i].source.path.clone(), i))
            .collect();
        let output_dir = self.output_dir.clone();
        let mode = self.mode;
        let quality = self.quality;
        let overwrite_original = self.overwrite_original;
        let service = self.service.clone_for_background();
        let total_count = items_for_worker.len();

        // Mark queued items as Running.
        for &i in &pending_indices {
            self.items[i].status = QueueStatus::Running;
        }
        let batch_total = pending_indices.len();
        self.batch_total = batch_total;
        self.running = true;
        self.message = format!("正在压缩 0/{batch_total} 张…");

        // Fresh shared state for this batch.
        self.shared = SharedBatchResults::default();
        let shared = self.shared.clone();
        let cancel_flag = shared.cancel_requested.clone();

        // Spawn background worker on a real background thread.
        cx.background_executor()
            .spawn(async move {
                for (path, index) in &items_for_worker {
                    // Check cancel flag before each item.
                    if cancel_flag.load(Ordering::Relaxed) {
                        break;
                    }

                    let result =
                        service.compress_file(path, &output_dir, mode, quality, overwrite_original);

                    let batch_item = BatchResultItem {
                        index: *index,
                        result: result.map_err(|e| e.to_string()),
                    };

                    // Brief lock to store the per-file result.
                    if let Ok(mut state) = shared.inner.lock() {
                        state.results.push(batch_item);
                    }

                    // Yield the OS thread so the foreground drain can refresh the UI.
                    thread::yield_now();
                }

                let processed = shared.inner.lock().map(|s| s.results.len()).unwrap_or(0);
                let success = shared
                    .inner
                    .lock()
                    .map(|s| s.results.iter().filter(|r| r.result.is_ok()).count())
                    .unwrap_or(0);
                let cancelled = total_count - processed;
                let failed_errors = processed - success;

                let msg = if cancel_flag.load(Ordering::Relaxed) && cancelled > 0 {
                    format!("已取消，完成 {} 张，取消 {} 张", success, cancelled)
                } else if failed_errors == 0 {
                    format!("压缩完成，共处理 {total_count} 张")
                } else {
                    format!("压缩完成，成功 {success} 张，失败 {failed_errors} 张")
                };

                if let Ok(mut state) = shared.inner.lock() {
                    state.message = Some(msg);
                    state.batch_done = true;
                }
            })
            .detach();

        // Schedule a periodic foreground drain to refresh the UI during compression.
        let drain_shared = self.shared.clone();
        self.drain_task = Some(cx.spawn(async move |async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(100))
                    .await;

                let batch_done = drain_shared
                    .inner
                    .lock()
                    .map(|s| s.batch_done)
                    .unwrap_or(true);

                if batch_done {
                    break;
                }
            }
        }));
    }

    /// Drain per-file results from the background worker into panel items.
    /// Returns true if any results were applied.
    pub fn collect_results(&mut self) -> bool {
        let mut results = Vec::new();
        let mut batch_message = None;
        let mut single_results = Vec::new();

        if let Ok(mut state) = self.shared.inner.lock() {
            if !state.results.is_empty() {
                results = std::mem::take(&mut state.results);
            }
            if !state.single_results.is_empty() {
                single_results = std::mem::take(&mut state.single_results);
            }
            if state.batch_done {
                if let Some(ref msg) = state.message {
                    batch_message = Some(msg.clone());
                }
            }
        }

        if results.is_empty() && single_results.is_empty() && batch_message.is_none() {
            return false;
        }

        let has_results = !results.is_empty();
        for batch_item in &results {
            if let Some(item) = self.items.get_mut(batch_item.index) {
                match &batch_item.result {
                    Ok(result) => apply_result(item, result.clone()),
                    Err(error) => {
                        item.status = QueueStatus::Failed;
                        item.output_size = None;
                        item.output_path = None;
                        item.reduction_ratio = None;
                        item.error_message = error.clone();
                    }
                }
            }
        }

        for action in single_results {
            if let Some(item) = self.items.get_mut(action.index) {
                match action.kind {
                    SingleActionKind::Retry => match action.result {
                        Ok(Some(result)) => {
                            apply_result(item, result);
                            self.message = String::from("重试成功");
                        }
                        Ok(None) => {}
                        Err(error) => {
                            item.status = QueueStatus::Failed;
                            item.output_size = None;
                            item.output_path = None;
                            item.reduction_ratio = None;
                            item.error_message = error.clone();
                            self.message = format!("重试失败: {error}");
                        }
                    },
                    SingleActionKind::Overwrite => match action.result {
                        Ok(Some(result)) => {
                            item.source.original_size = result.output_size;
                            item.output_size = Some(result.output_size);
                            item.reduction_ratio = Some(result.reduction_ratio);
                            self.message = String::from("已覆盖原图");
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.message = format!("覆盖失败: {error}");
                        }
                    },
                }
            }
        }

        if let Some(msg) = batch_message {
            self.message = msg;
        } else if self.running && has_results && self.batch_total > 0 {
            let completed = self.batch_completed();
            self.message = format!("正在压缩 {}/{} 张…", completed, self.batch_total);
        }

        // Safety: if no items are still Running, ensure running flag is cleared.
        if self.running && !self.items.iter().any(|i| i.status == QueueStatus::Running) {
            self.running = false;
            self.batch_total = 0;
        }

        true
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Request cancellation of the current batch.
    /// Sets the atomic flag for the worker. Items currently Running are reverted to Pending.
    /// Items already processed by the worker (results in SharedBatchResults) will get their
    /// correct status override when collect_results applies the results on next render.
    /// The old worker holds its own Arc<SharedBatchResults>, so the worker's late writes
    /// are harmless — run_compression creates a fresh SharedBatchResults per batch.
    pub fn request_cancel(&mut self) {
        if !self.running {
            return;
        }
        self.shared.cancel_requested.store(true, Ordering::Relaxed);

        // Revert items that haven't started processing yet back to Pending.
        // Items that the worker already finished (results in shared.results) will
        // be corrected to Success/Failed by collect_results on the next render.
        let mut cancelled_count = 0usize;
        for item in &mut self.items {
            if item.status == QueueStatus::Running {
                item.status = QueueStatus::Pending;
                cancelled_count += 1;
            }
        }

        let completed = self.batch_completed();
        self.message = if cancelled_count > 0 {
            format!("正在取消… {completed} 张已完成，{cancelled_count} 张已回退")
        } else {
            format!("正在取消… {completed} 张已完成")
        };

        // Allow the user to start a new batch immediately.
        // The old worker holds its own Arc<SharedBatchResults> and writes results there.
        // run_compression creates a fresh SharedBatchResults per batch.
        self.running = false;
        self.batch_total = 0;
    }
}

fn apply_result(item: &mut QueueItem, result: CompressionResult) {
    item.status = QueueStatus::Success;
    item.output_size = Some(result.output_size);
    item.output_path = Some(result.output_path);
    item.reduction_ratio = Some(result.reduction_ratio);
    item.error_message.clear();
}

fn looks_like_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        })
        .unwrap_or(false)
}

fn image_paths_from_input(text: &str) -> Vec<PathBuf> {
    text.lines()
        .flat_map(|line| line.split('\0'))
        .map(normalize_image_input_path)
        .filter(|path| looks_like_image_path(path))
        .collect()
}

fn normalize_image_input_path(value: &str) -> PathBuf {
    let mut path = value.trim().trim_matches(&['"', '\''][..]).to_string();
    if let Some(rest) = path.strip_prefix("file://") {
        path = if let Some(local) = rest.strip_prefix("localhost/") {
            format!("/{local}")
        } else if rest.starts_with('/') {
            rest.to_string()
        } else {
            format!("/{rest}")
        };
    }

    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }

    PathBuf::from(path)
}

pub struct ImageCompressElement {
    pub panel: Rc<RefCell<ImageCompressPanel>>,
}

impl IntoElement for ImageCompressElement {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for ImageCompressElement {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Drain background worker results before rendering.
        self.panel.borrow_mut().collect_results();

        let panel = self.panel.borrow();
        let dark = crate::app::theme_mode::is_dark();
        let items = panel.items.clone();
        let mode = panel.mode;
        let quality = panel.quality;
        let overwrite_original = panel.overwrite_original;
        let message = panel.message.clone();
        let pending_count = panel.pending_count();
        let running_count = panel.running_count();
        let running = panel.is_running();
        let average_ratio = panel.average_ratio();
        let success_count = panel.success_count();
        let output_dir = panel.output_dir.display().to_string();
        let batch_total = panel.batch_total;
        let batch_completed = panel.batch_completed();
        drop(panel);

        ui::plugin_surface(dark).child(
            ui::plugin_content().child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(section_title("🖼 图片压缩", "Image Compress", dark))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(ui::text_secondary())
                                            .child("PNG / JPEG / WebP · 批量处理"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        mode_chip(
                                            "视觉无损",
                                            mode == CompressionMode::VisuallyLossless,
                                            dark,
                                        )
                                        .id("image-compress-mode-lossless")
                                        .on_click({
                                            let panel = Rc::clone(&self.panel);
                                            move |_, window, _cx| {
                                                panel
                                                    .borrow_mut()
                                                    .set_mode(CompressionMode::VisuallyLossless);
                                                window.refresh();
                                            }
                                        }),
                                    )
                                    .child(
                                        mode_chip(
                                            "普通压缩",
                                            mode == CompressionMode::Standard,
                                            dark,
                                        )
                                        .id("image-compress-mode-standard")
                                        .on_click({
                                            let panel = Rc::clone(&self.panel);
                                            move |_, window, _cx| {
                                                panel
                                                    .borrow_mut()
                                                    .set_mode(CompressionMode::Standard);
                                                window.refresh();
                                            }
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family("SF Mono")
                                            .text_color(theme::semantic(dark).text_regular)
                                            .child(format!("{quality}%")),
                                    )
                                    .child(
                                        quality_button("−", dark)
                                            .id("image-compress-quality-down")
                                            .on_click({
                                                let panel = Rc::clone(&self.panel);
                                                move |_, window, _cx| {
                                                    panel.borrow_mut().adjust_quality(-5);
                                                    window.refresh();
                                                }
                                            }),
                                    )
                                    .child(
                                        quality_button("+", dark)
                                            .id("image-compress-quality-up")
                                            .on_click({
                                                let panel = Rc::clone(&self.panel);
                                                move |_, window, _cx| {
                                                    panel.borrow_mut().adjust_quality(5);
                                                    window.refresh();
                                                }
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        drop_zone(dark, pending_count)
                            .child(
                                primary_button("📋 粘贴剪贴板", PluginAccent::Amber, dark)
                                    .id("image-compress-paste")
                                    .on_click({
                                        let panel = Rc::clone(&self.panel);
                                        move |_, window, cx| {
                                            panel.borrow_mut().paste_from_clipboard(cx);
                                            window.refresh();
                                        }
                                    }),
                            )
                            .child(
                                secondary_button("📂 选择图片", dark)
                                    .id("image-compress-choose")
                                    .on_click({
                                        let panel = Rc::clone(&self.panel);
                                        move |_, window, _cx| {
                                            panel.borrow_mut().choose_images();
                                            window.refresh();
                                        }
                                    }),
                            ),
                    )
                    .child(image_table(items, dark, Rc::clone(&self.panel)))
                    .child(footer_bar(
                        dark,
                        message,
                        output_dir,
                        overwrite_original,
                        pending_count,
                        running_count,
                        success_count,
                        average_ratio,
                        running,
                        batch_total,
                        batch_completed,
                        Rc::clone(&self.panel),
                    )),
            ),
        )
    }
}

fn section_title(title: &str, tag: &str, dark: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic(dark).text_primary)
                .child(title.to_string()),
        )
        .child(
            div()
                .px_2()
                .h(px(20.0))
                .rounded(px(999.0))
                .bg(theme::rgba_with_alpha(
                    ui::accent_color(PluginAccent::Amber),
                    0.12,
                ))
                .flex()
                .items_center()
                .text_size(px(10.0))
                .text_color(ui::accent_color(PluginAccent::Amber))
                .child(tag.to_string()),
        )
}

fn mode_chip(label: &str, active: bool, dark: bool) -> gpui::Div {
    div()
        .h(px(28.0))
        .px_3()
        .rounded(px(8.0))
        .bg(if active {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Amber), 0.12)
        } else {
            theme::rgba_with_alpha(theme::semantic(dark).bg_surface, 0.82)
        })
        .border_1()
        .border_color(if active {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Amber), 0.25)
        } else {
            ui::border_light()
        })
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(if active {
            ui::accent_color(PluginAccent::Amber)
        } else {
            ui::text_secondary()
        })
        .child(label.to_string())
}

fn quality_button(label: &str, dark: bool) -> gpui::Div {
    div()
        .size(px(26.0))
        .rounded(px(8.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic(dark).bg_surface,
            0.88,
        ))
        .border_1()
        .border_color(ui::border_light())
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::semantic(dark).text_primary)
        .child(label.to_string())
}

fn drop_zone(dark: bool, pending_count: usize) -> gpui::Div {
    div()
        .rounded(px(12.0))
        .bg(theme::rgba_with_alpha(
            ui::accent_color(PluginAccent::Amber),
            0.05,
        ))
        .border_1()
        .border_color(theme::rgba_with_alpha(
            ui::accent_color(PluginAccent::Amber),
            0.18,
        ))
        .p_3()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(42.0))
                .rounded(px(12.0))
                .bg(theme::rgba_with_alpha(
                    ui::accent_color(PluginAccent::Amber),
                    0.12,
                ))
                .flex()
                .items_center()
                .justify_center()
                .child(ui::icon_element("folder.svg", ui::text_secondary(), 20.0)),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme::semantic(dark).text_primary)
                        .child("粘贴剪贴板或拖入图片"),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(ui::text_tertiary())
                        .child("支持 PNG, JPG, WebP · 批量处理"),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::semantic(dark).warning)
                .child(format!("待压缩 {pending_count} 张")),
        )
}

fn image_table(
    items: Vec<QueueItem>,
    dark: bool,
    panel: Rc<RefCell<ImageCompressPanel>>,
) -> impl IntoElement {
    let content = if items.is_empty() {
        div()
            .flex_1()
            .rounded(px(12.0))
            .bg(theme::rgba_with_alpha(
                theme::semantic(dark).bg_surface,
                0.74,
            ))
            .border_1()
            .border_color(ui::border_light())
            .child(ui::ui_empty_state("还没有图片，先导入一张试试", dark))
            .into_any_element()
    } else {
        div()
            .flex_1()
            .rounded(px(12.0))
            .bg(theme::rgba_with_alpha(
                theme::semantic(dark).bg_surface,
                0.78,
            ))
            .border_1()
            .border_color(ui::border_light())
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(34.0))
                    .px_3()
                    .bg(theme::rgba_with_alpha(
                        theme::semantic(dark).bg_subtle,
                        0.65,
                    ))
                    .border_b_1()
                    .border_color(ui::border_light())
                    .flex()
                    .items_center()
                    .text_size(px(10.0))
                    .text_color(ui::text_tertiary())
                    .child(components::table_header_cell("预览", 76.0))
                    .child(components::table_header_flex("文件名", 2.4))
                    .child(components::table_header_flex("原始大小", 1.0))
                    .child(components::table_header_flex("压缩后", 1.0))
                    .child(components::table_header_flex("状态", 1.0))
                    .child(components::table_header_cell("", 30.0)),
            )
            .child(
                div()
                    .id("image-compress-table-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .scrollbar_width(px(6.0))
                    .children(
                        items
                            .into_iter()
                            .enumerate()
                            .map(|(index, item)| image_row(item, index, dark, Rc::clone(&panel))),
                    ),
            )
            .into_any_element()
    };

    content
}

fn image_row(
    item: QueueItem,
    index: usize,
    dark: bool,
    panel: Rc<RefCell<ImageCompressPanel>>,
) -> impl IntoElement {
    let from_clipboard = item.from_clipboard;
    let is_success = item.status == QueueStatus::Success;
    let is_failed = item.status == QueueStatus::Failed;
    let is_running = item.status == QueueStatus::Running;
    let has_source = !item.source.path.as_os_str().is_empty();

    div()
        .h(px(58.0))
        .px_3()
        .border_b_1()
        .border_color(ui::border_light())
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(60.0))
                .flex()
                .justify_center()
                .child(thumbnail(&item.source.path, dark)),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(theme::semantic(dark).text_primary)
                                        .child(item.source.file_name.clone()),
                                )
                                .children(if from_clipboard {
                                    Some(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(ui::text_tertiary())
                                            .child("📋"),
                                    )
                                } else {
                                    None
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family("SF Mono")
                                .text_color(ui::text_tertiary())
                                .child(format!(
                                    "{} x {}{}",
                                    item.source.preview.width,
                                    item.source.preview.height,
                                    if item.source.preview.has_alpha {
                                        " · alpha"
                                    } else {
                                        ""
                                    }
                                )),
                        ),
                )
                .child(
                    div()
                        .w(px(96.0))
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(ui::text_tertiary())
                        .child(format_size(item.source.original_size)),
                )
                .child(
                    div()
                        .w(px(96.0))
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(ui::text_tertiary())
                        .child(
                            item.output_size
                                .map(format_size)
                                .unwrap_or_else(|| "—".to_string()),
                        ),
                )
                .child(div().w(px(88.0)).flex().justify_center().child(status_tag(
                    item.status,
                    dark,
                    item.reduction_ratio,
                )))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        // "复制" — copy compressed image to clipboard (success only)
                        .children(if is_success {
                            Some(
                                action_button("复制", dark)
                                    .id(("image-compress-copy", index))
                                    .on_click({
                                        let panel = Rc::clone(&panel);
                                        move |_, window, cx| {
                                            let output_path = panel
                                                .borrow()
                                                .items
                                                .get(index)
                                                .and_then(|i| i.output_path.clone());
                                            if let Some(path) = output_path {
                                                match ImageCompressService::output_bytes(&path) {
                                                    Ok(bytes) => {
                                                        let fmt = platform::clipboard::image_format_from_path(&path)
                                                            .unwrap_or(gpui::ImageFormat::Png);
                                                        platform::clipboard::write_image(cx, fmt, bytes);
                                                        panel.borrow_mut().message =
                                                            String::from("已复制压缩图片到剪贴板");
                                                    }
                                                    Err(e) => {
                                                        panel.borrow_mut().message =
                                                            format!("复制失败: {e}");
                                                    }
                                                }
                                            }
                                            window.refresh();
                                        }
                                    }),
                            )
                        } else {
                            None
                        })
                        // "定位" — reveal in Finder (success only)
                        .children(if is_success {
                            Some(
                                action_button("定位", dark)
                                    .id(("image-compress-reveal", index))
                                    .on_click({
                                        let panel = Rc::clone(&panel);
                                        move |_, window, _cx| {
                                            panel.borrow_mut().reveal_entry(index);
                                            window.refresh();
                                        }
                                    }),
                            )
                        } else {
                            None
                        })
                        // "覆盖" — overwrite original (success, real file only)
                        .children(if is_success && !from_clipboard && has_source {
                            Some(
                                action_button("覆盖", dark)
                                    .id(("image-compress-overwrite", index))
                                    .on_click({
                                        let panel = Rc::clone(&panel);
                                        move |_, window, cx| {
                                            panel
                                                .borrow_mut()
                                                .overwrite_entry_background(index, cx.to_async());
                                            window.refresh();
                                        }
                                    }),
                            )
                        } else {
                            None
                        })
                        // "另存为" — save-as copy (success only)
                        .children(if is_success {
                            Some(
                                action_button("另存", dark)
                                    .id(("image-compress-save-as", index))
                                    .on_click({
                                        let panel = Rc::clone(&panel);
                                        move |_, window, _cx| {
                                            let output_path = panel
                                                .borrow()
                                                .items
                                                .get(index)
                                                .and_then(|i| i.output_path.clone());
                                            if let Some(src) = output_path {
                                                if let Some(target) = rfd::FileDialog::new()
                                                    .set_title("另存为")
                                                    .set_file_name(
                                                        src.file_name()
                                                            .and_then(|n| n.to_str())
                                                            .unwrap_or("compressed.png"),
                                                    )
                                                    .save_file()
                                                {
                                                    let mut p = panel.borrow_mut();
                                                    match std::fs::copy(&src, &target) {
                                                        Ok(_) => {
                                                            p.message = format!(
                                                                "已保存到 {}",
                                                                target.display()
                                                            );
                                                        }
                                                        Err(e) => {
                                                            p.message = format!("保存失败: {e}")
                                                        }
                                                    }
                                                }
                                            }
                                            window.refresh();
                                        }
                                    }),
                            )
                        } else {
                            None
                        })
                        // "重试" — retry failed entry
                        .children(if is_failed {
                            Some(
                                action_button("重试", dark)
                                    .id(("image-compress-retry", index))
                                    .on_click({
                                        let panel = Rc::clone(&panel);
                                        move |_, window, cx| {
                                            panel
                                                .borrow_mut()
                                                .retry_entry_background(index, cx.to_async());
                                            window.refresh();
                                        }
                                    }),
                            )
                        } else {
                            None
                        })
                        // "✕" — remove (hidden while running)
                        .children(if !is_running {
                            Some(
                                div()
                                    .id(("image-compress-remove", index))
                                    .w(px(20.0))
                                    .text_size(px(10.0))
                                    .text_color(ui::text_tertiary())
                                    .hover(move |style| style.cursor_pointer())
                                    .child("✕")
                                    .on_click({
                                        let panel = Rc::clone(&panel);
                                        move |_, window, _cx| {
                                            panel.borrow_mut().remove_item(index);
                                            window.refresh();
                                        }
                                    }),
                            )
                        } else {
                            None
                        }),
                ),
        )
}

fn thumbnail(path: &Path, dark: bool) -> impl IntoElement {
    div()
        .size(px(THUMB_SIZE))
        .rounded(px(8.0))
        .bg(theme::rgba_with_alpha(theme::semantic(dark).bg_subtle, 0.8))
        .border_1()
        .border_color(ui::border_light())
        .overflow_hidden()
        .child(
            img(path.to_path_buf())
                .size(px(THUMB_SIZE))
                .into_any_element(),
        )
}

fn status_tag(status: QueueStatus, dark: bool, reduction_ratio: Option<f32>) -> impl IntoElement {
    let (bg, text) = match status {
        QueueStatus::Success => (
            theme::rgba_with_alpha(theme::semantic(dark).success, 0.1),
            theme::semantic(dark).success,
        ),
        QueueStatus::Running => (
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Blue), 0.1),
            ui::accent_color(PluginAccent::Blue),
        ),
        QueueStatus::Pending => (
            theme::rgba_with_alpha(theme::semantic(dark).warning, 0.1),
            theme::semantic(dark).warning,
        ),
        QueueStatus::Failed => (
            theme::rgba_with_alpha(theme::semantic(dark).danger, 0.1),
            theme::semantic(dark).danger,
        ),
    };

    let label = match (status, reduction_ratio) {
        (QueueStatus::Success, Some(ratio)) if ratio.is_finite() => {
            format!("✓ {:.0}%", (ratio.max(0.0) * 100.0).round())
        }
        _ => status.label().to_string(),
    };

    div()
        .px_2()
        .h(px(20.0))
        .rounded(px(999.0))
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(text)
        .child(label)
}

fn table_header_cell(label: &str, width: f32) -> impl IntoElement {
    div().w(px(width)).child(label.to_string())
}

fn table_header_flex(label: &str, grow: f32) -> impl IntoElement {
    let cell = div().child(label.to_string());
    if grow >= 2.0 {
        cell.flex_1().into_any_element()
    } else {
        cell.w(px(96.0)).into_any_element()
    }
}

fn footer_bar(
    dark: bool,
    message: String,
    output_dir: String,
    overwrite_original: bool,
    pending_count: usize,
    running_count: usize,
    success_count: usize,
    average_ratio: Option<f32>,
    running: bool,
    batch_total: usize,
    batch_completed: usize,
    panel: Rc<RefCell<ImageCompressPanel>>,
) -> impl IntoElement {
    let summary = if running && batch_total > 0 {
        format!(
            "{}/{} 张已完成 · {} 张压缩中",
            batch_completed, batch_total, running_count
        )
    } else if running_count > 0 {
        format!("{running_count} 张压缩中 · {pending_count} 张待处理 · {success_count} 张已完成")
    } else if let Some(ratio) = average_ratio {
        format!(
            "{} 张待处理 · {} 张已完成 · 平均压缩率 {:.0}%",
            pending_count,
            success_count,
            (ratio.max(0.0) * 100.0).round()
        )
    } else {
        format!("{pending_count} 张待处理 · {success_count} 张已完成")
    };

    div()
        .rounded(px(10.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic(dark).bg_surface,
            0.7,
        ))
        .border_1()
        .border_color(ui::border_light())
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    primary_button(
                        if running {
                            "⏳ 压缩中…"
                        } else {
                            "▶ 开始压缩"
                        },
                        PluginAccent::Amber,
                        dark,
                    )
                    .id("image-compress-run")
                    .on_click({
                        let panel = Rc::clone(&panel);
                        move |_, window, cx| {
                            panel.borrow_mut().run_compression(cx);
                            window.refresh();
                        }
                    }),
                )
                .children(if running {
                    Some(
                        secondary_button("⏹ 取消", dark)
                            .id("image-compress-cancel")
                            .on_click({
                                let panel = Rc::clone(&panel);
                                move |_, window, _cx| {
                                    panel.borrow_mut().request_cancel();
                                    window.refresh();
                                }
                            }),
                    )
                } else {
                    None
                })
                .child(
                    secondary_button(
                        if overwrite_original {
                            "📝 覆盖原图"
                        } else {
                            "📝 输出到目录"
                        },
                        dark,
                    )
                    .id("image-compress-toggle-overwrite")
                    .on_click({
                        let panel = Rc::clone(&panel);
                        move |_, window, _cx| {
                            panel.borrow_mut().toggle_overwrite();
                            window.refresh();
                        }
                    }),
                )
                .child(
                    secondary_button("💾 选择目录", dark)
                        .id("image-compress-output-dir")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().choose_output_dir();
                                window.refresh();
                            }
                        }),
                )
                .child(
                    secondary_button("📂 打开目录", dark)
                        .id("image-compress-open-dir")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().open_output_dir();
                                window.refresh();
                            }
                        }),
                )
                .child(
                    ghost_button("🗑 清空", dark)
                        .id("image-compress-clear")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().clear_items();
                                window.refresh();
                            }
                        }),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_tertiary())
                        .child(summary),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(theme::semantic(dark).text_regular)
                        .child(message),
                )
                .child(
                    div()
                        .w(px(320.0))
                        .text_size(px(10.0))
                        .font_family("SF Mono")
                        .text_color(ui::text_tertiary())
                        .child(output_dir),
                ),
        )
}

fn primary_button(label: &str, accent: PluginAccent, dark: bool) -> gpui::Div {
    components::button(
        label.to_string(),
        components::ButtonVariant::Primary,
        Some(accent),
        dark,
    )
}

fn secondary_button(label: &str, dark: bool) -> gpui::Div {
    components::button(
        label.to_string(),
        components::ButtonVariant::Secondary,
        None,
        dark,
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

fn action_button(label: &str, dark: bool) -> gpui::Div {
    div()
        .h(px(22.0))
        .px_2()
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(
            theme::semantic(dark).bg_surface,
            0.88,
        ))
        .border_1()
        .border_color(ui::border_light())
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(theme::semantic(dark).text_primary)
        .child(label.to_string())
}

fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.0} KB", bytes_f / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::image_compress::service::ImportPreview;

    #[test]
    fn formats_byte_sizes() {
        assert_eq!(format_size(980), "980 B");
        assert_eq!(format_size(4 * 1024), "4 KB");
        assert_eq!(format_size(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn normalize_plain_path() {
        let p = normalize_image_input_path("/Users/me/photo.png");
        assert_eq!(p, PathBuf::from("/Users/me/photo.png"));
    }

    #[test]
    fn normalize_file_url() {
        let p = normalize_image_input_path("file:///Users/me/photo.jpg");
        assert_eq!(p, PathBuf::from("/Users/me/photo.jpg"));
    }

    #[test]
    fn normalize_file_url_localhost() {
        let p = normalize_image_input_path("file://localhost/Users/me/photo.webp");
        assert_eq!(p, PathBuf::from("/Users/me/photo.webp"));
    }

    #[test]
    fn normalize_quoted_path() {
        let p = normalize_image_input_path("\"/Users/me/my photo.png\"");
        assert_eq!(p, PathBuf::from("/Users/me/my photo.png"));
    }

    #[test]
    fn normalize_single_quoted_path() {
        let p = normalize_image_input_path("'/Users/me/photo.png'");
        assert_eq!(p, PathBuf::from("/Users/me/photo.png"));
    }

    #[test]
    fn normalize_tilde_path() {
        let p = normalize_image_input_path("~/Pictures/test.jpeg");
        let home = dirs::home_dir().expect("home dir should exist");
        assert_eq!(p, home.join("Pictures/test.jpeg"));
    }

    #[test]
    fn looks_like_image_true() {
        assert!(looks_like_image_path(Path::new("/a/b.png")));
        assert!(looks_like_image_path(Path::new("/a/b.jpg")));
        assert!(looks_like_image_path(Path::new("/a/b.jpeg")));
        assert!(looks_like_image_path(Path::new("/a/b.webp")));
        assert!(looks_like_image_path(Path::new("/a/b.PNG")));
    }

    #[test]
    fn looks_like_image_false() {
        assert!(!looks_like_image_path(Path::new("/a/b.txt")));
        assert!(!looks_like_image_path(Path::new("/a/b")));
        assert!(!looks_like_image_path(Path::new("/a/b.pdf")));
    }

    #[test]
    fn image_paths_from_input_multi_line() {
        let text = "/a/photo1.png\n/b/photo2.jpg\n/c/document.txt";
        let paths = image_paths_from_input(text);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/a/photo1.png"));
        assert_eq!(paths[1], PathBuf::from("/b/photo2.jpg"));
    }

    #[test]
    fn image_paths_from_input_null_separated() {
        let text = "/a/photo.png\0/b/photo.webp";
        let paths = image_paths_from_input(text);
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn image_paths_from_input_with_file_urls() {
        let text = "file:///Users/me/a.png\nfile://localhost/Users/me/b.jpg";
        let paths = image_paths_from_input(text);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/Users/me/a.png"));
        assert_eq!(paths[1], PathBuf::from("/Users/me/b.jpg"));
    }

    #[test]
    fn image_paths_from_input_empty() {
        assert!(image_paths_from_input("").is_empty());
        assert!(image_paths_from_input("no paths here").is_empty());
    }

    #[test]
    fn queue_status_running_label() {
        assert_eq!(QueueStatus::Running.label(), "压缩中…");
        assert!(!QueueStatus::Running.is_terminal());
        assert!(QueueStatus::Success.is_terminal());
        assert!(QueueStatus::Failed.is_terminal());
        assert!(!QueueStatus::Pending.is_terminal());
    }

    #[test]
    fn shared_batch_results_default_empty() {
        let shared = SharedBatchResults::default();
        let state = shared.inner.lock().unwrap();
        assert!(state.results.is_empty());
        assert!(state.message.is_none());
    }

    #[test]
    fn shared_batch_results_drain_results() {
        let shared = SharedBatchResults::default();

        // Simulate worker writing results
        {
            let mut state = shared.inner.lock().unwrap();
            state.results.push(BatchResultItem {
                index: 0,
                result: Err("file not found".to_string()),
            });
            state.results.push(BatchResultItem {
                index: 1,
                result: Ok(CompressionResult {
                    output_path: PathBuf::from("/out/a_compressed.png"),
                    output_size: 500,
                    reduction_ratio: 0.5,
                }),
            });
            state.message = Some("done".to_string());
            state.batch_done = true;
        }

        // Simulate panel draining results
        let mut results = Vec::new();
        let mut batch_message = None;
        {
            let mut state = shared.inner.lock().unwrap();
            if !state.results.is_empty() {
                results = std::mem::take(&mut state.results);
            }
            if state.batch_done {
                if let Some(ref msg) = state.message {
                    batch_message = Some(msg.clone());
                }
            }
        }

        assert_eq!(results.len(), 2);
        assert!(results[0].result.is_err());
        assert!(results[1].result.is_ok());
        assert_eq!(batch_message, Some("done".to_string()));

        // After drain, shared state should be empty
        let state = shared.inner.lock().unwrap();
        assert!(state.results.is_empty());
    }

    #[test]
    fn shared_batch_results_clone_shares_state() {
        let shared = SharedBatchResults::default();
        let clone = shared.clone();

        // Write through clone
        {
            let mut state = clone.inner.lock().unwrap();
            state.results.push(BatchResultItem {
                index: 0,
                result: Err("err".to_string()),
            });
        }

        // Read through original
        let state = shared.inner.lock().unwrap();
        assert_eq!(state.results.len(), 1);
    }

    #[test]
    fn cancel_flag_default_false() {
        let shared = SharedBatchResults::default();
        assert!(!shared.cancel_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_flag_can_be_set() {
        let shared = SharedBatchResults::default();
        shared.cancel_requested.store(true, Ordering::Relaxed);
        assert!(shared.cancel_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn batch_done_default_false() {
        let shared = SharedBatchResults::default();
        let state = shared.inner.lock().unwrap();
        assert!(!state.batch_done);
    }

    #[test]
    fn batch_done_set_by_worker() {
        let shared = SharedBatchResults::default();
        {
            let mut state = shared.inner.lock().unwrap();
            state.message = Some("cancelled".to_string());
            state.batch_done = true;
        }
        let state = shared.inner.lock().unwrap();
        assert!(state.batch_done);
        assert_eq!(state.message, Some("cancelled".to_string()));
    }

    #[test]
    fn collect_results_with_batch_done_drains_message() {
        // Simulate: worker writes 1 result, then sets batch_done + message.
        let shared = SharedBatchResults::default();
        {
            let mut state = shared.inner.lock().unwrap();
            state.results.push(BatchResultItem {
                index: 0,
                result: Ok(CompressionResult {
                    output_path: PathBuf::from("/out/a.png"),
                    output_size: 100,
                    reduction_ratio: 0.5,
                }),
            });
            state.batch_done = true;
            state.message = Some("压缩完成，共处理 1 张".to_string());
        }

        // Panel collects
        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 0,
            items: vec![QueueItem {
                source: ImportedImage {
                    path: PathBuf::from("/src/a.png"),
                    file_name: "a.png".to_string(),
                    original_size: 200,
                    preview: ImportPreview {
                        width: 10,
                        height: 10,
                        has_alpha: false,
                    },
                },
                status: QueueStatus::Running,
                output_size: None,
                output_path: None,
                reduction_ratio: None,
                error_message: String::new(),
                from_clipboard: false,
            }],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        let changed = panel.collect_results();
        assert!(changed);
        assert_eq!(panel.items[0].status, QueueStatus::Success);
        // running should be cleared because no items are Running anymore
        assert!(!panel.running);
        assert_eq!(panel.message, "压缩完成，共处理 1 张");
    }

    #[test]
    fn collect_results_without_batch_done_drains_but_keeps_running() {
        let shared = SharedBatchResults::default();
        {
            let mut state = shared.inner.lock().unwrap();
            state.results.push(BatchResultItem {
                index: 0,
                result: Err("oops".to_string()),
            });
            // batch_done is still false — worker hasn't finished all items
        }

        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 0,
            items: vec![
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/a.png"),
                        file_name: "a.png".to_string(),
                        original_size: 200,
                        preview: ImportPreview {
                            width: 10,
                            height: 10,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/b.png"),
                        file_name: "b.png".to_string(),
                        original_size: 300,
                        preview: ImportPreview {
                            width: 20,
                            height: 20,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
            ],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        let changed = panel.collect_results();
        assert!(changed);
        assert_eq!(panel.items[0].status, QueueStatus::Failed);
        // Item 1 is still Running, so running should stay true
        assert!(panel.running);
        // No batch message since batch_done is false
        assert!(panel.message.is_empty());
    }

    #[test]
    fn request_cancel_sets_flag_and_resets_pending_items() {
        let shared = SharedBatchResults::default();
        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 2,
            items: vec![
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/a.png"),
                        file_name: "a.png".to_string(),
                        original_size: 200,
                        preview: ImportPreview {
                            width: 10,
                            height: 10,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/b.png"),
                        file_name: "b.png".to_string(),
                        original_size: 300,
                        preview: ImportPreview {
                            width: 20,
                            height: 20,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Pending,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
            ],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        panel.request_cancel();

        assert!(shared.cancel_requested.load(Ordering::Relaxed));
        // Running item was reset to Pending
        assert_eq!(panel.items[0].status, QueueStatus::Pending);
        // Pending item stays Pending
        assert_eq!(panel.items[1].status, QueueStatus::Pending);
        assert!(
            panel.message.contains("正在取消…"),
            "expected cancel message, got: {}",
            panel.message
        );
        // Running should be cleared after cancel
        assert!(!panel.running);
        // batch_total should be cleared after cancel
        assert_eq!(panel.batch_total, 0);
    }

    #[test]
    fn request_cancel_noop_when_not_running() {
        let shared = SharedBatchResults::default();
        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 0,
            items: vec![],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: false,
            shared: shared.clone(),
            drain_task: None,
        };

        panel.request_cancel();
        // Flag should NOT be set when not running
        assert!(!shared.cancel_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn batch_completed_counts_correctly() {
        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            items: vec![],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: false,
            batch_total: 5,
            shared: SharedBatchResults::default(),
            drain_task: None,
        };

        // No items running, batch_total = 5 -> completed = 5
        assert_eq!(panel.batch_completed(), 5);

        // Add running items
        panel.batch_total = 5;
        for _ in 0..3 {
            panel.items.push(QueueItem {
                source: ImportedImage {
                    path: PathBuf::from("/src/x.png"),
                    file_name: "x.png".to_string(),
                    original_size: 100,
                    preview: ImportPreview {
                        width: 10,
                        height: 10,
                        has_alpha: false,
                    },
                },
                status: QueueStatus::Running,
                output_size: None,
                output_path: None,
                reduction_ratio: None,
                error_message: String::new(),
                from_clipboard: false,
            });
        }

        // 3 running out of 5 total -> completed = 2
        assert_eq!(panel.batch_completed(), 2);
    }

    #[test]
    fn batch_completed_never_negative() {
        let panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            items: vec![],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: false,
            batch_total: 0,
            shared: SharedBatchResults::default(),
            drain_task: None,
        };

        assert_eq!(panel.batch_completed(), 0);
    }

    #[test]
    fn collect_results_updates_progress_during_batch() {
        let shared = SharedBatchResults::default();
        {
            let mut state = shared.inner.lock().unwrap();
            // 1 result written, batch not done yet
            state.results.push(BatchResultItem {
                index: 0,
                result: Ok(CompressionResult {
                    output_path: PathBuf::from("/out/a.png"),
                    output_size: 400,
                    reduction_ratio: 0.6,
                }),
            });
        }

        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 3,
            items: vec![
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/a.png"),
                        file_name: "a.png".to_string(),
                        original_size: 1000,
                        preview: ImportPreview {
                            width: 10,
                            height: 10,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/b.png"),
                        file_name: "b.png".to_string(),
                        original_size: 2000,
                        preview: ImportPreview {
                            width: 20,
                            height: 20,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/c.png"),
                        file_name: "c.png".to_string(),
                        original_size: 3000,
                        preview: ImportPreview {
                            width: 30,
                            height: 30,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
            ],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        let changed = panel.collect_results();
        assert!(changed);
        // Item 0 is now Success
        assert_eq!(panel.items[0].status, QueueStatus::Success);
        // Items 1 and 2 still Running
        assert_eq!(panel.items[1].status, QueueStatus::Running);
        assert_eq!(panel.items[2].status, QueueStatus::Running);
        // Still running overall
        assert!(panel.running);
        // Progress message should show 1/3 completed
        assert!(
            panel.message.contains("1/3"),
            "expected progress 1/3, got: {}",
            panel.message
        );
        // batch_total should be preserved
        assert_eq!(panel.batch_total, 3);
    }

    #[test]
    fn collect_results_clears_batch_total_on_completion() {
        let shared = SharedBatchResults::default();
        {
            let mut state = shared.inner.lock().unwrap();
            state.results.push(BatchResultItem {
                index: 0,
                result: Ok(CompressionResult {
                    output_path: PathBuf::from("/out/a.png"),
                    output_size: 400,
                    reduction_ratio: 0.6,
                }),
            });
            state.batch_done = true;
            state.message = Some("压缩完成，共处理 1 张".to_string());
        }

        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 1,
            items: vec![QueueItem {
                source: ImportedImage {
                    path: PathBuf::from("/src/a.png"),
                    file_name: "a.png".to_string(),
                    original_size: 1000,
                    preview: ImportPreview {
                        width: 10,
                        height: 10,
                        has_alpha: false,
                    },
                },
                status: QueueStatus::Running,
                output_size: None,
                output_path: None,
                reduction_ratio: None,
                error_message: String::new(),
                from_clipboard: false,
            }],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        panel.collect_results();
        assert!(!panel.running);
        assert_eq!(panel.batch_total, 0);
        assert_eq!(panel.message, "压缩完成，共处理 1 张");
    }

    #[test]
    fn collect_results_no_progress_when_batch_total_zero() {
        let shared = SharedBatchResults::default();
        {
            let mut state = shared.inner.lock().unwrap();
            state.results.push(BatchResultItem {
                index: 0,
                result: Err("failed".to_string()),
            });
        }

        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 0,
            items: vec![QueueItem {
                source: ImportedImage {
                    path: PathBuf::from("/src/a.png"),
                    file_name: "a.png".to_string(),
                    original_size: 100,
                    preview: ImportPreview {
                        width: 10,
                        height: 10,
                        has_alpha: false,
                    },
                },
                status: QueueStatus::Running,
                output_size: None,
                output_path: None,
                reduction_ratio: None,
                error_message: String::new(),
                from_clipboard: false,
            }],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        panel.collect_results();
        // Status updated
        assert_eq!(panel.items[0].status, QueueStatus::Failed);
        // No progress message because batch_total is 0
        assert!(panel.message.is_empty());
    }

    #[test]
    fn cancel_tracks_completed_and_cancelled_counts() {
        let shared = SharedBatchResults::default();
        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 5,
            items: vec![
                // Already completed item
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/a.png"),
                        file_name: "a.png".to_string(),
                        original_size: 200,
                        preview: ImportPreview {
                            width: 10,
                            height: 10,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Success,
                    output_size: Some(100),
                    output_path: Some(PathBuf::from("/out/a.png")),
                    reduction_ratio: Some(0.5),
                    error_message: String::new(),
                    from_clipboard: false,
                },
                // Running item — will be cancelled
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/b.png"),
                        file_name: "b.png".to_string(),
                        original_size: 300,
                        preview: ImportPreview {
                            width: 20,
                            height: 20,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
                // Running item — will be cancelled
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/c.png"),
                        file_name: "c.png".to_string(),
                        original_size: 400,
                        preview: ImportPreview {
                            width: 30,
                            height: 30,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Running,
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
            ],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        panel.request_cancel();

        assert!(shared.cancel_requested.load(Ordering::Relaxed));
        // Running items reverted to Pending
        assert_eq!(panel.items[1].status, QueueStatus::Pending);
        assert_eq!(panel.items[2].status, QueueStatus::Pending);
        // Already-success item unchanged
        assert_eq!(panel.items[0].status, QueueStatus::Success);
        // Cancel finalizes state
        assert!(!panel.running);
        assert_eq!(panel.batch_total, 0);
        assert!(
            panel.message.contains("2 张已回退"),
            "expected 2 cancelled items in message, got: {}",
            panel.message
        );
    }

    #[test]
    fn cancel_then_collect_results_overrides_processed_items() {
        // Simulates: worker processed item 0, then cancel was requested.
        // collect_results should override Pending→Success for the processed item.
        let shared = SharedBatchResults::default();
        {
            let mut state = shared.inner.lock().unwrap();
            state.results.push(BatchResultItem {
                index: 1,
                result: Ok(CompressionResult {
                    output_path: PathBuf::from("/out/b.png"),
                    output_size: 150,
                    reduction_ratio: 0.5,
                }),
            });
            state.batch_done = true;
            state.message = Some("已取消，完成 1 张，取消 1 张".to_string());
        }

        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            batch_total: 2,
            items: vec![
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/a.png"),
                        file_name: "a.png".to_string(),
                        original_size: 200,
                        preview: ImportPreview {
                            width: 10,
                            height: 10,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Success,
                    output_size: Some(100),
                    output_path: Some(PathBuf::from("/out/a.png")),
                    reduction_ratio: Some(0.5),
                    error_message: String::new(),
                    from_clipboard: false,
                },
                QueueItem {
                    source: ImportedImage {
                        path: PathBuf::from("/src/b.png"),
                        file_name: "b.png".to_string(),
                        original_size: 300,
                        preview: ImportPreview {
                            width: 20,
                            height: 20,
                            has_alpha: false,
                        },
                    },
                    status: QueueStatus::Pending, // was reset by cancel
                    output_size: None,
                    output_path: None,
                    reduction_ratio: None,
                    error_message: String::new(),
                    from_clipboard: false,
                },
            ],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: true,
            shared: shared.clone(),
            drain_task: None,
        };

        let changed = panel.collect_results();
        assert!(changed);
        // Item 1 was processed before cancel → now Success (overrides Pending)
        assert_eq!(panel.items[1].status, QueueStatus::Success);
        assert_eq!(panel.items[1].output_size, Some(150));
        // Batch done, running cleared
        assert!(!panel.running);
        // Message from worker
        assert_eq!(panel.message, "已取消，完成 1 张，取消 1 张");
    }

    #[test]
    fn batch_total_saturating_sub_never_panics() {
        let mut panel = ImageCompressPanel {
            service: ImageCompressService::for_test(PathBuf::from("/tmp")),
            items: vec![],
            mode: CompressionMode::VisuallyLossless,
            quality: 80,
            overwrite_original: false,
            output_dir: PathBuf::from("/tmp"),
            message: String::new(),
            running: false,
            batch_total: 3,
            shared: SharedBatchResults::default(),
            drain_task: None,
        };

        // Add 5 running items — more than batch_total
        for _ in 0..5 {
            panel.items.push(QueueItem {
                source: ImportedImage {
                    path: PathBuf::from("/src/x.png"),
                    file_name: "x.png".to_string(),
                    original_size: 100,
                    preview: ImportPreview {
                        width: 10,
                        height: 10,
                        has_alpha: false,
                    },
                },
                status: QueueStatus::Running,
                output_size: None,
                output_path: None,
                reduction_ratio: None,
                error_message: String::new(),
                from_clipboard: false,
            });
        }

        // 5 running, batch_total 3 -> completed = 0 (saturating)
        assert_eq!(panel.batch_completed(), 0);
    }
}
