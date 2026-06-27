//! 托盘配置界面
//!
//! GPUI 内联视图，通过 `Plugin::settings_view()` 暴露。

use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext, Context, ElementId, Entity, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};

use gpui_component::{
    Sizable, Size,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
    slider::{Slider, SliderEvent, SliderState, SliderValue},
    switch::Switch,
    theme::Theme,
};

use qingqi_plugin::plugin::{InlineView, PluginId};
use qingqi_ui::ui::components;
use qingqi_ui::{theme, ui};

use crate::{
    service::NetworkSpeedService,
    settings::{NetworkSpeedDisplayMode, NetworkSpeedTextMode},
};

// ── InlineView 包装器 ──

pub struct SettingsPanel {
    view: Entity<SettingsView>,
    plugin_id: PluginId,
}

impl InlineView for SettingsPanel {
    fn plugin_id(&self) -> PluginId {
        self.plugin_id.clone()
    }

    fn title(&self) -> std::sync::Arc<str> {
        "托盘设置".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }

    fn on_close(&mut self) {}
}

// ── GPUI 视图 ──

pub struct SettingsView {
    service: Arc<NetworkSpeedService>,
    interval_slider: Option<Entity<SliderState>>,
    popup_width_slider: Option<Entity<SliderState>>,
    max_interfaces_slider: Option<Entity<SliderState>>,
    slider_subs: Vec<Subscription>,
    message: String,
}

impl SettingsView {
    pub fn new(service: Arc<NetworkSpeedService>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            service,
            interval_slider: None,
            popup_width_slider: None,
            max_interfaces_slider: None,
            slider_subs: Vec::new(),
            message: String::new(),
        };
        this.ensure_sliders(cx);
        this.attach_slider_subs(cx);
        this
    }

    pub fn new_panel(
        service: Arc<NetworkSpeedService>,
        cx: &mut App,
        plugin_id: PluginId,
    ) -> SettingsPanel {
        let view = cx.new(|cx| SettingsView::new(service, cx));
        SettingsPanel { view, plugin_id }
    }

    fn settings(&self) -> crate::settings::NetworkSpeedSettings {
        self.service.settings()
    }

    // ── Slider 懒初始化 ──

    fn ensure_sliders(&mut self, cx: &mut Context<Self>) {
        let s = self.settings();

        if self.interval_slider.is_none() {
            let slider = cx.new(|_| {
                SliderState::new()
                    .max(5000.0)
                    .min(500.0)
                    .step(500.0)
                    .default_value(s.network_speed_update_interval_ms as f32)
            });
            self.interval_slider = Some(slider);
        }

        if self.popup_width_slider.is_none() {
            let slider = cx.new(|_| {
                SliderState::new()
                    .max(520.0)
                    .min(280.0)
                    .step(20.0)
                    .default_value(s.popup_width as f32)
            });
            self.popup_width_slider = Some(slider);
        }

        if self.max_interfaces_slider.is_none() {
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(10.0)
                    .step(1.0)
                    .default_value(s.network_speed_max_interfaces as f32)
            });
            self.max_interfaces_slider = Some(slider);
        }
    }

    fn attach_slider_subs(&mut self, cx: &mut Context<Self>) {
        self.slider_subs.clear();

        if let Some(slider) = &self.interval_slider {
            let sub = cx.subscribe(slider, |this, _slider, event, cx| {
                if let SliderEvent::Change(SliderValue::Single(value)) = event {
                    let _ = this
                        .service
                        .set_network_speed_update_interval_ms(*value as u64);
                    this.message = format!("刷新间隔已更新为 {} ms", *value as u64);
                    cx.notify();
                }
            });
            self.slider_subs.push(sub);
        }

        if let Some(slider) = &self.popup_width_slider {
            let sub = cx.subscribe(slider, |this, _slider, event, cx| {
                if let SliderEvent::Change(SliderValue::Single(value)) = event {
                    let _ = this.service.set_popup_size(*value as u32, 360);
                    this.message = format!("弹窗宽度已更新为 {} px", *value as u32);
                    cx.notify();
                }
            });
            self.slider_subs.push(sub);
        }

        if let Some(slider) = &self.max_interfaces_slider {
            let sub = cx.subscribe(slider, |this, _slider, event, cx| {
                if let SliderEvent::Change(SliderValue::Single(value)) = event {
                    let _ = this.service.set_network_speed_max_interfaces(*value as u8);
                    this.message = format!("最大网卡数已更新为 {}", *value as u8);
                    cx.notify();
                }
            });
            self.slider_subs.push(sub);
        }
    }

    // ── 辅助 setter ──

    fn set_visible(&mut self, visible: bool) {
        let _ = self.service.set_network_speed_visible(visible);
    }

    fn set_display_mode(&mut self, mode: NetworkSpeedDisplayMode) {
        let _ = self.service.set_network_speed_display_mode(mode);
    }

    fn set_text_mode(&mut self, mode: NetworkSpeedTextMode) {
        let _ = self.service.set_network_speed_text_mode(mode);
    }

    fn set_show_totals(&mut self, show: bool) {
        let _ = self.service.set_network_speed_show_totals(show);
    }

    fn set_show_interfaces(&mut self, show: bool) {
        let _ = self.service.set_network_speed_show_interfaces(show);
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_sliders(cx);
        let s = self.settings();
        let entity = cx.entity();
        let message = self.message.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::global(cx).background)
            .font_family(ui::font_ui())
            .text_color(Theme::global(cx).foreground)
            .child(
                // 页头
                div()
                    .px(theme::space_4())
                    .pt(theme::space_4())
                    .pb(theme::space_2())
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("托盘设置"),
                    )
                    .child(
                        div()
                            .text_size(theme::font_size_caption())
                            .text_color(Theme::global(cx).muted_foreground)
                            .child("配置菜单栏网速显示和详情弹窗"),
                    ),
            )
            .child(
                // 内容区 - 可滚动
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_y_scrollbar()
                    .px(theme::space_4())
                    .pb(theme::space_4())
                    .child(components::settings_card(
                        "菜单栏显示",
                        None::<&str>,
                        div()
                            .flex()
                            .flex_col()
                            .gap_0()
                            .child(components::settings_row(
                                "启用网速显示",
                                "在系统菜单栏显示当前网速",
                                toggle(
                                    entity.clone(),
                                    cx,
                                    "speed-visible",
                                    s.network_speed_visible,
                                    SettingsView::set_visible,
                                ),
                                cx,
                            ))
                            .child(components::settings_row(
                                "显示形式",
                                "文字 / 图标 / 图标+文字",
                                display_mode_segment(
                                    entity.clone(),
                                    s.network_speed_display_mode,
                                    cx,
                                ),
                                cx,
                            ))
                            .child(components::settings_row(
                                "文字内容",
                                "下载 / 上传 / 双向 / 主速优先",
                                text_mode_segment(entity.clone(), s.network_speed_text_mode, cx),
                                cx,
                            )),
                        cx,
                    ))
                    .child(div().h(theme::space_4()))
                    .child(components::settings_card(
                        "详情弹窗",
                        None::<&str>,
                        div()
                            .flex()
                            .flex_col()
                            .gap_0()
                            .child(components::settings_row(
                                "刷新间隔",
                                "采样越频繁越实时",
                                slider_row(
                                    self.interval_slider.as_ref(),
                                    format!("{} ms", s.network_speed_update_interval_ms),
                                    cx,
                                ),
                                cx,
                            ))
                            .child(components::settings_row(
                                "弹窗宽度",
                                "",
                                slider_row(
                                    self.popup_width_slider.as_ref(),
                                    format!("{} px", s.popup_width),
                                    cx,
                                ),
                                cx,
                            ))
                            .child(components::settings_row(
                                "显示总流量",
                                "展示累计接收和发送量",
                                toggle(
                                    entity.clone(),
                                    cx,
                                    "show-totals",
                                    s.network_speed_show_totals,
                                    SettingsView::set_show_totals,
                                ),
                                cx,
                            ))
                            .child(components::settings_row(
                                "显示网卡列表",
                                "展示各活跃网络接口速率",
                                toggle(
                                    entity.clone(),
                                    cx,
                                    "show-interfaces",
                                    s.network_speed_show_interfaces,
                                    SettingsView::set_show_interfaces,
                                ),
                                cx,
                            ))
                            .child(components::settings_row(
                                "最大网卡数",
                                "",
                                slider_row(
                                    self.max_interfaces_slider.as_ref(),
                                    format!("{} 个", s.network_speed_max_interfaces),
                                    cx,
                                ),
                                cx,
                            )),
                        cx,
                    ))
                    .when(!message.is_empty(), |el| {
                        el.child(
                            div()
                                .px(theme::space_4())
                                .py(theme::space_2())
                                .text_size(theme::font_size_caption())
                                .text_color(Theme::global(cx).muted_foreground)
                                .child(message.clone()),
                        )
                    }),
            )
    }
}

// ── 辅助控件 ──

fn toggle(
    entity: Entity<SettingsView>,
    _cx: &App,
    id: &'static str,
    value: bool,
    apply: fn(&mut SettingsView, bool),
) -> impl IntoElement {
    Switch::new(id)
        .checked(value)
        .on_click(move |checked, _window, cx| {
            entity.update(cx, |this, cx| {
                apply(this, *checked);
                cx.notify();
            });
        })
}

fn display_mode_segment(
    entity: Entity<SettingsView>,
    current: NetworkSpeedDisplayMode,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .flex()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded(theme::radius_md())
        .border_1()
        .border_color(t.border)
        .bg(t.muted)
        .child(seg_btn(
            entity.clone(),
            NetworkSpeedDisplayMode::TextOnly,
            current,
            "文字",
            |v, mode| v.set_display_mode(mode),
        ))
        .child(seg_btn(
            entity.clone(),
            NetworkSpeedDisplayMode::IconOnly,
            current,
            "图标",
            |v, mode| v.set_display_mode(mode),
        ))
        .child(seg_btn(
            entity.clone(),
            NetworkSpeedDisplayMode::IconAndText,
            current,
            "图标+文字",
            |v, mode| v.set_display_mode(mode),
        ))
}

fn text_mode_segment(
    entity: Entity<SettingsView>,
    current: NetworkSpeedTextMode,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .flex()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded(theme::radius_md())
        .border_1()
        .border_color(t.border)
        .bg(t.muted)
        .child(seg_btn(
            entity.clone(),
            NetworkSpeedTextMode::DownloadOnly,
            current,
            "下载",
            |v, mode| v.set_text_mode(mode),
        ))
        .child(seg_btn(
            entity.clone(),
            NetworkSpeedTextMode::UploadOnly,
            current,
            "上传",
            |v, mode| v.set_text_mode(mode),
        ))
        .child(seg_btn(
            entity.clone(),
            NetworkSpeedTextMode::Both,
            current,
            "双向",
            |v, mode| v.set_text_mode(mode),
        ))
        .child(seg_btn(
            entity.clone(),
            NetworkSpeedTextMode::Dominant,
            current,
            "主速",
            |v, mode| v.set_text_mode(mode),
        ))
}

fn seg_btn<T: Copy + PartialEq + std::fmt::Debug + 'static>(
    entity: Entity<SettingsView>,
    mode: T,
    current: T,
    label: &'static str,
    apply: fn(&mut SettingsView, T),
) -> impl IntoElement {
    let active = mode == current;
    let id = ElementId::Name(format!("seg-{:?}", mode).into());
    let mut btn = if active {
        Button::new(id).primary().label(label)
    } else {
        Button::new(id).ghost().label(label)
    };
    btn = btn.with_size(Size::XSmall);
    btn.on_click(move |_, _, cx| {
        entity.update(cx, |view, cx| {
            apply(view, mode);
            cx.notify();
        });
    })
}

fn slider_row(
    slider: Option<&Entity<SliderState>>,
    value_label: String,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .w(px(220.0))
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_1()
                .min_w(px(120.0))
                .when_some(slider, |el, s| el.child(Slider::new(s).horizontal())),
        )
        .child(
            div()
                .min_w(px(54.0))
                .text_right()
                .font_family("SF Mono")
                .text_size(theme::font_size_caption())
                .text_color(t.muted_foreground)
                .child(value_label),
        )
}
