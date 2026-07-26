use std::{sync::Arc, time::Duration};

use gpui::{
    AnyElement, Context, ElementId, IntoElement, ParentElement, Render, SharedString, Styled, Task,
    div, prelude::FluentBuilder as _,
};
use qingqi_platform::display::DisplayModeKey;
use qingqi_ui::{
    components::{
        Button, ButtonVariant, ButtonVariants, Disableable, Selectable, Sizable, Size, Tag,
    },
    icon, theme, ui,
};

use crate::{
    model::{HidpiPreset, ManagedDisplay, OptimizerSnapshot, OptimizerStatus},
    service::DisplayOptimizerService,
};

#[derive(Clone, Debug, Default)]
struct DisplayOptimizerViewModel {
    snapshot: Arc<OptimizerSnapshot>,
    notice: Option<SharedString>,
    notice_is_error: bool,
}

#[derive(Clone, Copy)]
struct PendingRevert {
    display_id: u32,
    previous_mode: DisplayModeKey,
    remaining_seconds: u8,
}

pub struct DisplayOptimizerView {
    service: Arc<DisplayOptimizerService>,
    vm: DisplayOptimizerViewModel,
    selected_key: Option<String>,
    selected_preset: HidpiPreset,
    loading: bool,
    confirm_uninstall: bool,
    generation: u64,
    action_generation: u64,
    countdown_generation: u64,
    pending_revert: Option<PendingRevert>,
    reload_task: Option<Task<()>>,
    action_task: Option<Task<()>>,
    countdown_task: Option<Task<()>>,
}

impl DisplayOptimizerView {
    pub fn new(service: Arc<DisplayOptimizerService>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            service,
            vm: DisplayOptimizerViewModel::default(),
            selected_key: None,
            selected_preset: HidpiPreset::Recommended,
            loading: false,
            confirm_uninstall: false,
            generation: 0,
            action_generation: 0,
            countdown_generation: 0,
            pending_revert: None,
            reload_task: None,
            action_task: None,
            countdown_task: None,
        };
        view.refresh(cx);
        view
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let service = Arc::clone(&self.service);
        self.reload_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.snapshot() })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                if view.generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(snapshot) => {
                        let previous = view.selected_key.as_deref();
                        let selected_exists = previous.is_some_and(|key| {
                            snapshot
                                .displays
                                .iter()
                                .any(|display| display.identity_key() == key)
                        });
                        if !selected_exists {
                            view.selected_key =
                                snapshot.displays.first().map(ManagedDisplay::identity_key);
                        }
                        view.vm.snapshot = Arc::new(snapshot);
                    }
                    Err(error) => {
                        view.vm.notice = Some(format!("检测显示器失败: {error}").into());
                        view.vm.notice_is_error = true;
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn selected_display(&self) -> Option<&ManagedDisplay> {
        let key = self.selected_key.as_deref()?;
        self.vm
            .snapshot
            .displays
            .iter()
            .find(|display| display.identity_key() == key)
    }

    fn select_display(&mut self, key: String, cx: &mut Context<Self>) {
        self.selected_key = Some(key);
        self.confirm_uninstall = false;
        self.vm.notice = None;
        cx.notify();
    }

    fn select_preset(&mut self, preset: HidpiPreset, cx: &mut Context<Self>) {
        self.selected_preset = preset;
        self.vm.notice = None;
        cx.notify();
    }

    fn install_selected(&mut self, cx: &mut Context<Self>) {
        let Some(display_id) = self.selected_display().and_then(ManagedDisplay::display_id) else {
            self.set_error("目标显示器已断开", cx);
            return;
        };
        self.loading = true;
        self.vm.notice = Some("正在请求管理员授权...".into());
        self.action_generation = self.action_generation.wrapping_add(1);
        let generation = self.action_generation;
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.install(display_id) })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                if view.action_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(()) => {
                        view.vm.notice = Some("HiDPI 配置已安装，重启 Mac 后生效".into());
                        view.vm.notice_is_error = false;
                        view.refresh(cx);
                    }
                    Err(error) => view.set_error(format!("安装失败: {error}"), cx),
                }
            });
        }));
        cx.notify();
    }

    fn request_uninstall(&mut self, cx: &mut Context<Self>) {
        self.confirm_uninstall = true;
        self.vm.notice = Some("恢复后需要重启，确认移除该型号的 HiDPI 配置".into());
        self.vm.notice_is_error = false;
        cx.notify();
    }

    fn cancel_uninstall(&mut self, cx: &mut Context<Self>) {
        self.confirm_uninstall = false;
        self.vm.notice = None;
        cx.notify();
    }

    fn uninstall_selected(&mut self, cx: &mut Context<Self>) {
        let Some(display) = self.selected_display() else {
            self.set_error("没有可恢复的显示器配置", cx);
            return;
        };
        let (vendor_id, product_id) = (display.vendor_id, display.product_id);
        self.loading = true;
        self.confirm_uninstall = false;
        self.vm.notice = Some("正在请求管理员授权...".into());
        self.action_generation = self.action_generation.wrapping_add(1);
        let generation = self.action_generation;
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.uninstall(vendor_id, product_id) })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                if view.action_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(()) => {
                        view.vm.notice = Some("原始显示配置已恢复，重启 Mac 后生效".into());
                        view.vm.notice_is_error = false;
                        view.refresh(cx);
                    }
                    Err(error) => view.set_error(format!("恢复失败: {error}"), cx),
                }
            });
        }));
        cx.notify();
    }

    fn apply_selected_mode(&mut self, cx: &mut Context<Self>) {
        let Some(display) = self.selected_display() else {
            self.set_error("没有可用的显示器", cx);
            return;
        };
        let Some(display_id) = display.display_id() else {
            self.set_error("目标显示器已断开", cx);
            return;
        };
        let Some(previous_mode) = display.current_mode().map(|mode| mode.key) else {
            self.set_error("无法读取当前显示模式", cx);
            return;
        };
        let Some(requested) = display.mode_for_preset(self.selected_preset) else {
            self.set_error("该 HiDPI 模式尚未生效，请先重启 Mac", cx);
            return;
        };
        self.loading = true;
        self.vm.notice = Some("正在切换显示模式...".into());
        self.action_generation = self.action_generation.wrapping_add(1);
        let generation = self.action_generation;
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(async move { service.apply_mode(display_id, requested) })
                .await;
            let _ = view.update(async_cx, |view, cx| {
                if view.action_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(()) => {
                        view.pending_revert = Some(PendingRevert {
                            display_id,
                            previous_mode,
                            remaining_seconds: 15,
                        });
                        view.start_revert_countdown(cx);
                    }
                    Err(error) => view.set_error(format!("切换失败: {error}"), cx),
                }
            });
        }));
        cx.notify();
    }

    fn start_revert_countdown(&mut self, cx: &mut Context<Self>) {
        self.countdown_generation = self.countdown_generation.wrapping_add(1);
        let generation = self.countdown_generation;
        let service = Arc::clone(&self.service);
        self.countdown_task = Some(cx.spawn(async move |view, async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_secs(1))
                    .await;
                let decision = view.update(async_cx, |view, cx| {
                    if view.countdown_generation != generation {
                        return None;
                    }
                    let pending = view.pending_revert.as_mut()?;
                    pending.remaining_seconds = pending.remaining_seconds.saturating_sub(1);
                    if pending.remaining_seconds == 0 {
                        let expired = *pending;
                        view.pending_revert = None;
                        cx.notify();
                        Some(expired)
                    } else {
                        cx.notify();
                        None
                    }
                });
                match decision {
                    Ok(Some(expired)) => {
                        let result = async_cx
                            .background_executor()
                            .spawn(async move {
                                service.restore_mode(expired.display_id, expired.previous_mode)
                            })
                            .await;
                        let _ = view.update(async_cx, |view, cx| {
                            match result {
                                Ok(()) => {
                                    view.vm.notice = Some("未确认新模式，已自动恢复".into());
                                    view.vm.notice_is_error = false;
                                }
                                Err(error) => {
                                    view.vm.notice = Some(format!("自动恢复失败: {error}").into());
                                    view.vm.notice_is_error = true;
                                }
                            }
                            view.refresh(cx);
                        });
                        break;
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        }));
        cx.notify();
    }

    fn keep_mode(&mut self, cx: &mut Context<Self>) {
        self.countdown_generation = self.countdown_generation.wrapping_add(1);
        self.pending_revert = None;
        self.vm.notice = Some("已保留新的 HiDPI 模式".into());
        self.vm.notice_is_error = false;
        self.refresh(cx);
    }

    fn restore_now(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_revert.take() else {
            return;
        };
        self.countdown_generation = self.countdown_generation.wrapping_add(1);
        self.loading = true;
        self.action_generation = self.action_generation.wrapping_add(1);
        let generation = self.action_generation;
        let service = Arc::clone(&self.service);
        self.action_task = Some(cx.spawn(async move |view, async_cx| {
            let result = async_cx
                .background_executor()
                .spawn(
                    async move { service.restore_mode(pending.display_id, pending.previous_mode) },
                )
                .await;
            let _ = view.update(async_cx, |view, cx| {
                if view.action_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(()) => {
                        view.vm.notice = Some("已恢复先前显示模式".into());
                        view.vm.notice_is_error = false;
                        view.refresh(cx);
                    }
                    Err(error) => view.set_error(format!("恢复失败: {error}"), cx),
                }
            });
        }));
        cx.notify();
    }

    fn set_error(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.loading = false;
        self.vm.notice = Some(message.into());
        self.vm.notice_is_error = true;
        cx.notify();
    }

    fn render_display_selector(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.selected_key.as_deref();
        let mut row = div().flex().flex_wrap().gap_2();
        for display in &self.vm.snapshot.displays {
            let key = display.identity_key();
            let label = display.name.clone();
            let click_key = key.clone();
            row = row.child(
                Button::new(ElementId::Name(
                    format!("display-optimizer-display-{key}").into(),
                ))
                .label(label)
                .with_size(Size::Small)
                .selected(selected == Some(key.as_str()))
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.select_display(click_key.clone(), cx);
                })),
            );
        }
        row.into_any_element()
    }

    fn render_selected_display(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(display) = self.selected_display() else {
            return ui::section_card(cx)
                .p_4()
                .text_color(ui::text_secondary(cx))
                .child("没有检测到原生 2560×1440 的外接显示器")
                .into_any_element();
        };
        let current = display.current_mode().map_or_else(
            || "未连接".to_string(),
            |mode| {
                format!(
                    "{}×{} · {:.0} Hz{}",
                    mode.key.width,
                    mode.key.height,
                    f64::from(mode.key.refresh_millihz) / 1000.0,
                    if mode.is_hidpi() { " · HiDPI" } else { "" }
                )
            },
        );
        let status_tag = match display.status {
            OptimizerStatus::Active => Tag::success(),
            OptimizerStatus::PendingRestart => Tag::warning(),
            OptimizerStatus::Conflict | OptimizerStatus::Unsupported => Tag::danger(),
            OptimizerStatus::NotInstalled | OptimizerStatus::Disconnected => Tag::secondary(),
        }
        .small()
        .child(display.status.label());

        ui::section_card(cx)
            .p_4()
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
                            .child(
                                div()
                                    .text_size(theme::font_size_body())
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(ui::text_primary(cx))
                                    .child(display.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(theme::font_size_caption())
                                    .text_color(ui::text_secondary(cx))
                                    .child(current),
                            ),
                    )
                    .child(status_tag),
            )
            .child(
                div()
                    .text_size(theme::font_size_caption())
                    .text_color(ui::text_tertiary(cx))
                    .child(format!(
                        "Vendor {:04x} · Product {:04x} · 同型号显示器会共用此配置",
                        display.vendor_id, display.product_id
                    )),
            )
            .into_any_element()
    }

    fn render_presets(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected_display = self.selected_display();
        let can_select = selected_display.is_some_and(|display| {
            display.status == OptimizerStatus::Active && display.display_id().is_some()
        });
        let mut row = div().flex().flex_wrap().gap_2();
        for preset in HidpiPreset::ALL {
            let (width, height) = preset.logical_size();
            let available = selected_display
                .and_then(|display| display.mode_for_preset(preset))
                .is_some();
            row = row.child(
                Button::new(ElementId::Name(
                    format!("display-optimizer-preset-{}", preset.label()).into(),
                ))
                .label(format!("{}  {}×{}", preset.label(), width, height))
                .with_size(Size::Small)
                .selected(self.selected_preset == preset)
                .disabled(!can_select || !available || self.loading)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.select_preset(preset, cx);
                })),
            );
        }
        row.into_any_element()
    }

    fn render_uninstall_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.confirm_uninstall {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("display-optimizer-uninstall-confirm")
                        .label("确认恢复")
                        .with_size(Size::Small)
                        .with_variant(ButtonVariant::Danger)
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.uninstall_selected(cx);
                        })),
                )
                .child(
                    Button::new("display-optimizer-uninstall-cancel")
                        .label("取消")
                        .with_size(Size::Small)
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.cancel_uninstall(cx);
                        })),
                )
                .into_any_element()
        } else {
            Button::new("display-optimizer-uninstall")
                .label("恢复系统默认")
                .with_size(Size::Small)
                .with_variant(ButtonVariant::Danger)
                .outline()
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.request_uninstall(cx);
                }))
                .into_any_element()
        }
    }

    fn render_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(display) = self.selected_display() else {
            return div().into_any_element();
        };
        if let Some(pending) = self.pending_revert {
            return div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(ui::warning(cx))
                        .child(format!("{} 秒后自动恢复", pending.remaining_seconds)),
                )
                .child(
                    Button::new("display-optimizer-keep")
                        .label("保留")
                        .with_size(Size::Small)
                        .with_variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|view, _, _, cx| view.keep_mode(cx))),
                )
                .child(
                    Button::new("display-optimizer-revert")
                        .label("恢复")
                        .with_size(Size::Small)
                        .on_click(cx.listener(|view, _, _, cx| view.restore_now(cx))),
                )
                .into_any_element();
        }

        let mut row = div().flex().items_center().gap_2();
        match display.status {
            OptimizerStatus::NotInstalled => {
                row = row.child(
                    Button::new("display-optimizer-install")
                        .label("启用 HiDPI")
                        .icon(icon!(monitor_up))
                        .with_size(Size::Small)
                        .with_variant(ButtonVariant::Primary)
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| view.install_selected(cx))),
                );
            }
            OptimizerStatus::Active => {
                row = row.child(
                    Button::new("display-optimizer-apply")
                        .label("应用所选模式")
                        .with_size(Size::Small)
                        .with_variant(ButtonVariant::Primary)
                        .disabled(
                            self.loading || display.mode_for_preset(self.selected_preset).is_none(),
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.apply_selected_mode(cx))),
                );
                if display.is_managed {
                    row = row.child(self.render_uninstall_controls(cx));
                }
            }
            OptimizerStatus::PendingRestart => {
                row = row.child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(ui::warning(cx))
                        .child("重启 Mac 后即可选择 HiDPI 模式"),
                );
                if display.is_managed {
                    row = row.child(self.render_uninstall_controls(cx));
                }
            }
            OptimizerStatus::Conflict => {
                row = row.child(
                    div()
                        .text_size(theme::font_size_caption())
                        .text_color(ui::danger(cx))
                        .child("系统配置已被其他工具修改，为避免覆盖已停止自动操作"),
                );
            }
            OptimizerStatus::Unsupported => {
                row = row.child("需要 macOS 12.4 或更高版本");
                if display.is_managed {
                    row = row.child(self.render_uninstall_controls(cx));
                }
            }
            OptimizerStatus::Disconnected => {
                row = row.child("连接该显示器后可继续管理");
                if display.is_managed {
                    row = row.child(self.render_uninstall_controls(cx));
                }
            }
        }
        row.into_any_element()
    }
}

impl Render for DisplayOptimizerView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notice = self.vm.notice.clone();
        let notice_color = if self.vm.notice_is_error {
            ui::danger(cx)
        } else {
            ui::success(cx)
        };

        div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(ui::page_title(
                        "外接屏优化",
                        "为原生 2560×1440 显示器启用 HiDPI 渲染",
                        cx,
                    ))
                    .child(
                        Button::new("display-optimizer-refresh")
                            .icon(icon!(refresh_cw))
                            .tooltip("重新检测显示器")
                            .with_size(Size::Small)
                            .loading(self.loading)
                            .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))),
                    ),
            )
            .when(!self.vm.snapshot.displays.is_empty(), |root| {
                root.child(self.render_display_selector(cx))
            })
            .child(self.render_selected_display(cx))
            .child(
                ui::section_card(cx)
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_size(theme::font_size_body())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ui::text_primary(cx))
                            .child("HiDPI 模式"),
                    )
                    .child(self.render_presets(cx))
                    .child(self.render_actions(cx)),
            )
            .when_some(notice, |root, notice| {
                root.child(ui::status_bar(notice, notice_color, cx))
            })
    }
}
