use gpui::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div, px,
};
use gpui_component::{
    IconName, Sizable,
    badge::Badge,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    slider::{Slider, SliderState},
    switch::Switch,
    tab::TabBar,
};
use std::sync::Arc;

use crate::manifest;
use qingqi_plugin::{
    plugin::{InlineView, Plugin, PluginCx, PluginId, PluginView},
    plugin_spec::PluginAccent,
};
use qingqi_ui::{theme, ui, ui::components};

pub struct GpuiDemoPlugin;

impl GpuiDemoPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for GpuiDemoPlugin {
    fn manifest(&self) -> qingqi_plugin::plugin::Manifest {
        manifest::manifest()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let panel = cx.app.new(|cx| {
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(100.0)
                    .step(1.0)
                    .default_value(65.0)
            });
            GpuiDemoView {
                active_tab: 0,
                dark_switch: false,
                checkbox_checked: true,
                slider,
            }
        });
        Ok(PluginView::Inline(Box::new(GpuiDemoInline { panel })))
    }
}

struct GpuiDemoInline {
    panel: Entity<GpuiDemoView>,
}

impl InlineView for GpuiDemoInline {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "GPUI 学习演示".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.panel.clone().into_any_element()
    }
}

struct GpuiDemoView {
    active_tab: usize,
    dark_switch: bool,
    checkbox_checked: bool,
    slider: Entity<SliderState>,
}

impl Render for GpuiDemoView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = PluginAccent::Purple;

        // Pre-read slider value so helper functions do not need App
        let slider_value = self.slider.read(cx).value().start() as i32;

        div()
            .size_full()
            .bg(theme::semantic().bg_page)
            .font_family(ui::font_ui())
            .text_color(theme::semantic().text_primary)
            .flex()
            .flex_col()
            .p_4()
            .gap_3()
            .child(header(accent))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap_3()
                    .overflow_hidden()
                    .child(component_column(
                        self.dark_switch,
                        self.checkbox_checked,
                        slider_value,
                        &self.slider,
                    ))
                    .child(layout_column(self.active_tab))
                    .child(state_column(accent)),
            )
            .child(ui::status_bar(
                "gpui-component 组件演示 — 点击按钮、切换 tab、调整 slider 查看交互效果",
                theme::semantic().text_secondary,
            ))
    }
}

fn header(accent: PluginAccent) -> impl IntoElement {
    div()
        .rounded(theme::radius_lg())
        .bg(theme::semantic().bg_surface)
        .border_1()
        .border_color(theme::semantic().border_default)
        .p_4()
        .flex()
        .items_center()
        .gap_3()
        .child(ui::icon_tile(
            "icons/school.svg",
            accent,
            52.0,
        ))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(20.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("GPUI 学习演示"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(theme::semantic().text_regular)
                        .child("用 gpui-component 真实组件替代静态描述，验证按钮、标签页、开关等控件的交互行为。"),
                ),
        )
        .child(components::status_pill(
            "预览",
            components::StatusTone::Warning,
        ))
}

fn component_column(
    dark_switch: bool,
    checkbox_checked: bool,
    slider_value: i32,
    slider: &Entity<SliderState>,
) -> impl IntoElement {
    panel("基础控件")
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_secondary)
                .child("Button 变体"),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .flex_wrap()
                .child(Button::new("demo-primary").label("Primary").primary())
                .child(Button::new("demo-secondary").label("Secondary"))
                .child(Button::new("demo-danger").label("Danger").danger())
                .child(Button::new("demo-ghost").label("Ghost").ghost()),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .flex_wrap()
                .child(
                    Button::new("demo-icon-search")
                        .icon(IconName::Search)
                        .ghost()
                        .small(),
                )
                .child(
                    Badge::new()
                        .count(3)
                        .child(Button::new("demo-badge-btn").label("通知").small()),
                )
                .child(
                    Badge::new()
                        .dot()
                        .child(Button::new("demo-dot-btn").label("状态").small()),
                ),
        )
        .child(
            div()
                .mt_1()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_secondary)
                .child("Switch / Checkbox"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Switch::new("demo-switch")
                        .label("暗色模式")
                        .checked(dark_switch),
                )
                .child(
                    Checkbox::new("demo-checkbox")
                        .label("启用通知")
                        .checked(checkbox_checked),
                ),
        )
        .child(
            div()
                .mt_1()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_secondary)
                .child(format!("Slider — {}", slider_value)),
        )
        .child(Slider::new(slider).horizontal())
}

fn layout_column(active_tab: usize) -> impl IntoElement {
    let tabs = vec!["概览", "设置", "历史"];

    panel("布局模式")
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_secondary)
                .child("TabBar — Underline"),
        )
        .child(
            TabBar::new("demo-tabs")
                .underline()
                .children(tabs.clone())
                .selected_index(active_tab),
        )
        .child(
            div()
                .rounded(theme::radius_md())
                .bg(theme::semantic().bg_subtle_2)
                .border_1()
                .border_color(theme::semantic().border_default)
                .p_3()
                .text_size(px(13.0))
                .child(format!(
                    "当前标签: {}",
                    tabs[active_tab.min(tabs.len() - 1)]
                )),
        )
        .child(
            div()
                .mt_1()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic().text_secondary)
                .child("TabBar — Segmented"),
        )
        .child(
            TabBar::new("demo-tabs-seg")
                .segmented()
                .children(vec!["全部", "收藏", "最近"])
                .selected_index(0),
        )
        .child(
            div()
                .mt_1()
                .rounded(theme::radius_md())
                .border_1()
                .border_color(theme::semantic().border_default)
                .overflow_hidden()
                .child(sample_strip("左侧导航", 0.28))
                .child(sample_strip("中间内容", 0.42))
                .child(sample_strip("右侧详情", 0.30)),
        )
}

fn state_column(accent: PluginAccent) -> impl IntoElement {
    panel("状态与服务")
        .child(demo_row(
            accent,
            "Runtime",
            "长生命周期，持有 service、cache、background handle",
        ))
        .child(demo_row(
            accent,
            "Session",
            "窗口生命周期，只持有 UI entity state",
        ))
        .child(demo_row(accent, "Service", "可单测业务逻辑，不依赖 GPUI"))
        .child(demo_row(
            accent,
            "Store",
            "持久化和分页查询，锁短、连接可控",
        ))
        .child(
            div()
                .mt_2()
                .flex()
                .gap_2()
                .child(Button::new("demo-outline-btn").label("Outline").outline())
                .child(Button::new("demo-success-btn").label("Success").success())
                .child(Button::new("demo-warning-btn").label("Warning").warning()),
        )
}

fn panel(title: &'static str) -> gpui::Div {
    div()
        .flex_1()
        .rounded(theme::radius_lg())
        .bg(theme::semantic().bg_surface)
        .border_1()
        .border_color(theme::semantic().border_default)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::accent_color(
                    qingqi_plugin::plugin_spec::PluginAccent::Purple,
                ))
                .child(title),
        )
}

fn demo_row(accent: PluginAccent, title: &'static str, body: &'static str) -> impl IntoElement {
    div()
        .rounded(theme::radius_md())
        .bg(theme::semantic().bg_subtle_2)
        .border_1()
        .border_color(theme::semantic().border_default)
        .p_3()
        .flex()
        .gap_2()
        .child(
            div()
                .size(px(28.0))
                .rounded(theme::radius_sm())
                .bg(ui::accent_soft(accent))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(ui::accent_color(accent))
                .child(title.chars().next().unwrap_or('G').to_string()),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .line_height(px(16.0))
                        .text_color(theme::semantic().text_regular)
                        .child(body),
                ),
        )
}

fn sample_strip(label: &'static str, width_ratio: f32) -> impl IntoElement {
    let width = 240.0 * width_ratio;
    div()
        .h(px(34.0))
        .w(px(width))
        .bg(theme::semantic().bg_subtle)
        .border_b_1()
        .border_color(theme::semantic().border_default)
        .flex()
        .items_center()
        .px_2()
        .text_size(px(11.0))
        .text_color(theme::semantic().text_secondary)
        .child(SharedString::from(label))
}
