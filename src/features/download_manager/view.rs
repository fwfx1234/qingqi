use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gpui::{
    App, AppContext, Component, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px, relative,
};

use crate::{
    app::{
        text_input::{TextInput, TextInputStyle},
        theme, ui,
    },
    core::{
        job::{JobId, JobProvider},
        plugin_spec::PluginAccent,
    },
    platform,
};

use super::{
    model::{DownloadTask, TaskStatus},
    service::DownloadService,
    store::{DownloadStats, TaskCounts},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterTab {
    All,
    Active,
    Paused,
    Completed,
    Failed,
    Video,
    Audio,
    Document,
    Archive,
    Image,
    Software,
    Other,
}

impl FilterTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Active => "进行中",
            Self::Paused => "已暂停",
            Self::Completed => "已完成",
            Self::Failed => "失败",
            Self::Video => "视频",
            Self::Audio => "音频",
            Self::Document => "文档",
            Self::Archive => "压缩包",
            Self::Image => "图片",
            Self::Software => "软件",
            Self::Other => "其他",
        }
    }

    fn count(&self, counts: &TaskCounts) -> usize {
        match self {
            Self::All => counts.total,
            Self::Active => counts.active(),
            Self::Paused => counts.paused,
            Self::Completed => counts.completed,
            Self::Failed => counts.failed + counts.cancelled,
            Self::Video => counts.video,
            Self::Audio => counts.audio,
            Self::Document => counts.document,
            Self::Archive => counts.archive,
            Self::Image => counts.image,
            Self::Software => counts.software,
            Self::Other => counts.other,
        }
    }
}

pub struct DownloadManagerPanel {
    service: Rc<RefCell<DownloadService>>,
    tasks: Vec<DownloadTask>,
    job_summary: DownloadJobSummary,
    task_counts: TaskCounts,
    settings_snapshot: super::model::DownloadSettings,
    save_dir_snapshot: String,
    stats_snapshot: DownloadStats,
    filter: FilterTab,
    url_input_entity: Option<Entity<TextInput>>,
    url_text: String,
    message: String,
    service_revision: u64,
    subscriptions: Vec<Subscription>,
    show_settings: bool,
    settings_need_reload: bool,
    // Settings input entities
    save_root_input: Option<Entity<TextInput>>,
    concurrent_input: Option<Entity<TextInput>>,
    speed_limit_input: Option<Entity<TextInput>>,
    timeout_input: Option<Entity<TextInput>>,
    retry_input: Option<Entity<TextInput>>,
    proxy_input: Option<Entity<TextInput>>,
    user_agent_input: Option<Entity<TextInput>>,
    referer_input: Option<Entity<TextInput>>,
    cookie_input: Option<Entity<TextInput>>,
    headers_input: Option<Entity<TextInput>>,
}

impl DownloadManagerPanel {
    pub fn new(service: Rc<RefCell<DownloadService>>) -> Self {
        let tasks = Self::load_tasks(&service, FilterTab::All);
        let (
            job_summary,
            task_counts,
            settings_snapshot,
            save_dir_snapshot,
            stats_snapshot,
            service_revision,
        ) = {
            let svc = service.borrow();
            (
                DownloadJobSummary::from_jobs(svc.job_snapshots()),
                svc.task_counts(),
                svc.settings_snapshot(),
                svc.effective_save_dir().to_string_lossy().to_string(),
                svc.stats(),
                svc.revision(),
            )
        };
        Self {
            service,
            tasks,
            job_summary,
            task_counts,
            settings_snapshot,
            save_dir_snapshot,
            stats_snapshot,
            filter: FilterTab::All,
            url_input_entity: None,
            url_text: String::new(),
            message: String::from("输入 URL 或粘贴链接开始下载"),
            service_revision,
            subscriptions: Vec::new(),
            show_settings: false,
            settings_need_reload: false,
            save_root_input: None,
            concurrent_input: None,
            speed_limit_input: None,
            timeout_input: None,
            retry_input: None,
            proxy_input: None,
            user_agent_input: None,
            referer_input: None,
            cookie_input: None,
            headers_input: None,
        }
    }

    pub fn init(&mut self, cx: &mut App) {
        self.ensure_inputs(cx);
    }

    fn ensure_inputs(&mut self, cx: &mut App) {
        if self.url_input_entity.is_none() {
            let input = cx.new(|cx| {
                let mut input = TextInput::new(cx, "输入下载链接...", "");
                input.set_chrome(false, cx);
                input.set_monospace(true, cx);
                input.set_style(
                    TextInputStyle {
                        height: 32.0,
                        font_size: 12.0,
                        padding: 10.0,
                    },
                    cx,
                );
                input
            });
            self.url_input_entity = Some(input);
        }

        let settings = self.service.borrow().settings_snapshot();

        if self.save_root_input.is_none() {
            self.save_root_input = Some(self.make_settings_input(cx, settings.save_root.clone()));
        }
        if self.concurrent_input.is_none() {
            self.concurrent_input =
                Some(self.make_settings_input(cx, settings.max_concurrent.to_string()));
        }
        if self.speed_limit_input.is_none() {
            let speed_val = if settings.speed_limit_kbps > 0 {
                settings.speed_limit_kbps.to_string()
            } else {
                String::new()
            };
            self.speed_limit_input = Some(self.make_settings_input(cx, speed_val));
        }
        if self.timeout_input.is_none() {
            self.timeout_input =
                Some(self.make_settings_input(cx, settings.timeout_secs.to_string()));
        }
        if self.retry_input.is_none() {
            self.retry_input = Some(self.make_settings_input(cx, settings.retry_limit.to_string()));
        }
        if self.proxy_input.is_none() {
            self.proxy_input = Some(self.make_settings_input(cx, settings.proxy_url.clone()));
        }
        if self.user_agent_input.is_none() {
            self.user_agent_input = Some(self.make_settings_input(cx, settings.user_agent.clone()));
        }
        if self.referer_input.is_none() {
            self.referer_input = Some(self.make_settings_input(cx, settings.referer.clone()));
        }
        if self.cookie_input.is_none() {
            self.cookie_input = Some(self.make_settings_input(cx, settings.cookie.clone()));
        }
        if self.headers_input.is_none() {
            self.headers_input =
                Some(self.make_settings_input(cx, settings.custom_headers.clone()));
        }

        if self.settings_need_reload {
            self.reload_settings_inputs(cx);
            self.settings_need_reload = false;
        }
    }

    fn make_settings_input(&self, cx: &mut App, value: String) -> Entity<TextInput> {
        cx.new(|cx| {
            let mut input = TextInput::new(cx, "", &value);
            input.set_chrome(false, cx);
            input.set_monospace(true, cx);
            input.set_style(
                TextInputStyle {
                    height: 28.0,
                    font_size: 11.0,
                    padding: 8.0,
                },
                cx,
            );
            input
        })
    }

    fn load_tasks(service: &Rc<RefCell<DownloadService>>, filter: FilterTab) -> Vec<DownloadTask> {
        let tasks = service.borrow().tasks_snapshot();

        match filter {
            FilterTab::All => tasks,
            FilterTab::Active => tasks
                .into_iter()
                .filter(|t| t.status == TaskStatus::Pending || t.status == TaskStatus::Downloading)
                .collect(),
            FilterTab::Paused => tasks
                .into_iter()
                .filter(|t| t.status == TaskStatus::Paused)
                .collect(),
            FilterTab::Completed => tasks
                .into_iter()
                .filter(|t| t.status == TaskStatus::Completed)
                .collect(),
            FilterTab::Failed => tasks
                .into_iter()
                .filter(|t| t.status == TaskStatus::Failed || t.status == TaskStatus::Cancelled)
                .collect(),
            FilterTab::Video => tasks
                .into_iter()
                .filter(|t| t.category == super::model::FileCategory::Video)
                .collect(),
            FilterTab::Audio => tasks
                .into_iter()
                .filter(|t| t.category == super::model::FileCategory::Audio)
                .collect(),
            FilterTab::Document => tasks
                .into_iter()
                .filter(|t| t.category == super::model::FileCategory::Document)
                .collect(),
            FilterTab::Archive => tasks
                .into_iter()
                .filter(|t| t.category == super::model::FileCategory::Archive)
                .collect(),
            FilterTab::Image => tasks
                .into_iter()
                .filter(|t| t.category == super::model::FileCategory::Image)
                .collect(),
            FilterTab::Software => tasks
                .into_iter()
                .filter(|t| t.category == super::model::FileCategory::Software)
                .collect(),
            FilterTab::Other => tasks
                .into_iter()
                .filter(|t| t.category == super::model::FileCategory::Other)
                .collect(),
        }
    }

    pub fn refresh(&mut self) {
        let service = self.service.borrow();
        self.tasks = Self::load_tasks(&self.service, self.filter);
        self.job_summary = DownloadJobSummary::from_jobs(service.job_snapshots());
        self.task_counts = service.task_counts();
        self.settings_snapshot = service.settings_snapshot();
        self.save_dir_snapshot = service.effective_save_dir().to_string_lossy().to_string();
        self.stats_snapshot = service.stats();
        self.service_revision = service.revision();
    }

    fn refresh_if_stale(&mut self) {
        let revision = self.service.borrow().revision();
        if revision != self.service_revision {
            self.refresh();
        }
    }

    pub fn cleanup(&mut self) {
        self.subscriptions.clear();
    }

    pub fn observe_inputs(&mut self, handle: Rc<RefCell<DownloadManagerPanel>>, cx: &mut App) {
        if !self.subscriptions.is_empty() {
            return;
        }
        let Some(input) = self.url_input_entity.clone() else {
            return;
        };
        let observed_input = input.clone();
        let subscription = cx.observe(&observed_input, move |_, cx| {
            let text = input.read(cx).text();
            handle.borrow_mut().url_text = text;
        });
        self.subscriptions.push(subscription);
    }

    fn clear_url_input(&mut self, cx: &mut App) {
        self.url_text.clear();
        if let Some(input) = self.url_input_entity.as_ref() {
            input.update(cx, |input, input_cx| input.clear(input_cx));
        }
    }

    fn set_url_input_text(&mut self, text: String, cx: &mut App) {
        self.url_text = text.clone();
        if let Some(input) = self.url_input_entity.as_ref() {
            input.update(cx, |input, input_cx| input.set_text(text, input_cx));
        }
    }

    pub fn add_download(&mut self, cx: &mut App) {
        let text = self.url_text.trim().to_string();
        if text.is_empty() {
            self.message = String::from("请输入下载链接");
            return;
        }
        // Try multi-URL extraction first
        let urls = super::model::extract_urls_from_text(&text);
        if urls.len() > 1 {
            let result = { self.service.borrow().add_urls_from_text(&text) };
            match result {
                Ok(tasks) => {
                    self.clear_url_input(cx);
                    self.message = format!("已添加 {} 个下载任务", tasks.len());
                    self.refresh();
                }
                Err(e) => {
                    self.message = format!("添加失败: {e}");
                }
            }
        } else if urls.len() == 1 {
            let (add_result, task_id) = {
                let svc = self.service.borrow();
                let result = svc.add_task(&urls[0]);
                let tid = result.as_ref().map(|t| t.id.clone()).ok();
                (result, tid)
            };
            match add_result {
                Ok(task) => {
                    self.clear_url_input(cx);
                    self.message = format!("已添加: {}", task.file_name);
                    if let Some(tid) = task_id {
                        let start_result = { self.service.borrow().start_download(&tid) };
                        if let Err(e) = start_result {
                            self.message = format!("启动失败: {e}");
                        }
                    }
                    self.refresh();
                }
                Err(e) => {
                    self.message = format!("添加失败: {e}");
                }
            }
        } else {
            self.message = String::from("未识别到有效 HTTP/HTTPS 链接");
        }
    }

    pub fn paste_and_add(&mut self, cx: &App) {
        let text = platform::clipboard::read_text(cx).unwrap_or_default();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.message = String::from("剪贴板为空");
            return;
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            self.url_text = trimmed.to_string();
        } else {
            self.message = String::from("剪贴板内容不是有效链接");
        }
    }

    pub fn set_filter(&mut self, filter: FilterTab) {
        self.filter = filter;
        self.refresh();
    }

    pub fn toggle_settings(&mut self) {
        self.show_settings = !self.show_settings;
        if self.show_settings {
            self.settings_need_reload = true;
        }
    }

    fn reload_settings_inputs(&mut self, cx: &mut App) {
        let settings = self.service.borrow().settings_snapshot();
        if let Some(input) = &self.save_root_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.save_root.clone(), input_cx)
            });
        }
        if let Some(input) = &self.concurrent_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.max_concurrent.to_string(), input_cx)
            });
        }
        if let Some(input) = &self.speed_limit_input {
            let val = if settings.speed_limit_kbps > 0 {
                settings.speed_limit_kbps.to_string()
            } else {
                String::new()
            };
            input.update(cx, |input, input_cx| input.set_text(val, input_cx));
        }
        if let Some(input) = &self.timeout_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.timeout_secs.to_string(), input_cx)
            });
        }
        if let Some(input) = &self.retry_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.retry_limit.to_string(), input_cx)
            });
        }
        if let Some(input) = &self.proxy_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.proxy_url.clone(), input_cx)
            });
        }
        if let Some(input) = &self.user_agent_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.user_agent.clone(), input_cx)
            });
        }
        if let Some(input) = &self.referer_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.referer.clone(), input_cx)
            });
        }
        if let Some(input) = &self.cookie_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.cookie.clone(), input_cx)
            });
        }
        if let Some(input) = &self.headers_input {
            input.update(cx, |input, input_cx| {
                input.set_text(settings.custom_headers.clone(), input_cx)
            });
        }
    }

    pub fn save_settings(&mut self, cx: &App) {
        let save_root = self
            .save_root_input
            .as_ref()
            .map(|e| e.read(cx).text())
            .unwrap_or_default();
        let concurrent: usize = self
            .concurrent_input
            .as_ref()
            .map(|e| e.read(cx).text().parse().unwrap_or(3))
            .unwrap_or(3);
        let speed_limit: u32 = self
            .speed_limit_input
            .as_ref()
            .map(|e| e.read(cx).text().parse().unwrap_or(0))
            .unwrap_or(0);
        let timeout: u32 = self
            .timeout_input
            .as_ref()
            .map(|e| e.read(cx).text().parse().unwrap_or(30))
            .unwrap_or(30);
        let retry: u32 = self
            .retry_input
            .as_ref()
            .map(|e| e.read(cx).text().parse().unwrap_or(2))
            .unwrap_or(2);
        let proxy = self
            .proxy_input
            .as_ref()
            .map(|e| e.read(cx).text())
            .unwrap_or_default();
        let user_agent = self
            .user_agent_input
            .as_ref()
            .map(|e| e.read(cx).text())
            .unwrap_or_default();
        let referer = self
            .referer_input
            .as_ref()
            .map(|e| e.read(cx).text())
            .unwrap_or_default();
        let cookie = self
            .cookie_input
            .as_ref()
            .map(|e| e.read(cx).text())
            .unwrap_or_default();
        let headers = self
            .headers_input
            .as_ref()
            .map(|e| e.read(cx).text())
            .unwrap_or_default();

        if !save_root.is_empty() {
            if let Err(e) = self.service.borrow().set_save_root(&save_root) {
                self.message = format!("保存目录设置失败: {e}");
            }
        }
        if let Err(e) = self.service.borrow().set_max_concurrent(concurrent) {
            self.message = format!("并发设置失败: {e}");
        }
        if let Err(e) = self.service.borrow().set_speed_limit_kbps(speed_limit) {
            self.message = format!("限速设置失败: {e}");
        }
        if let Err(e) = self.service.borrow().set_network_options(
            &user_agent,
            &referer,
            &cookie,
            &headers,
            &proxy,
            timeout,
            retry,
        ) {
            self.message = format!("网络设置失败: {e}");
        }

        self.show_settings = false;
        self.message = String::from("设置已保存");
        self.refresh();
    }

    pub fn pause_task(&mut self, id: &str) {
        if let Err(e) = self.service.borrow().pause_job(&JobId::new(id)) {
            self.message = format!("暂停失败: {e}");
        } else {
            self.message = String::from("已暂停");
        }
        self.refresh();
    }

    pub fn resume_task(&mut self, id: &str) {
        if let Err(e) = self.service.borrow().resume_job(&JobId::new(id)) {
            self.message = format!("恢复失败: {e}");
        } else {
            self.message = String::from("已恢复");
        }
        self.refresh();
    }

    pub fn cancel_task(&mut self, id: &str) {
        if let Err(e) = self.service.borrow().cancel_job(&JobId::new(id)) {
            self.message = format!("取消失败: {e}");
        } else {
            self.message = String::from("已取消");
        }
        self.refresh();
    }

    pub fn delete_task(&mut self, id: &str) {
        if let Err(e) = self.service.borrow().delete_task(id) {
            self.message = format!("删除失败: {e}");
        } else {
            self.message = String::from("已删除");
        }
        self.refresh();
    }

    pub fn start_all(&mut self) {
        match self.service.borrow().start_all_pending() {
            Ok(n) if n > 0 => {
                self.message = format!("已启动 {n} 个任务");
            }
            _ => self.message = String::from("没有待下载的任务"),
        }
        self.refresh();
    }

    pub fn pause_all(&mut self) {
        if let Err(e) = self.service.borrow().pause_all() {
            self.message = format!("暂停失败: {e}");
        } else {
            self.message = String::from("已暂停全部");
        }
        self.refresh();
    }

    pub fn resume_all(&mut self) {
        if let Err(e) = self.service.borrow().resume_all() {
            self.message = format!("恢复失败: {e}");
        } else {
            self.message = String::from("已恢复全部");
        }
        self.refresh();
    }

    pub fn clear_completed(&mut self) {
        match self.service.borrow().clear_completed() {
            Ok(n) if n > 0 => {
                self.message = format!("已清除 {n} 个已完成任务");
            }
            _ => self.message = String::from("没有已完成的任务"),
        }
        self.refresh();
    }

    pub fn clear_failed(&mut self) {
        match self.service.borrow().clear_failed() {
            Ok(n) if n > 0 => {
                self.message = format!("已清除 {n} 个失败/取消任务");
            }
            _ => self.message = String::from("没有失败的任务"),
        }
        self.refresh();
    }

    pub fn retry_task(&mut self, id: &str) {
        match self.service.borrow().retry_task(id) {
            Ok(()) => self.message = String::from("已加入队列"),
            Err(e) => self.message = format!("重试失败: {e}"),
        }
        self.refresh();
    }

    pub fn open_file(&mut self, task: &DownloadTask) {
        if task.status != TaskStatus::Completed {
            self.message = String::from("文件尚未下载完成");
            return;
        }
        match platform::shell::open_path(std::path::Path::new(&task.save_path)) {
            Ok(_) => self.message = String::from("已打开文件"),
            Err(e) => self.message = format!("打开失败: {e}"),
        }
    }

    pub fn open_save_dir(&mut self) {
        let dir = self.service.borrow().effective_save_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.message = format!("创建目录失败: {e}");
            return;
        }
        match platform::shell::open_path(&dir) {
            Ok(_) => self.message = String::from("已打开下载目录"),
            Err(e) => self.message = format!("打开失败: {e}"),
        }
    }
}

pub struct DownloadManagerElement {
    pub panel: Rc<RefCell<DownloadManagerPanel>>,
}

impl IntoElement for DownloadManagerElement {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for DownloadManagerElement {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut panel = self.panel.borrow_mut();
        panel.ensure_inputs(_cx);
        panel.observe_inputs(Rc::clone(&self.panel), _cx);
        panel.refresh_if_stale();
        let dark = crate::app::theme_mode::is_dark();
        let tasks = panel.tasks.clone();
        let filter = panel.filter;
        let show_settings = panel.show_settings;
        let url_input = panel.url_input_entity.clone().expect("url input missing");
        let message = panel.message.clone();
        let job_summary = panel.job_summary.clone();
        let task_counts = panel.task_counts.clone();
        let settings = panel.settings_snapshot.clone();
        let save_dir = panel.save_dir_snapshot.clone();
        let stats = panel.stats_snapshot.clone();
        // Clone settings input entities for overlay
        let save_root_input = panel.save_root_input.clone();
        let concurrent_input = panel.concurrent_input.clone();
        let speed_limit_input = panel.speed_limit_input.clone();
        let timeout_input = panel.timeout_input.clone();
        let retry_input = panel.retry_input.clone();
        let proxy_input = panel.proxy_input.clone();
        let user_agent_input = panel.user_agent_input.clone();
        let referer_input = panel.referer_input.clone();
        let cookie_input = panel.cookie_input.clone();
        let headers_input = panel.headers_input.clone();
        drop(panel);

        ui::plugin_surface(dark).child(
            ui::plugin_content().child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(header_bar(dark, job_summary.active_count))
                    .child(url_input_bar(dark, url_input, Rc::clone(&self.panel)))
                    .child(filter_bar(
                        dark,
                        filter,
                        &task_counts,
                        Rc::clone(&self.panel),
                    ))
                    .child(task_list(
                        dark,
                        tasks,
                        job_summary.progress_by_id,
                        Rc::clone(&self.panel),
                    ))
                    .child(bottom_bar(
                        dark,
                        message,
                        &save_dir,
                        &settings,
                        &stats,
                        Rc::clone(&self.panel),
                    ))
                    .child(if show_settings {
                        settings_overlay(
                            dark,
                            save_root_input,
                            concurrent_input,
                            speed_limit_input,
                            timeout_input,
                            retry_input,
                            proxy_input,
                            user_agent_input,
                            referer_input,
                            cookie_input,
                            headers_input,
                            Rc::clone(&self.panel),
                        )
                        .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            ),
        )
    }
}

struct DownloadJobSummary {
    active_count: usize,
    progress_by_id: HashMap<String, f64>,
}

impl Clone for DownloadJobSummary {
    fn clone(&self) -> Self {
        Self {
            active_count: self.active_count,
            progress_by_id: self.progress_by_id.clone(),
        }
    }
}

impl DownloadJobSummary {
    fn from_jobs(jobs: Vec<crate::core::job::JobSnapshot>) -> Self {
        let active_count = jobs.iter().filter(|job| job.status.is_active()).count();
        let progress_by_id = jobs
            .into_iter()
            .filter_map(|job| job.progress().map(|progress| (job.id.0, progress)))
            .collect();
        Self {
            active_count,
            progress_by_id,
        }
    }
}

fn header_bar(dark: bool, active_count: usize) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme::token("color-text-primary", dark))
                                .child("下载管理器"),
                        )
                        .child(
                            div()
                                .px_2()
                                .h(px(20.0))
                                .rounded(px(999.0))
                                .bg(theme::rgba_with_alpha(
                                    ui::accent_color(PluginAccent::Green),
                                    0.12,
                                ))
                                .flex()
                                .items_center()
                                .text_size(px(10.0))
                                .text_color(ui::accent_color(PluginAccent::Green))
                                .child("Download Manager"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::launcher_muted_text(dark))
                        .child("HTTP/HTTPS · 多任务 · 断点续传"),
                ),
        )
        .child(
            div().flex().items_center().gap_2().child(
                div()
                    .h(px(28.0))
                    .px_3()
                    .rounded(px(8.0))
                    .bg(if active_count > 0 {
                        theme::rgba_with_alpha(ui::accent_color(PluginAccent::Green), 0.12)
                    } else {
                        theme::rgba_with_alpha(theme::token("color-bg-surface", dark), 0.82)
                    })
                    .border_1()
                    .border_color(if active_count > 0 {
                        theme::rgba_with_alpha(ui::accent_color(PluginAccent::Green), 0.25)
                    } else {
                        theme::launcher_soft_line(dark)
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .text_color(if active_count > 0 {
                        ui::accent_color(PluginAccent::Green)
                    } else {
                        theme::launcher_muted_text(dark)
                    })
                    .child(format!("{} 进行中", active_count)),
            ),
        )
}

fn url_input_bar(
    dark: bool,
    url_input: Entity<TextInput>,
    panel: Rc<RefCell<DownloadManagerPanel>>,
) -> impl IntoElement {
    div()
        .rounded(px(12.0))
        .bg(theme::rgba_with_alpha(
            ui::accent_color(PluginAccent::Green),
            0.05,
        ))
        .border_1()
        .border_color(theme::rgba_with_alpha(
            ui::accent_color(PluginAccent::Green),
            0.18,
        ))
        .p_3()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .size(px(36.0))
                .rounded(px(10.0))
                .bg(theme::rgba_with_alpha(
                    ui::accent_color(PluginAccent::Green),
                    0.12,
                ))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .child("\u{1f517}"),
        )
        .child(
            div()
                .flex_1()
                .h(px(32.0))
                .rounded(px(8.0))
                .bg(theme::rgba_with_alpha(
                    theme::token("color-bg-surface", dark),
                    0.88,
                ))
                .border_1()
                .border_color(theme::launcher_soft_line(dark))
                .flex()
                .items_center()
                .child(url_input.into_any_element()),
        )
        .child(
            action_button("\u{1f4cb} 粘贴", dark)
                .id("download-paste")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, cx| {
                        let mut panel = panel.borrow_mut();
                        panel.paste_and_add(cx);
                        let text = panel.url_text.clone();
                        panel.set_url_input_text(text, cx);
                        window.refresh();
                    }
                }),
        )
        .child(
            primary_btn("添加下载", PluginAccent::Green, dark)
                .id("download-add")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, cx| {
                        panel.borrow_mut().add_download(cx);
                        window.refresh();
                    }
                }),
        )
}

fn filter_bar(
    dark: bool,
    active_filter: FilterTab,
    counts: &TaskCounts,
    panel: Rc<RefCell<DownloadManagerPanel>>,
) -> impl IntoElement {
    let status_filters = [
        FilterTab::All,
        FilterTab::Active,
        FilterTab::Paused,
        FilterTab::Completed,
        FilterTab::Failed,
    ];
    let category_filters = [
        FilterTab::Video,
        FilterTab::Audio,
        FilterTab::Document,
        FilterTab::Archive,
        FilterTab::Image,
        FilterTab::Software,
        FilterTab::Other,
    ];

    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .children(status_filters.iter().map(|&tab| {
                    let active = tab == active_filter;
                    let count = tab.count(counts);
                    filter_chip(tab.label(), count, active, dark)
                        .id(("download-filter", tab as usize))
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().set_filter(tab);
                                window.refresh();
                            }
                        })
                }))
                .child(div().flex_1())
                .child(
                    action_button("\u{25b6} 全部开始", dark)
                        .id("download-start-all")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().start_all();
                                window.refresh();
                            }
                        }),
                )
                .child(
                    action_button("\u{23f8} 全部暂停", dark)
                        .id("download-pause-all")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().pause_all();
                                window.refresh();
                            }
                        }),
                )
                .child(
                    action_button("\u{1f5d1} 清除已完成", dark)
                        .id("download-clear-done")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().clear_completed();
                                window.refresh();
                            }
                        }),
                )
                .child(
                    action_button("\u{26a0} 清除失败", dark)
                        .id("download-clear-failed")
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().clear_failed();
                                window.refresh();
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(theme::launcher_faint_text(dark))
                        .child("分类:"),
                )
                .children(category_filters.iter().map(|&tab| {
                    let active = tab == active_filter;
                    let count = tab.count(counts);
                    if count > 0 || active {
                        filter_chip(tab.label(), count, active, dark)
                            .id(("download-cat-filter", tab as usize))
                            .on_click({
                                let panel = Rc::clone(&panel);
                                move |_, window, _cx| {
                                    panel.borrow_mut().set_filter(tab);
                                    window.refresh();
                                }
                            })
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }
                })),
        )
}

fn filter_chip(label: &str, count: usize, active: bool, dark: bool) -> gpui::Div {
    div()
        .h(px(28.0))
        .px_3()
        .rounded(px(8.0))
        .bg(if active {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Green), 0.12)
        } else {
            theme::rgba_with_alpha(theme::token("color-bg-surface", dark), 0.82)
        })
        .border_1()
        .border_color(if active {
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Green), 0.25)
        } else {
            theme::launcher_soft_line(dark)
        })
        .hover(|style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .text_size(px(11.0))
        .text_color(if active {
            ui::accent_color(PluginAccent::Green)
        } else {
            theme::launcher_muted_text(dark)
        })
        .child(label.to_string())
        .child(
            div()
                .text_size(px(9.0))
                .text_color(if active {
                    ui::accent_color(PluginAccent::Green)
                } else {
                    theme::launcher_faint_text(dark)
                })
                .child(format!("{count}")),
        )
}

fn task_list(
    dark: bool,
    tasks: Vec<DownloadTask>,
    progress_by_id: HashMap<String, f64>,
    panel: Rc<RefCell<DownloadManagerPanel>>,
) -> impl IntoElement {
    if tasks.is_empty() {
        return div()
            .flex_1()
            .rounded(px(12.0))
            .bg(theme::rgba_with_alpha(
                theme::token("color-bg-surface", dark),
                0.74,
            ))
            .border_1()
            .border_color(theme::launcher_soft_line(dark))
            .child(ui::ui_empty_state("还没有下载任务", dark))
            .into_any_element();
    }

    div()
        .flex_1()
        .rounded(px(12.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.78,
        ))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(34.0))
                .px_3()
                .bg(theme::rgba_with_alpha(
                    theme::token("color-bg-subtle", dark),
                    0.65,
                ))
                .border_b_1()
                .border_color(theme::launcher_soft_line(dark))
                .flex()
                .items_center()
                .text_size(px(10.0))
                .text_color(theme::launcher_faint_text(dark))
                .child(table_header_cell("", 28.0))
                .child(table_header_flex("文件名", 2.2))
                .child(table_header_cell("大小", 90.0))
                .child(table_header_cell("速度", 80.0))
                .child(table_header_flex("进度", 1.6))
                .child(table_header_cell("状态", 72.0))
                .child(table_header_cell("", 30.0)),
        )
        .child(
            div()
                .id("download-task-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(6.0))
                .children(tasks.into_iter().enumerate().map(|(index, task)| {
                    let progress = progress_by_id.get(&task.id).copied();
                    task_row(task, progress, index, dark, Rc::clone(&panel))
                })),
        )
        .into_any_element()
}

fn task_row(
    task: DownloadTask,
    job_progress: Option<f64>,
    index: usize,
    dark: bool,
    panel: Rc<RefCell<DownloadManagerPanel>>,
) -> impl IntoElement {
    let status = task.status;
    let task_id = task.id.clone();
    let task_id2 = task.id.clone();
    let task_id3 = task.id.clone();
    let task_id4 = task.id.clone();
    let is_completed = task.status == TaskStatus::Completed;
    let is_active = task.status.is_active();
    let is_paused = task.status == TaskStatus::Paused;
    let is_failed = task.status == TaskStatus::Failed || task.status == TaskStatus::Cancelled;
    let is_terminal = task.status.is_terminal();

    div()
        .h(px(56.0))
        .px_3()
        .border_b_1()
        .border_color(theme::launcher_soft_line(dark))
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(28.0))
                .flex()
                .justify_center()
                .text_size(px(14.0))
                .child(task.category.icon()),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::token("color-text-primary", dark))
                        .child(task.file_name.clone()),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family("SF Mono")
                        .text_color(theme::launcher_faint_text(dark))
                        .child(truncate_url(&task.url, 56)),
                ),
        )
        .child(
            div()
                .w(px(90.0))
                .text_size(px(10.0))
                .font_family("SF Mono")
                .text_color(theme::launcher_faint_text(dark))
                .child(format_progress(&task, true)),
        )
        .child(
            div()
                .w(px(80.0))
                .text_size(px(10.0))
                .font_family("SF Mono")
                .text_color(if is_active {
                    ui::accent_color(PluginAccent::Green)
                } else {
                    theme::launcher_faint_text(dark)
                })
                .child(if is_active {
                    format_speed(task.speed_bps)
                } else {
                    "—".to_string()
                }),
        )
        .child(
            div()
                .w(px(150.0))
                .flex()
                .items_center()
                .gap_2()
                .child(progress_bar(
                    dark,
                    job_progress,
                    task.progress_percent(),
                    is_active,
                ))
                .child(
                    div()
                        .w(px(36.0))
                        .text_size(px(9.0))
                        .font_family("SF Mono")
                        .text_align(gpui::TextAlign::Right)
                        .text_color(theme::launcher_faint_text(dark))
                        .child(format!("{:.0}%", task.progress_percent())),
                ),
        )
        .child(
            div()
                .w(px(72.0))
                .flex()
                .justify_center()
                .child(status_tag(status, dark)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_0p5()
                .child(if is_active {
                    action_icon("\u{23f8}", dark)
                        .id(("dl-pause", index))
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().pause_task(&task_id);
                                window.refresh();
                            }
                        })
                        .into_any_element()
                } else if is_paused {
                    action_icon("\u{25b6}", dark)
                        .id(("dl-resume", index))
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().resume_task(&task_id2);
                                window.refresh();
                            }
                        })
                        .into_any_element()
                } else if is_failed {
                    action_icon("\u{21bb}", dark)
                        .id(("dl-retry", index))
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().retry_task(&task_id3);
                                window.refresh();
                            }
                        })
                        .into_any_element()
                } else {
                    div().w(px(22.0)).into_any_element()
                })
                .child(if !is_terminal {
                    action_icon("\u{23f9}", dark)
                        .id(("dl-cancel", index))
                        .on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().cancel_task(&task_id4);
                                window.refresh();
                            }
                        })
                        .into_any_element()
                } else {
                    action_icon("\u{1f5d1}", dark)
                        .id(("dl-delete", index))
                        .on_click({
                            let panel = Rc::clone(&panel);
                            let id = task.id.clone();
                            move |_, window, _cx| {
                                panel.borrow_mut().delete_task(&id);
                                window.refresh();
                            }
                        })
                        .into_any_element()
                })
                .child(if is_completed {
                    action_icon("\u{1f4c2}", dark)
                        .id(("dl-open", index))
                        .on_click({
                            let panel = Rc::clone(&panel);
                            let task = task.clone();
                            move |_, window, _cx| {
                                panel.borrow_mut().open_file(&task);
                                window.refresh();
                            }
                        })
                        .into_any_element()
                } else {
                    div().w(px(22.0)).into_any_element()
                })
                .child(if is_completed {
                    action_icon("\u{1f50d}", dark)
                        .id(("dl-reveal", index))
                        .on_click({
                            let task = task.clone();
                            move |_, window, _cx| {
                                if let Some(parent) = std::path::Path::new(&task.save_path).parent()
                                {
                                    let _ = platform::shell::open_path(parent);
                                }
                                window.refresh();
                            }
                        })
                        .into_any_element()
                } else {
                    div().w(px(22.0)).into_any_element()
                }),
        )
}

fn progress_bar(
    dark: bool,
    job_progress: Option<f64>,
    percent: f64,
    is_active: bool,
) -> impl IntoElement {
    let pct = job_progress
        .map(|progress| progress * 100.0)
        .unwrap_or(percent)
        .clamp(0.0, 100.0);
    let fill = if is_active || pct >= 100.0 {
        theme::rgba_with_alpha(ui::accent_color(PluginAccent::Green), 1.0)
    } else {
        theme::rgba_with_alpha(ui::accent_color(PluginAccent::Green), 0.5)
    };
    div()
        .w_full()
        .h(px(6.0))
        .rounded(px(3.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-subtle", dark),
            0.8,
        ))
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(pct.clamp(0.0, 100.0) as f32 / 100.0))
                .rounded(px(3.0))
                .bg(fill),
        )
}

fn status_tag(status: TaskStatus, dark: bool) -> impl IntoElement {
    let (bg, text) = match status {
        TaskStatus::Completed => (
            theme::rgba_with_alpha(theme::token("color-success", dark), 0.1),
            theme::token("color-success", dark),
        ),
        TaskStatus::Downloading => (
            theme::rgba_with_alpha(ui::accent_color(PluginAccent::Green), 0.1),
            ui::accent_color(PluginAccent::Green),
        ),
        TaskStatus::Pending => (
            theme::rgba_with_alpha(theme::token("color-warning", dark), 0.1),
            theme::token("color-warning", dark),
        ),
        TaskStatus::Paused => (
            theme::rgba_with_alpha(theme::token("color-warning", dark), 0.1),
            theme::token("color-warning", dark),
        ),
        TaskStatus::Failed => (
            theme::rgba_with_alpha(theme::token("color-danger", dark), 0.1),
            theme::token("color-danger", dark),
        ),
        TaskStatus::Cancelled => (
            theme::rgba_with_alpha(theme::launcher_muted_text(dark), 0.1),
            theme::launcher_muted_text(dark),
        ),
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
        .child(status.label())
}

fn bottom_bar(
    dark: bool,
    message: String,
    save_dir: &str,
    settings: &super::model::DownloadSettings,
    stats: &DownloadStats,
    panel: Rc<RefCell<DownloadManagerPanel>>,
) -> impl IntoElement {
    let speed_note = if settings.speed_limit_kbps > 0 {
        format!("  限速 {} KB/s", settings.speed_limit_kbps)
    } else {
        "  不限速".to_string()
    };

    let summary = format!(
        "{} 个任务 · {} 已完成 · {} 进行中 · {} 失败 · 共 {}",
        stats.total,
        stats.completed,
        stats.active,
        stats.failed,
        format_bytes(stats.total_downloaded),
    );

    div()
        .rounded(px(10.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.7,
        ))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .p_3()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .text_color(theme::token("color-text-regular", dark))
                .child(message),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::launcher_faint_text(dark))
                .overflow_hidden()
                .child(format!(
                    "目录: {}  并发 {}{}",
                    truncate_path(save_dir, 32),
                    settings.max_concurrent,
                    speed_note,
                )),
        )
        .child(
            secondary_btn("\u{2699} 设置", dark)
                .id("download-settings")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, _cx| {
                        panel.borrow_mut().toggle_settings();
                        window.refresh();
                    }
                }),
        )
        .child(
            secondary_btn("打开目录", dark)
                .id("download-open-dir")
                .on_click({
                    let panel = Rc::clone(&panel);
                    move |_, window, _cx| {
                        panel.borrow_mut().open_save_dir();
                        window.refresh();
                    }
                }),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::launcher_faint_text(dark))
                .child(summary),
        )
}

fn settings_overlay(
    dark: bool,
    save_root_input: Option<Entity<TextInput>>,
    concurrent_input: Option<Entity<TextInput>>,
    speed_limit_input: Option<Entity<TextInput>>,
    timeout_input: Option<Entity<TextInput>>,
    retry_input: Option<Entity<TextInput>>,
    proxy_input: Option<Entity<TextInput>>,
    user_agent_input: Option<Entity<TextInput>>,
    referer_input: Option<Entity<TextInput>>,
    cookie_input: Option<Entity<TextInput>>,
    headers_input: Option<Entity<TextInput>>,
    panel: Rc<RefCell<DownloadManagerPanel>>,
) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.92,
        ))
        .rounded(px(12.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .p_4()
                .border_b_1()
                .border_color(theme::launcher_soft_line(dark))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::token("color-text-primary", dark))
                        .child("下载设置"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(secondary_btn("保存", dark).id("settings-save").on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, cx| {
                                panel.borrow_mut().save_settings(cx);
                                window.refresh();
                            }
                        }))
                        .child(secondary_btn("取消", dark).id("settings-cancel").on_click({
                            let panel = Rc::clone(&panel);
                            move |_, window, _cx| {
                                panel.borrow_mut().show_settings = false;
                                window.refresh();
                            }
                        })),
                ),
        )
        .child(
            div()
                .id("settings-scroll")
                .flex_1()
                .overflow_y_scroll()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(settings_field(dark, "保存目录", save_root_input))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(settings_field(dark, "并发数 (1-16)", concurrent_input))
                        .child(settings_field(
                            dark,
                            "限速 KB/s (0=不限)",
                            speed_limit_input,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(settings_field(dark, "超时 (秒)", timeout_input))
                        .child(settings_field(dark, "重试次数", retry_input)),
                )
                .child(settings_field(dark, "代理 URL", proxy_input))
                .child(settings_field(dark, "User-Agent", user_agent_input))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(settings_field(dark, "Referer", referer_input))
                        .child(settings_field(dark, "Cookie", cookie_input)),
                )
                .child(settings_field(
                    dark,
                    "自定义请求头 (每行 Key: Value)",
                    headers_input,
                )),
        )
}

fn settings_field(dark: bool, label: &str, input: Option<Entity<TextInput>>) -> gpui::Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::launcher_faint_text(dark))
                .child(label.to_string()),
        )
        .child(
            div()
                .h(px(28.0))
                .rounded(px(6.0))
                .bg(theme::rgba_with_alpha(
                    theme::token("color-bg-subtle", dark),
                    0.65,
                ))
                .border_1()
                .border_color(theme::launcher_soft_line(dark))
                .flex()
                .items_center()
                .children(input.map(|e| e.into_any_element())),
        )
}

// ── Helper Components ──

fn primary_btn(label: &str, accent: PluginAccent, _dark: bool) -> gpui::Div {
    div()
        .h(px(32.0))
        .px_3()
        .rounded(px(8.0))
        .bg(ui::accent_color(accent))
        .hover(|style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::white())
        .child(label.to_string())
}

fn secondary_btn(label: &str, dark: bool) -> gpui::Div {
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
        .hover(|style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::token("color-text-primary", dark))
        .child(label.to_string())
}

fn action_button(label: &str, dark: bool) -> gpui::Div {
    div()
        .h(px(28.0))
        .px_2()
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.88,
        ))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .hover(|style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .text_color(theme::token("color-text-primary", dark))
        .child(label.to_string())
}

fn action_icon(icon: &str, dark: bool) -> gpui::Div {
    div()
        .size(px(22.0))
        .rounded(px(4.0))
        .hover(|style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(theme::launcher_muted_text(dark))
        .child(icon.to_string())
}

fn table_header_cell(label: &str, width: f32) -> gpui::Div {
    div().w(px(width)).child(label.to_string())
}

fn table_header_flex(label: &str, grow: f32) -> gpui::Div {
    let cell = div().child(label.to_string());
    if grow >= 2.0 {
        cell.flex_1()
    } else {
        cell.w(px(96.0))
    }
}

// ── Formatting Helpers ──

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let f = bytes as f64;
    if f >= GB {
        format!("{:.1} GB", f / GB)
    } else if f >= MB {
        format!("{:.1} MB", f / MB)
    } else if f >= KB {
        format!("{:.0} KB", f / KB)
    } else {
        format!("{bytes} B")
    }
}

fn format_speed(bps: f64) -> String {
    if bps <= 0.0 {
        return String::new();
    }
    format!("{}/s", format_bytes(bps as u64))
}

fn format_eta(seconds: Option<u64>) -> String {
    match seconds {
        None => String::new(),
        Some(0) => String::new(),
        Some(s) if s < 60 => format!("{s}s"),
        Some(s) if s < 3600 => format!("{}m{}s", s / 60, s % 60),
        Some(s) => format!("{}h{}m", s / 3600, (s % 3600) / 60),
    }
}

fn format_progress(task: &DownloadTask, include_eta: bool) -> String {
    let progress = if let Some(size) = task.file_size {
        format!("{} / {}", format_bytes(task.downloaded), format_bytes(size))
    } else if task.downloaded > 0 {
        format!("{}", format_bytes(task.downloaded))
    } else {
        String::from("-")
    };

    if !include_eta || !task.status.is_active() {
        return progress;
    }

    let eta = format_eta(task.eta_seconds());
    if eta.is_empty() {
        progress
    } else {
        format!("{progress} · {eta}")
    }
}

fn truncate_url(url: &str, max: usize) -> String {
    if url.len() <= max {
        url.to_string()
    } else {
        format!("{}...", &url[..max.saturating_sub(3)])
    }
}

fn truncate_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else {
        format!(
            "...{}",
            &path[path.len().saturating_sub(max.saturating_sub(3))..]
        )
    }
}
