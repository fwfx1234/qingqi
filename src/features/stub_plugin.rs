use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, rgb};

use crate::{
    app::ui,
    core::{
        plugin::{
            InlineView, ListItem, ListView, Plugin, PluginCx, PluginManifest, PluginView,
            WindowView,
        },
        plugin_spec::{PluginOverviewSection, PluginStats, PluginWindowMode},
    },
};

pub struct StubPluginRuntime {
    id: &'static str,
    title: &'static str,
    manifest: PluginManifest,
    hero: &'static str,
    sections: &'static [(&'static str, &'static str)],
}

impl StubPluginRuntime {
    pub fn new(
        id: &'static str,
        title: &'static str,
        manifest: PluginManifest,
        hero: &'static str,
        sections: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            id,
            title,
            manifest,
            hero,
            sections,
        }
    }
}

impl Plugin for StubPluginRuntime {
    fn manifest(&self) -> PluginManifest {
        self.manifest.clone()
    }

    fn open(&mut self, _: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let mode = self.manifest.visual.mode;
        let view = StubPluginView {
            id: self.id,
            title: self.title,
            manifest: self.manifest.clone(),
            hero: self.hero,
            sections: self
                .sections
                .iter()
                .map(|(title, body)| PluginOverviewSection::new(*title, *body))
                .collect(),
        };
        Ok(match mode {
            PluginWindowMode::Inline => PluginView::Inline(Box::new(view)),
            PluginWindowMode::List => PluginView::List(Box::new(view)),
            PluginWindowMode::Window => PluginView::Window(Box::new(view)),
        })
    }
}

struct StubPluginView {
    id: &'static str,
    title: &'static str,
    manifest: PluginManifest,
    hero: &'static str,
    sections: Vec<PluginOverviewSection>,
}

impl WindowView for StubPluginView {
    fn plugin_id(&self) -> &str {
        self.id
    }

    fn title(&self) -> &str {
        self.title
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        StubPluginPage {
            id: self.id,
            title: self.title,
            manifest: self.manifest.clone(),
            hero: self.hero,
            sections: self.sections.clone(),
        }
        .into_any_element()
    }
}

impl InlineView for StubPluginView {
    fn plugin_id(&self) -> &str {
        self.id
    }

    fn title(&self) -> &str {
        self.title
    }

    fn render(&mut self, window: &mut Window, cx: &mut App) -> AnyElement {
        WindowView::render(self, window, cx)
    }
}

impl ListView for StubPluginView {
    fn plugin_id(&self) -> &str {
        self.id
    }

    fn title(&self) -> &str {
        self.title
    }

    fn items(&mut self, _cx: &mut App) -> Vec<ListItem> {
        Vec::new()
    }
}

struct StubPluginPage {
    id: &'static str,
    title: &'static str,
    manifest: PluginManifest,
    hero: &'static str,
    sections: Vec<PluginOverviewSection>,
}

impl IntoElement for StubPluginPage {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

impl RenderOnce for StubPluginPage {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let visual = self.manifest.visual.clone();
        let stats: PluginStats = self.manifest.stats.clone();
        let accent = ui::accent_color(visual.accent);

        div()
            .size_full()
            .bg(ui::bg_canvas())
            .font_family("PingFang SC")
            .text_color(ui::text_primary())
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .rounded(px(12.0))
                    .bg(ui::bg_surface())
                    .border_1()
                    .border_color(ui::border_light())
                    .p_4()
                    .flex()
                    .gap_3()
                    .child(ui::icon_tile(visual.icon.as_str(), visual.accent, 52.0))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(ui::page_title(
                                        self.manifest.name.to_string(),
                                        self.manifest.description.to_string(),
                                    ))
                                    .child(ui::status_pill(visual.status.label(), visual.status))
                                    .child(ui::category_pill(
                                        visual.category.label(),
                                        visual.category,
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .line_height(px(20.0))
                                    .text_color(ui::text_secondary())
                                    .child(self.hero),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(ui::stat_card("主能力", stats.primary, visual.accent))
                                    .child(ui::stat_card(
                                        "交互模式",
                                        visual.mode.label(),
                                        visual.accent,
                                    ))
                                    .child(ui::stat_card(
                                        "补全状态",
                                        stats.secondary.to_string(),
                                        visual.accent,
                                    ))
                                    .child(ui::stat_card(
                                        "实现策略",
                                        stats.tertiary.to_string(),
                                        visual.accent,
                                    )),
                            ),
                    ),
            )
            .children(self.sections.into_iter().map(|section| {
                div()
                    .rounded(px(12.0))
                    .bg(ui::bg_surface())
                    .border_1()
                    .border_color(ui::border_light())
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(accent)
                            .child(section.title),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .line_height(px(20.0))
                            .text_color(ui::text_secondary())
                            .child(section.body),
                    )
            }))
            .child(ui::status_bar(
                format!(
                    "{}: {}",
                    self.manifest.name.as_ref(),
                    self.manifest.command_hint.as_ref()
                ),
                rgb(0x475569),
            ))
    }
}
