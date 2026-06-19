use std::sync::{Arc, Mutex};

use gpui::{
    App, AppContext, AsyncApp, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
    px,
};

use crate::service::{self, JsonMode, JsonResult, JsonStats};
use gpui_component::{scroll::ScrollableElement, theme::Theme};
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
                        height: 320.0,
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
                        height: 32.0,
                        font_size: 12.0,
                        padding: 6.0,
                    },
                    cx,
                );
                input
            }));
        }
        if self.output.is_none() {
            self.output = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "处理结果…", "");
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_read_only(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 400.0,
                        font_size: 12.0,
                        padding: 10.0,
                    },
                    cx,
                );
                input
            }));
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
            output.update(cx, |input, cx| input.set_text(output_text, cx));
        }
        self.status_text = result.status;
        self.error_loc_text = result
            .error
            .as_ref()
            .map(|e| {
                if e.line > 0 {
                    format!("L{}:C{}", e.line, e.column)
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        self.status_tone = if let Some(e) = result.error {
            self.status_text = e.message;
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
            input.update(cx, |input, cx| {
                if input.text() != text {
                    input.set_text(text.to_string(), cx);
                }
            });
        }
        if text.trim().is_empty() {
            self.clear();
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
        let pending = self.pending.lock().ok().and_then(|mut s| s.take());
        if let Some(bg) = pending {
            self.apply_result(bg.result, bg.mode, cx);
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
        self.status_text = "处理中...".into();
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
                if let Ok(mut s) = pending.lock() {
                    *s = Some(JsonBackgroundResult { result, mode });
                }
                let _ = async_cx.refresh();
            })
            .detach();
    }
}

impl Render for JsonView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.collect_pending_result(cx);
        self.ensure_inputs(cx);

        let t = Theme::global(cx);

        let input = self.input.clone();
        let query = self.query.clone();
        let output = self.output.clone();
        let status_text = self.status_text.clone();
        let stats_text = self.stats_text.clone();
        let error_loc_text = self.error_loc_text.clone();
        let last_mode = self.last_mode;
        let status_tone = self.status_tone;
        let panel = cx.entity();

        ui::plugin_surface(cx).child(
            ui::plugin_content()
                .flex()
                .flex_col()
                .gap_2()
                .child(header(&panel, cx))
                // Left-right layout
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .flex()
                        .gap_3()
                        // Left panel: input + query
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_h(px(0.0))
                                        .rounded(px(10.0))
                                        .bg(t.list)
                                        .border_1()
                                        .border_color(ui::border_light(cx))
                                        .overflow_hidden()
                                        .child(input.unwrap()),
                                )
                                .child(query_row(query.unwrap(), &panel, last_mode, cx)),
                        )
                        // Right panel: output
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
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
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(mode_pill(
                                                    "格式化",
                                                    JsonAction::Format,
                                                    &panel,
                                                    last_mode == JsonMode::Format,
                                                    cx,
                                                ))
                                                .child(mode_pill(
                                                    "压缩",
                                                    JsonAction::Compact,
                                                    &panel,
                                                    last_mode == JsonMode::Compact,
                                                    cx,
                                                ))
                                                .child(mode_pill(
                                                    "验证",
                                                    JsonAction::ValidateOnly,
                                                    &panel,
                                                    last_mode == JsonMode::Validate,
                                                    cx,
                                                )),
                                        )
                                        .child(secondary_button(
                                            "复制输出",
                                            JsonAction::CopyOutput,
                                            &panel,
                                            cx,
                                        )),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_h(px(0.0))
                                        .rounded(px(10.0))
                                        .bg(t.list)
                                        .border_1()
                                        .border_color(ui::border_light(cx))
                                        .overflow_hidden()
                                        // Render output as highlighted JSON
                                        .child({
                                            let output_text = output
                                                .as_ref()
                                                .map(|o| o.read(cx).text())
                                                .unwrap_or_default();
                                            if output_text.is_empty() {
                                                div()
                                                    .size_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(12.0))
                                                    .text_color(ui::text_tertiary(cx))
                                                    .child("输出结果")
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .size_full()
                                                    .overflow_y_scrollbar()
                                                    .p(px(10.0))
                                                    .font_family(ui::font_mono())
                                                    .text_size(px(12.0))
                                                    .child(highlight_json(&output_text, cx))
                                                    .into_any_element()
                                            }
                                        }),
                                ),
                        ),
                )
                .child(status_footer(
                    status_text,
                    stats_text,
                    error_loc_text,
                    status_tone,
                    cx,
                )),
        )
    }
}

// ── Header ──

fn header(panel: &Entity<JsonView>, cx: &App) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.foreground)
                .child("JSON 解析"),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(secondary_button("粘贴", JsonAction::PasteInput, panel, cx))
                .child(secondary_button("清空", JsonAction::Clear, panel, cx)),
        )
}

// ── Query Row ──

fn query_row(
    query: Entity<TextInput>,
    panel: &Entity<JsonView>,
    _last_mode: JsonMode,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(ui::text_secondary(cx))
                        .child("JSONPath"),
                )
                .child(query_execute_button("查询", panel, cx)),
        )
        .child(
            div()
                .rounded(px(8.0))
                .bg(t.list)
                .border_1()
                .border_color(ui::border_light(cx))
                .overflow_hidden()
                .child(query),
        )
}

// ── Status Footer ──

fn status_footer(
    status_text: String,
    stats_text: String,
    error_loc_text: String,
    status_tone: StatusTone,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    let status_color = match status_tone {
        StatusTone::Neutral => t.muted_foreground,
        StatusTone::Success => t.success,
        StatusTone::Error => t.danger,
    };
    let tone_icon = match status_tone {
        StatusTone::Success => "✓",
        StatusTone::Error => "✗",
        StatusTone::Neutral => "",
    };
    div()
        .min_h(px(28.0))
        .rounded(px(8.0))
        .bg(t.list)
        .border_1()
        .border_color(ui::border_light(cx))
        .px_3()
        .py_1()
        .flex()
        .items_center()
        .flex_wrap()
        .gap_x_3()
        .gap_y_1()
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
                    .text_color(t.danger)
                    .child(error_loc_text),
            )
        })
        .when(!stats_text.is_empty(), |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .font_family(ui::font_mono())
                    .text_color(ui::text_tertiary(cx))
                    .child(stats_text),
            )
        })
}

// ── JSON Syntax Highlighting ──

fn highlight_json(text: &str, cx: &App) -> impl IntoElement {
    div().flex().flex_col().children(text.lines().map(|line| {
        let segments = tokenize_line(line, cx);
        div().flex().children(
            segments
                .into_iter()
                .map(|(s, color)| div().text_color(color).child(s)),
        )
    }))
}

fn tokenize_line(line: &str, cx: &App) -> Vec<(String, gpui::Hsla)> {
    let mut out = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let t = Theme::global(cx);
    let key_color = t.info;
    let string_color = t.success;
    let number_color = t.warning;
    let bool_null_color = theme::rgba_with_alpha(gpui::rgb(0x8B5CF6), 1.0);
    let punct_color = t.muted_foreground;
    let default_color = t.foreground;

    while i < n {
        let c = chars[i];
        if c == '"' {
            let start = i;
            i += 1;
            while i < n && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < n {
                    i += 1;
                }
                i += 1;
            }
            if i < n {
                i += 1;
            }
            let raw: String = chars[start..i].iter().collect();
            let mut j = i;
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            let is_key = j < n && chars[j] == ':';
            out.push((raw, if is_key { key_color } else { string_color }));
        } else if c == '-' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < n
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'e'
                    || chars[i] == 'E'
                    || chars[i] == '+'
                    || chars[i] == '-')
            {
                i += 1;
            }
            out.push((chars[start..i].iter().collect(), number_color));
        } else if c == 't' && line[i..].starts_with("true") {
            out.push(("true".into(), bool_null_color));
            i += 4;
        } else if c == 'f' && line[i..].starts_with("false") {
            out.push(("false".into(), bool_null_color));
            i += 5;
        } else if c == 'n' && line[i..].starts_with("null") {
            out.push(("null".into(), bool_null_color));
            i += 4;
        } else if c == '{' || c == '}' || c == '[' || c == ']' || c == ':' || c == ',' {
            out.push((c.to_string(), punct_color));
            i += 1;
        } else {
            out.push((c.to_string(), default_color));
            i += 1;
        }
    }
    out
}

// ── Button Helpers ──

fn secondary_button(
    label: &'static str,
    action: JsonAction,
    panel: &Entity<JsonView>,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .id(label)
        .h(px(26.0))
        .px_3()
        .rounded(px(5.0))
        .bg(t.list)
        .border_1()
        .border_color(ui::border_light(cx))
        .hover(move |s| s.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(t.foreground)
        .child(label)
        .on_click({
            let p = panel.clone();
            move |_, w, cx| {
                run_action(action, &p, cx);
                w.refresh();
            }
        })
}

fn mode_pill(
    label: &'static str,
    action: JsonAction,
    panel: &Entity<JsonView>,
    active: bool,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    let accent = theme::accent_color(qingqi_plugin::plugin_spec::PluginAccent::Green);
    let bg = if active {
        theme::rgba_with_alpha(accent, 0.14)
    } else {
        t.list
    };
    let border = if active {
        theme::rgba_with_alpha(accent, 0.24)
    } else {
        ui::border_light(cx)
    };
    let tc = if active {
        theme::rgba_with_alpha(accent, 1.0)
    } else {
        t.foreground
    };
    div()
        .id(label)
        .h(px(24.0))
        .px_2()
        .rounded(px(5.0))
        .bg(bg)
        .border_1()
        .border_color(border)
        .hover(move |s| s.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::default()
        })
        .text_color(tc)
        .child(label)
        .on_click({
            let p = panel.clone();
            move |_, w, cx| {
                run_action(action, &p, cx);
                w.refresh();
            }
        })
}

fn query_execute_button(
    label: &'static str,
    panel: &Entity<JsonView>,
    cx: &App,
) -> impl IntoElement {
    let t = Theme::global(cx);
    div()
        .id(label)
        .h(px(26.0))
        .px_3()
        .rounded(px(5.0))
        .bg(t.primary)
        .hover(move |s| s.bg(t.primary_hover).cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(ui::white())
        .child("▶")
        .child(label)
        .on_click({
            let p = panel.clone();
            move |_, w, cx| {
                run_action(JsonAction::Query, &p, cx);
                w.refresh();
            }
        })
}

// ── Actions ──

fn run_action(action: JsonAction, panel: &Entity<JsonView>, cx: &mut App) {
    match action {
        JsonAction::CopyOutput => {
            let output = panel
                .read(cx)
                .output
                .as_ref()
                .map(|e| e.read(cx).text())
                .unwrap_or_default();
            if output.is_empty() {
                panel.update(cx, |p, _| p.set_status("无可复制内容", StatusTone::Neutral));
                return;
            }
            qingqi_platform::clipboard::write_text(cx, output);
            panel.update(cx, |p, _| {
                p.set_status("已复制到剪贴板", StatusTone::Success)
            });
        }
        JsonAction::PasteInput => {
            let text = qingqi_platform::clipboard::read_text(cx).unwrap_or_default();
            if text.trim().is_empty() {
                panel.update(cx, |p, _| p.set_status("剪贴板为空", StatusTone::Neutral));
                return;
            }
            panel.update(cx, |p, cx| {
                p.ensure_inputs(cx);
                if let Some(i) = p.input.as_ref() {
                    i.update(cx, |i, cx| i.set_text(text.clone(), cx));
                }
            });
            apply_mode(JsonMode::Format, panel, cx);
        }
        JsonAction::Clear => {
            let _ = panel.update(cx, |p, cx| {
                p.clear();
                p.ensure_inputs(cx);
                if let Some(i) = p.input.as_ref() {
                    i.update(cx, |i, cx| i.clear(cx));
                }
                if let Some(q) = p.query.as_ref() {
                    q.update(cx, |q, cx| q.clear(cx));
                }
                if let Some(o) = p.output.as_ref() {
                    o.update(cx, |o, cx| o.clear(cx));
                }
                p.set_status("已清空", StatusTone::Neutral);
            });
        }
        JsonAction::Format => apply_mode(JsonMode::Format, panel, cx),
        JsonAction::Compact => apply_mode(JsonMode::Compact, panel, cx),
        JsonAction::ValidateOnly => apply_mode(JsonMode::Validate, panel, cx),
        JsonAction::Query => apply_mode(JsonMode::Query, panel, cx),
    }
}

fn apply_mode(mode: JsonMode, panel: &Entity<JsonView>, cx: &mut App) {
    panel.update(cx, |p, cx| {
        p.ensure_inputs(cx);
        let input_text = p
            .input
            .as_ref()
            .map(|i| i.read(cx).text())
            .unwrap_or_default();
        let query_text = p
            .query
            .as_ref()
            .map(|i| i.read(cx).text())
            .unwrap_or_default();
        p.run_async(input_text, query_text, mode, cx.to_async());
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
        let r = format_stats(&stats);
        assert!(
            r.contains("object")
                && r.contains("字符 120")
                && r.contains("行 5")
                && r.contains("元素 3")
                && r.contains("深度 2")
        );
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
        let r = format_stats(&stats);
        assert!(r.contains("string") && !r.contains("元素") && !r.contains("深度"));
    }
}
