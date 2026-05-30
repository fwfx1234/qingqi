use gpui::{
    AnyElement, App, AppContext, Component, Entity, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, px,
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

use crate::{
    app::{theme, ui},
    core::{
        icon::IconRef,
        plugin::{InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView},
        plugin_spec::{
            PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
            PluginWindowMode, WindowSpec,
        },
    },
};

pub struct GpuiDemoPlugin;

impl GpuiDemoPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn manifest_static() -> Manifest {
        Manifest {
            id: "gpui-demo".into(),
            name: "GPUI 学习演示".into(),
            description: "GPUI 组件、布局和交互的 Rust 实验场".into(),
            keywords: ["gpui", "rust", "学习", "demo", "组件", "演示", "教程"]
                .into_iter()
                .map(Into::into)
                .collect(),
            icon: IconRef::asset("qta/mdi6.school-outline.png"),
            prefixes: vec!["gpui".into(), "demo".into()],
            mode: PluginWindowMode::Inline,
            window: WindowSpec::ratio(0.8, 0.8),
            category: PluginCategory::Tool,
            status: PluginStatus::Preview,
            background: false,
            dynamic_commands: false,
            visual: Some(PluginVisualSpec {
                icon: IconRef::asset("qta/mdi6.school-outline.png"),
                accent: PluginAccent::Purple,
                category: PluginCategory::Tool,
                status: PluginStatus::Preview,
                mode: PluginWindowMode::Inline,
                window: WindowSpec::ratio(0.8, 0.8),
            }),
            stats: Some(PluginStats {
                primary: "控件范式".into(),
                secondary: "布局样例".into(),
                tertiary: "持续沉淀".into(),
            }),
            command_hint: Some("用于沉淀 Qingqi 的 GPUI 组件、布局和交互范式".into()),
            command_prefixes: ["gpui", "demo"].into_iter().map(Into::into).collect(),
        }
    }
}

impl Plugin for GpuiDemoPlugin {
    fn manifest(&self) -> Manifest {
        Self::manifest_static()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let slider = cx.app.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(65.0)
        });
        Ok(PluginView::Inline(Box::new(GpuiDemoView {
            active_tab: 0,
            dark_switch: false,
            checkbox_checked: true,
            slider,
        })))
    }
}

struct GpuiDemoView {
    active_tab: usize,
    dark_switch: bool,
    checkbox_checked: bool,
    slider: Entity<SliderState>,
}

impl InlineView for GpuiDemoView {
    fn plugin_id(&self) -> PluginId {
        "gpui-demo".into()
    }

    fn title(&self) -> Arc<str> {
        "GPUI 学习演示".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        GpuiDemoPage {
            active_tab: self.active_tab,
            dark_switch: self.dark_switch,
            checkbox_checked: self.checkbox_checked,
            slider: self.slider.clone(),
        }
        .into_any_element()
    }
}

struct GpuiDemoPage {
    active_tab: usize,
    dark_switch: bool,
    checkbox_checked: bool,
    slider: Entity<SliderState>,
}

impl IntoElement for GpuiDemoPage {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for GpuiDemoPage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dark = crate::app::theme_mode::is_dark();
        let accent = PluginAccent::Purple;

        div()
            .size_full()
            .bg(theme::semantic(dark).bg_page)
            .font_family(ui::font_ui())
            .text_color(theme::semantic(dark).text_primary)
            .flex()
            .flex_col()
            .p_4()
            .gap_3()
            .child(header(dark, accent))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap_3()
                    .overflow_hidden()
                    .child(component_column(
                        dark,
                        self.dark_switch,
                        self.checkbox_checked,
                        self.slider,
                        cx,
                    ))
                    .child(layout_column(dark, self.active_tab))
                    .child(state_column(dark, accent)),
            )
            .child(ui::status_bar(
                "gpui-component 组件演示 — 点击按钮、切换 tab、调整 slider 查看交互效果",
                theme::semantic(dark).text_secondary,
            ))
    }
}

fn header(dark: bool, accent: PluginAccent) -> impl IntoElement {
    div()
        .rounded(theme::radius_lg())
        .bg(theme::semantic(dark).bg_surface)
        .border_1()
        .border_color(theme::semantic(dark).border_default)
        .p_4()
        .flex()
        .items_center()
        .gap_3()
        .child(ui::icon_tile(
            "qta/mdi6.school-outline.png",
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
                        .text_color(theme::semantic(dark).text_regular)
                        .child("用 gpui-component 真实组件替代静态描述，验证按钮、标签页、开关等控件的交互行为。"),
                ),
        )
        .child(ui::status_pill("预览", PluginStatus::Preview))
}

fn component_column(
    dark: bool,
    dark_switch: bool,
    checkbox_checked: bool,
    slider: Entity<SliderState>,
    cx: &App,
) -> impl IntoElement {
    panel(dark, "基础控件")
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic(dark).text_secondary)
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
                .text_color(theme::semantic(dark).text_secondary)
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
                .text_color(theme::semantic(dark).text_secondary)
                .child(format!(
                    "Slider — {}",
                    slider.read(cx).value().start() as i32
                )),
        )
        .child(Slider::new(&slider).horizontal())
}

fn layout_column(dark: bool, active_tab: usize) -> impl IntoElement {
    let tabs = vec!["概览", "设置", "历史"];

    panel(dark, "布局模式")
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::semantic(dark).text_secondary)
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
                .bg(theme::semantic(dark).bg_subtle_2)
                .border_1()
                .border_color(theme::semantic(dark).border_default)
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
                .text_color(theme::semantic(dark).text_secondary)
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
                .border_color(theme::semantic(dark).border_default)
                .overflow_hidden()
                .child(sample_strip(dark, "左侧导航", 0.28))
                .child(sample_strip(dark, "中间内容", 0.42))
                .child(sample_strip(dark, "右侧详情", 0.30)),
        )
}

fn state_column(dark: bool, accent: PluginAccent) -> impl IntoElement {
    panel(dark, "状态与服务")
        .child(demo_row(
            dark,
            accent,
            "Runtime",
            "长生命周期，持有 service、cache、background handle",
        ))
        .child(demo_row(
            dark,
            accent,
            "Session",
            "窗口生命周期，只持有 UI entity state",
        ))
        .child(demo_row(
            dark,
            accent,
            "Service",
            "可单测业务逻辑，不依赖 GPUI",
        ))
        .child(demo_row(
            dark,
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

fn panel(dark: bool, title: &'static str) -> gpui::Div {
    div()
        .flex_1()
        .rounded(theme::radius_lg())
        .bg(theme::semantic(dark).bg_surface)
        .border_1()
        .border_color(theme::semantic(dark).border_default)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::accent_color(theme::ThemeAccent::Purple))
                .child(title),
        )
}

fn demo_row(
    dark: bool,
    accent: PluginAccent,
    title: &'static str,
    body: &'static str,
) -> impl IntoElement {
    div()
        .rounded(theme::radius_md())
        .bg(theme::semantic(dark).bg_subtle_2)
        .border_1()
        .border_color(theme::semantic(dark).border_default)
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
                        .text_color(theme::semantic(dark).text_regular)
                        .child(body),
                ),
        )
}

fn sample_strip(dark: bool, label: &'static str, width_ratio: f32) -> impl IntoElement {
    let width = 240.0 * width_ratio;
    div()
        .h(px(34.0))
        .w(px(width))
        .bg(theme::semantic(dark).bg_subtle)
        .border_b_1()
        .border_color(theme::semantic(dark).border_default)
        .flex()
        .items_center()
        .px_2()
        .text_size(px(11.0))
        .text_color(theme::semantic(dark).text_secondary)
        .child(SharedString::from(label))
}
