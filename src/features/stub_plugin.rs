use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, rgb};

use crate::{
    app::{events::AppEventBus, ui},
    core::{
        plugin::{PluginManifest, PluginRuntime, PluginSession},
        plugin_spec::{PluginOverviewSection, PluginStats},
    },
};

pub struct StubPluginRuntime {
    manifest: PluginManifest,
    hero: &'static str,
    sections: &'static [(&'static str, &'static str)],
}

impl StubPluginRuntime {
    pub fn new(
        manifest: PluginManifest,
        hero: &'static str,
        sections: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            manifest,
            hero,
            sections,
        }
    }
}

impl PluginRuntime for StubPluginRuntime {
    fn manifest(&self) -> PluginManifest {
        self.manifest
    }

    fn open_session(
        &mut self,
        _: AppEventBus,
        _: &mut App,
    ) -> anyhow::Result<Box<dyn PluginSession>> {
        Ok(Box::new(StubPluginSession {
            manifest: self.manifest,
            hero: self.hero,
            sections: self
                .sections
                .iter()
                .map(|(title, body)| PluginOverviewSection::new(*title, *body))
                .collect(),
        }))
    }
}

struct StubPluginSession {
    manifest: PluginManifest,
    hero: &'static str,
    sections: Vec<PluginOverviewSection>,
}

impl PluginSession for StubPluginSession {
    fn plugin_id(&self) -> &'static str {
        self.manifest.id
    }

    fn title(&self) -> &'static str {
        self.manifest.name
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        StubPluginPage {
            manifest: self.manifest,
            hero: self.hero,
            sections: self.sections.clone(),
        }
        .into_any_element()
    }
}

struct StubPluginPage {
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
        let visual = self.manifest.visual;
        let stats: PluginStats = self.manifest.stats;
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
                    .child(ui::icon_tile(visual.icon, visual.accent, 52.0))
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
                                        self.manifest.name,
                                        self.manifest.description,
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
                                        stats.secondary,
                                        visual.accent,
                                    ))
                                    .child(ui::stat_card(
                                        "实现策略",
                                        stats.tertiary,
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
                format!("{}: {}", self.manifest.name, self.manifest.command_hint),
                rgb(0x475569),
            ))
    }
}
