use std::sync::{Arc, Mutex};

use gpui::{
    App, AppContext, AsyncApp, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::service::{self, JsonMode, JsonResult, JsonStats};
use qingqi_ui::{
    text_input::{TextInput, TextInputStyle},
    theme, ui,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum JsonAction {
    Format,
    Compact,
    ValidateOnly,
    Query,
    CopyOutput,
    PasteInput,
    Clear,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusTone {
    Neutral,
    Success,
    Error,
}

pub struct JsonView {
    input: Option<Entity<TextInput>>,
    query: Option<Entity<TextInput>>,
    output: Option<Entity<TextInput>>,
    status_text: String,
    status_tone: StatusTone,
    stats_text: String,
    error_loc_text: String,
    last_mode: JsonMode,
    pending: Arc<Mutex<Option<JsonBackgroundResult>>>,
    last_output_height: f32,
}

#[derive(Clone)]
struct JsonBackgroundResult {
    result: JsonResult,
    mode: JsonMode,
}

impl JsonView {
    pub fn new() -> Self {
        Self {
            input: None,
            query: None,
            output: None,
            status_text: String::from("就绪"),
            status_tone: StatusTone::Neutral,
            stats_text: String::new(),
            error_loc_text: String::new(),
            last_mode: JsonMode::Format,
            pending: Arc::new(Mutex::new(None)),
            last_output_height: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.status_text = String::from("就绪");
        self.status_tone = StatusTone::Neutral;
        self.stats_text.clear();
        self.error_loc_text.clear();
    }

    fn ensure_inputs(&mut self, cx: &mut Context<Self>) {
        if self.input.is_none() {
            self.input = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "粘贴或输入 JSON…", "");
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 160.0,
                        font_size: 12.0,
                        padding: 10.0,
                    },
                    cx,
                );
                input
            }));
        }

        if self.query.is_none() {
            self.query = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "$.store.book[*].author", "");
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 38.0,
                        font_size: 13.0,
                        padding: 8.0,
                    },
                    cx,
                );
                input
            }));
        }

        if self.output.is_none() {
            let initial_height: f32 = 300.0;
            self.output = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "处理结果…", "");
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_read_only(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: initial_height,
                        font_size: 12.0,
                        padding: 10.0,
                    },
                    cx,
                );
                input
            }));
            self.last_output_height = initial_height;
        }
    }

    fn apply_result(&mut self, result: JsonResult, mode: JsonMode, cx: &mut Context<Self>) {
        self.last_mode = mode;
        let output_text = if result.output.is_empty() {
            String::new()
        } else {
            result.output.clone()
        };
        if let Some(output) = self.output.as_ref() {
            output.update(cx, |input, input_cx| input.set_text(output_text, input_cx));
        }
        self.status_text = result.status;
        self.error_loc_text = result
            .error
            .as_ref()
            .map(|error| {
                if error.line > 0 {
                    format!("L{}:C{}", error.line, error.column)
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        self.status_tone = if let Some(error) = result.error {
            self.status_text = error.message;
            StatusTone::Error
        } else if result.output.is_empty() {
            StatusTone::Neutral
        } else {
            StatusTone::Success
        };
        self.stats_text = result.stats.as_ref().map(format_stats).unwrap_or_default();
    }

    fn set_status(&mut self, text: impl Into<String>, tone: StatusTone) {
        self.status_text = text.into();
        self.status_tone = tone;
    }

    pub fn set_launch_input(&mut self, text: &str, cx: &mut Context<Self>) {
        self.ensure_inputs(cx);
        if let Some(input) = self.input.as_ref() {
            input.update(cx, |input, input_cx| {
                if input.text() != text {
                    input.set_text(text.to_string(), input_cx);
                }
            });
        }
        if text.trim().is_empty() {
            self.clear();
            if let Some(output) = self.output.clone() {
                output.update(cx, |input, input_cx| input.clear(input_cx));
            }
            return;
        }

        self.run_async(
            text.to_string(),
            String::new(),
            JsonMode::Format,
            cx.to_async(),
        );
    }

    fn collect_pending_result(&mut self, cx: &mut Context<Self>) {
        let pending = self.pending.lock().ok().and_then(|mut slot| slot.take());
        if let Some(background) = pending {
            self.apply_result(background.result, background.mode, cx);
        }
    }

    fn run_async(
        &mut self,
        input_text: String,
        query_text: String,
        mode: JsonMode,
        async_cx: AsyncApp,
    ) {
        self.last_mode = mode;
        self.status_text = String::from("处理中...");
        self.status_tone = StatusTone::Neutral;
        self.stats_text.clear();
        self.error_loc_text.clear();

        let pending = Arc::clone(&self.pending);
        async_cx
            .spawn(async move |async_cx| {
                let result = async_cx
                    .background_executor()
                    .spawn(async move { service::run(&input_text, &query_text, mode) })
                    .await;
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(JsonBackgroundResult { result, mode });
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }
}

impl Render for JsonView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.collect_pending_result(cx);
        self.ensure_inputs(cx);

        // Compute output height dynamically so it fills remaining space.
        // Pixels / Pixels = f32, so dividing by px(1.0) extracts the numeric value.
        let window_h: f32 = window.bounds().size.height / px(1.0);
        // Fixed vertical space consumed by header, input, query, status, gaps, padding.
        // Header: ~46px, Input section: ~200px, Query section: ~120px,
        // Status bar: ~36px, internal padding/gaps: ~60px.
        // That's about 462px overhead. The remaining goes to the output.
        let fixed_overhead: f32 = 462.0;
        let computed_output_h = (window_h - fixed_overhead).max(128.0);

        if (computed_output_h - self.last_output_height).abs() > 1.0 {
            self.last_output_height = computed_output_h;
            if let Some(output) = self.output.as_ref() {
                output.update(cx, |input, input_cx| {
                    input.set_style(
                        TextInputStyle {
                            height: computed_output_h,
                            font_size: 12.0,
                            padding: 10.0,
                        },
                        input_cx,
                    );
                });
            }
        }

        let input = self.input.clone();
        let query = self.query.clone();
        let output = self.output.clone();
        let status_text = self.status_text.clone();
        let stats_text = self.stats_text.clone();
        let error_loc_text = self.error_loc_text.clone();
        let last_mode = self.last_mode;
        let status_tone = self.status_tone;
        let panel = cx.entity();

        ui::plugin_surface().child(
            ui::plugin_content()
                .flex()
                .flex_col()
                .gap_3()
                .child(header(&panel))
                .child(input_section(input.unwrap()))
                .child(query_section(query.unwrap(), &panel, last_mode))
                .child(output_section(output.unwrap(), &panel, last_mode))
                .child(status_footer(status_text, stats_text, error_loc_text, status_tone)),
        )
    }
}

// ── Header ──────────────────────────────────────────────────────────────────

fn header(panel: &Entity<JsonView>) -> impl IntoElement {
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
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::semantic().text_primary)
                        .child("📦 JSON 解析"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary())
                        .child("JSON Parser"),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(secondary_button("粘贴", JsonAction::PasteInput, panel))
                .child(secondary_button("清空", JsonAction::Clear, panel)),
        )
}

// ── Input Section ───────────────────────────────────────────────────────────

fn input_section(input: Entity<TextInput>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ui::text_primary())
                .child("输入文本"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .rounded(px(10.0))
                .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.86))
                .border_1()
                .border_color(ui::border_light())
                .overflow_hidden()
                .child(input),
        )
}

// ── Query Section ───────────────────────────────────────────────────────────

fn query_section(
    query: Entity<TextInput>,
    panel: &Entity<JsonView>,
    last_mode: JsonMode,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::text_primary())
                        .child("JSONPath 查询"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(mode_pill("格式化", JsonAction::Format, panel, last_mode == JsonMode::Format))
                        .child(mode_pill("压缩", JsonAction::Compact, panel, last_mode == JsonMode::Compact))
                        .child(mode_pill("验证", JsonAction::ValidateOnly, panel, last_mode == JsonMode::Validate)),
                ),
        )
        .child(
            div()
                .rounded(px(10.0))
                .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.86))
                .border_1()
                .border_color(ui::border_light())
                .overflow_hidden()
                .child(query),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .child(query_execute_button("执行 JSONPath 查询", panel)),
        )
}

// ── Output Section ──────────────────────────────────────────────────────────

fn output_section(
    output: Entity<TextInput>,
    panel: &Entity<JsonView>,
    _last_mode: JsonMode,
) -> impl IntoElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::text_primary())
                        .child("输出结果"),
                )
                .child(secondary_button("复制输出", JsonAction::CopyOutput, panel)),
        )
        .child(
            div()
                .flex_1()
                .w_full()
                .rounded(px(10.0))
                .bg(theme::rgba_with_alpha(theme::semantic().bg_subtle, 0.76))
                .border_1()
                .border_color(ui::border_light())
                .overflow_hidden()
                .child(output),
        )
}

// ── Status Footer ───────────────────────────────────────────────────────────

fn status_footer(
    status_text: String,
    stats_text: String,
    error_loc_text: String,
    status_tone: StatusTone,
) -> impl IntoElement {
    let status_color = match status_tone {
        StatusTone::Neutral => theme::semantic().text_regular,
        StatusTone::Success => theme::semantic().success,
        StatusTone::Error => theme::semantic().danger,
    };

    let tone_icon = match status_tone {
        StatusTone::Success => "✓",
        StatusTone::Error => "✗",
        StatusTone::Neutral => "",
    };

    div()
        .min_h(px(32.0))
        .rounded(px(10.0))
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.7))
        .border_1()
        .border_color(ui::border_light())
        .px_3()
        .py_1p5()
        .flex()
        .items_center()
        .flex_wrap()
        .gap_x_3()
        .gap_y_1()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_tertiary())
                .child("统计"),
        )
        .when(!tone_icon.is_empty(), |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .text_color(status_color)
                    .child(tone_icon),
            )
        })
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .text_color(status_color)
                .child(status_text),
        )
        .when(!error_loc_text.is_empty(), |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .font_family(ui::font_mono())
                    .text_color(theme::semantic().danger)
                    .child(error_loc_text),
            )
        })
        .when(!stats_text.is_empty(), |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .font_family(ui::font_mono())
                    .text_color(ui::text_tertiary())
                    .child(stats_text),
            )
        })
}

// ── Button Helpers ──────────────────────────────────────────────────────────

fn secondary_button(
    label: &'static str,
    action: JsonAction,
    panel: &Entity<JsonView>,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(28.0))
        .px_3()
        .rounded(px(6.0))
        .bg(theme::rgba_with_alpha(theme::semantic().bg_surface, 0.84))
        .border_1()
        .border_color(ui::border_light())
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::semantic().text_primary)
        .child(label)
        .on_click({
            let panel = panel.clone();
            move |_, window, cx| {
                run_action(action, &panel, cx);
                window.refresh();
            }
        })
}

fn mode_pill(
    label: &'static str,
    action: JsonAction,
    panel: &Entity<JsonView>,
    active: bool,
) -> impl IntoElement {
    let accent = theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Green);
    let background = if active {
        theme::rgba_with_alpha(accent, 0.14)
    } else {
        theme::rgba_with_alpha(theme::semantic().bg_surface, 0.84)
    };
    let border = if active {
        theme::rgba_with_alpha(accent, 0.24)
    } else {
        ui::border_light()
    };
    let text_color = if active { accent } else { theme::semantic().text_primary };

    div()
        .id(label)
        .h(px(26.0))
        .px_2()
        .rounded(px(6.0))
        .bg(background)
        .border_1()
        .border_color(border)
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::default() })
        .text_color(text_color)
        .child(label)
        .on_click({
            let panel = panel.clone();
            move |_, window, cx| {
                run_action(action, &panel, cx);
                window.refresh();
            }
        })
}

fn query_execute_button(
    label: &'static str,
    panel: &Entity<JsonView>,
) -> impl IntoElement {
    let accent = theme::blue_500();
    div()
        .id(label)
        .h(px(30.0))
        .px_3()
        .rounded(px(6.0))
        .bg(accent)
        .hover(move |style| style.bg(theme::blue_600()).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::white())
        .child("▶")
        .child(label)
        .on_click({
            let panel = panel.clone();
            move |_, window, cx| {
                run_action(JsonAction::Query, &panel, cx);
                window.refresh();
            }
        })
}

// ── Actions ─────────────────────────────────────────────────────────────────

fn run_action(action: JsonAction, panel: &Entity<JsonView>, cx: &mut App) {
    match action {
        JsonAction::CopyOutput => {
            let output = panel
                .read(cx)
                .output
                .as_ref()
                .map(|entity| entity.read(cx).text())
                .unwrap_or_default();
            if output.is_empty() {
                panel.update(cx, |panel, _cx| {
                    panel.set_status("无可复制内容", StatusTone::Neutral);
                });
                return;
            }
            qingqi_platform::clipboard::write_text(cx, output);
            panel.update(cx, |panel, _cx| {
                panel.set_status("已复制到剪贴板", StatusTone::Success);
            });
        }
        JsonAction::PasteInput => {
            let text = qingqi_platform::clipboard::read_text(cx).unwrap_or_default();
            if text.trim().is_empty() {
                panel.update(cx, |panel, _cx| {
                    panel.set_status("剪贴板为空", StatusTone::Neutral);
                });
                return;
            }
            panel.update(cx, |panel, cx| {
                panel.ensure_inputs(cx);
                if let Some(input) = panel.input.as_ref() {
                    input.update(cx, |input, input_cx| input.set_text(text.clone(), input_cx));
                }
            });
            apply_mode(JsonMode::Format, panel, cx);
        }
        JsonAction::Clear => {
            panel.update(cx, |panel, cx| {
                panel.clear();
                panel.ensure_inputs(cx);
                if let Some(input) = panel.input.as_ref() {
                    input.update(cx, |input, input_cx| input.clear(input_cx));
                }
                if let Some(query) = panel.query.as_ref() {
                    query.update(cx, |input, input_cx| input.clear(input_cx));
                }
                if let Some(output) = panel.output.as_ref() {
                    output.update(cx, |input, input_cx| input.clear(input_cx));
                }
                panel.set_status("已清空", StatusTone::Neutral);
            });
        }
        JsonAction::Format => apply_mode(JsonMode::Format, panel, cx),
        JsonAction::Compact => apply_mode(JsonMode::Compact, panel, cx),
        JsonAction::ValidateOnly => apply_mode(JsonMode::Validate, panel, cx),
        JsonAction::Query => apply_mode(JsonMode::Query, panel, cx),
    }
}

fn apply_mode(mode: JsonMode, panel: &Entity<JsonView>, cx: &mut App) {
    panel.update(cx, |panel, cx| {
        panel.ensure_inputs(cx);
        let input_text = panel
            .input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default();
        let query_text = panel
            .query
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default();
        panel.run_async(input_text, query_text, mode, cx.to_async());
    });
}

fn format_stats(stats: &JsonStats) -> String {
    let mut parts = vec![
        stats.kind.clone(),
        format!("字符 {}", stats.char_count),
        format!("行 {}", stats.line_count),
    ];
    if stats.size > 0 {
        parts.push(format!("元素 {}", stats.size));
    }
    if stats.depth > 0 {
        parts.push(format!("深度 {}", stats.depth));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::format_stats;
    use crate::service::JsonStats;

    #[test]
    fn formats_stats_with_all_fields() {
        let stats = JsonStats {
            char_count: 120,
            line_count: 5,
            kind: "object".into(),
            size: 3,
            depth: 2,
        };
        let result = format_stats(&stats);
        assert!(result.contains("object"));
        assert!(result.contains("字符 120"));
        assert!(result.contains("行 5"));
        assert!(result.contains("元素 3"));
        assert!(result.contains("深度 2"));
    }

    #[test]
    fn formats_stats_without_optional_fields() {
        let stats = JsonStats {
            char_count: 42,
            line_count: 1,
            kind: "string".into(),
            size: 0,
            depth: 0,
        };
        let result = format_stats(&stats);
        assert!(result.contains("string"));
        assert!(!result.contains("元素"));
        assert!(!result.contains("深度"));
    }
}
