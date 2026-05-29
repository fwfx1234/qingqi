use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use gpui::{
    AnyElement, App, AppContext, AsyncApp, Component, Entity, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::{
    app::{
        text_input::{TextInput, TextInputStyle},
        theme, ui,
    },
    features::json_parser::service::{self, JsonMode, JsonResult, JsonStats},
    platform,
};

const STACKED_LAYOUT_BREAKPOINT_PX: f32 = 860.0;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum JsonLayoutMode {
    SideBySide,
    Stacked,
}

pub struct JsonPanel {
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

impl JsonPanel {
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

    fn ensure_inputs(&mut self, cx: &mut App) {
        if self.input.is_none() {
            self.input = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "粘贴或输入 JSON 内容", "");
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 248.0,
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
                let mut input = TextInput::new(cx, "$.foo.bar / .foo.bar / /foo/bar", "");
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 32.0,
                        font_size: 12.0,
                        padding: 8.0,
                    },
                    cx,
                );
                input
            }));
        }

        if self.output.is_none() {
            self.output = Some(cx.new(|cx| {
                let mut input = TextInput::new(cx, "等待操作...", "");
                input.set_multiline(true, cx);
                input.set_monospace(true, cx);
                input.set_read_only(true, cx);
                input.set_chrome(false, cx);
                input.set_style(
                    TextInputStyle {
                        height: 248.0,
                        font_size: 12.0,
                        padding: 10.0,
                    },
                    cx,
                );
                input
            }));
        }
    }

    fn apply_result(&mut self, result: JsonResult, mode: JsonMode, cx: &mut App) {
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

    pub fn set_launch_input(&mut self, text: &str, cx: &mut App) {
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

    fn collect_pending_result(&mut self, cx: &mut App) {
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

pub struct JsonParserElement {
    pub panel: Rc<RefCell<JsonPanel>>,
}

impl IntoElement for JsonParserElement {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for JsonParserElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        {
            let mut panel = self.panel.borrow_mut();
            panel.collect_pending_result(cx);
            panel.ensure_inputs(cx);
        }
        let panel = self.panel.borrow();
        let input = panel.input.clone();
        let query = panel.query.clone();
        let output = panel.output.clone();
        let status_text = panel.status_text.clone();
        let stats_text = panel.stats_text.clone();
        let error_loc_text = panel.error_loc_text.clone();
        let last_mode = panel.last_mode;
        let status_tone = panel.status_tone;
        drop(panel);

        let dark = crate::app::theme_mode::is_dark();
        let layout_mode = json_layout_mode(window.bounds().size.width);

        ui::plugin_surface(dark).child(
            ui::plugin_scroll_content()
                .flex()
                .flex_col()
                .gap_3()
                .child(module_header(dark))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(mode_button(
                            "格式化",
                            JsonAction::Format,
                            Rc::clone(&self.panel),
                            dark,
                            last_mode == JsonMode::Format,
                        ))
                        .child(mode_button(
                            "压缩",
                            JsonAction::Compact,
                            Rc::clone(&self.panel),
                            dark,
                            last_mode == JsonMode::Compact,
                        ))
                        .child(mode_button(
                            "验证",
                            JsonAction::ValidateOnly,
                            Rc::clone(&self.panel),
                            dark,
                            last_mode == JsonMode::Validate,
                        ))
                        .child(mode_button(
                            "执行查询",
                            JsonAction::Query,
                            Rc::clone(&self.panel),
                            dark,
                            last_mode == JsonMode::Query,
                        ))
                        .child(toolbar_button(
                            "复制输出",
                            JsonAction::CopyOutput,
                            Rc::clone(&self.panel),
                            dark,
                        ))
                        .child(toolbar_button(
                            "从剪贴板填充",
                            JsonAction::PasteInput,
                            Rc::clone(&self.panel),
                            dark,
                        ))
                        .child(div().flex_1())
                        .child(toolbar_button(
                            "清空",
                            JsonAction::Clear,
                            Rc::clone(&self.panel),
                            dark,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(74.0))
                                .text_size(px(11.0))
                                .text_color(theme::launcher_muted_text(dark))
                                .child("JSONPath"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .h(px(34.0))
                                .rounded(px(10.0))
                                .bg(theme::rgba_with_alpha(
                                    theme::token("color-bg-surface", dark),
                                    0.86,
                                ))
                                .border_1()
                                .border_color(theme::launcher_soft_line(dark))
                                .child(query.unwrap().into_any_element()),
                        ),
                )
                .child(editor_split(
                    layout_mode,
                    input.unwrap().into_any_element(),
                    output.unwrap(),
                    dark,
                    last_mode,
                ))
                .child(status_bar(
                    status_text,
                    stats_text,
                    error_loc_text,
                    status_tone,
                    dark,
                )),
        )
    }
}

fn json_layout_mode(width: gpui::Pixels) -> JsonLayoutMode {
    if width < px(STACKED_LAYOUT_BREAKPOINT_PX) {
        JsonLayoutMode::Stacked
    } else {
        JsonLayoutMode::SideBySide
    }
}

fn editor_split(
    layout_mode: JsonLayoutMode,
    input: AnyElement,
    output: Entity<TextInput>,
    dark: bool,
    last_mode: JsonMode,
) -> impl IntoElement {
    let split = div().flex_1().w_full().flex().gap_3();
    let split = match layout_mode {
        JsonLayoutMode::SideBySide => split,
        JsonLayoutMode::Stacked => split.flex_col(),
    };

    split
        .child(editor_pane(
            "输入",
            theme::token("color-bg-surface", dark),
            input,
            dark,
            layout_mode,
        ))
        .child(output_pane(output, dark, last_mode, layout_mode))
}

fn editor_pane(
    title: &'static str,
    background: gpui::Rgba,
    content: impl IntoElement,
    dark: bool,
    layout_mode: JsonLayoutMode,
) -> impl IntoElement {
    div()
        .flex_1()
        .w_full()
        .when(layout_mode == JsonLayoutMode::Stacked, |pane| {
            pane.min_h(px(220.0))
        })
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::launcher_muted_text(dark))
                .child(title),
        )
        .child(
            div()
                .flex_1()
                .rounded(px(12.0))
                .bg(theme::rgba_with_alpha(background, 0.86))
                .border_1()
                .border_color(theme::launcher_soft_line(dark))
                .child(content),
        )
}

fn output_pane(
    output: Entity<TextInput>,
    dark: bool,
    last_mode: JsonMode,
    layout_mode: JsonLayoutMode,
) -> impl IntoElement {
    div()
        .flex_1()
        .w_full()
        .when(layout_mode == JsonLayoutMode::Stacked, |pane| {
            pane.min_h(px(220.0))
        })
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme::launcher_muted_text(dark))
                .child(output_title(last_mode)),
        )
        .child(
            div()
                .flex_1()
                .rounded(px(12.0))
                .bg(theme::rgba_with_alpha(
                    theme::token("color-bg-subtle", dark),
                    0.76,
                ))
                .border_1()
                .border_color(theme::launcher_soft_line(dark))
                .child(output),
        )
}

fn status_bar(
    status_text: String,
    stats_text: String,
    error_loc_text: String,
    status_tone: StatusTone,
    dark: bool,
) -> impl IntoElement {
    let status_color = match status_tone {
        StatusTone::Neutral => theme::token("color-text-regular", dark),
        StatusTone::Success => theme::token("color-success", dark),
        StatusTone::Error => theme::token("color-danger", dark),
    };

    div()
        .h(px(32.0))
        .rounded(px(10.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.7,
        ))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .px_3()
        .flex()
        .items_center()
        .gap_3()
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
                    .font_family("SF Mono")
                    .text_color(theme::token("color-danger", dark))
                    .child(error_loc_text),
            )
        })
        .when(!stats_text.is_empty(), |bar| {
            bar.child(
                div()
                    .text_size(px(11.0))
                    .font_family("SF Mono")
                    .text_color(theme::launcher_faint_text(dark))
                    .child(stats_text),
            )
        })
}

fn output_title(mode: JsonMode) -> &'static str {
    match mode {
        JsonMode::Compact => "输出 (压缩)",
        JsonMode::Query => "输出 (查询结果)",
        JsonMode::Validate => "输出 (验证)",
        JsonMode::Format => "输出 (格式化)",
    }
}

fn mode_button(
    label: &'static str,
    action: JsonAction,
    panel: Rc<RefCell<JsonPanel>>,
    dark: bool,
    active: bool,
) -> impl IntoElement {
    let background = if active {
        theme::rgba_with_alpha(theme::accent_color(theme::ThemeAccent::Green), 0.14)
    } else {
        theme::rgba_with_alpha(theme::token("color-bg-surface", dark), 0.84)
    };
    let border = if active {
        theme::rgba_with_alpha(theme::accent_color(theme::ThemeAccent::Green), 0.24)
    } else {
        theme::launcher_soft_line(dark)
    };
    let text_color = if active {
        theme::accent_color(theme::ThemeAccent::Green)
    } else {
        theme::token("color-text-primary", dark)
    };

    div()
        .id(label)
        .h(px(30.0))
        .px_3()
        .rounded(px(8.0))
        .bg(background)
        .border_1()
        .border_color(border)
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(text_color)
        .child(label)
        .on_click(move |_, window, cx| {
            run_action(action, &panel, cx);
            window.refresh();
        })
}

fn toolbar_button(
    label: &'static str,
    action: JsonAction,
    panel: Rc<RefCell<JsonPanel>>,
    dark: bool,
) -> impl IntoElement {
    div()
        .id(label)
        .h(px(30.0))
        .px_3()
        .rounded(px(8.0))
        .bg(theme::rgba_with_alpha(
            theme::token("color-bg-surface", dark),
            0.84,
        ))
        .border_1()
        .border_color(theme::launcher_soft_line(dark))
        .hover(move |style| style.cursor_pointer())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(theme::token("color-text-primary", dark))
        .child(label)
        .on_click(move |_, window, cx| {
            run_action(action, &panel, cx);
            window.refresh();
        })
}

fn module_header(dark: bool) -> impl IntoElement {
    div().flex().items_center().justify_between().child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::token("color-text-primary", dark))
                    .child("📦 JSON 解析"),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::launcher_muted_text(dark))
                    .child("JSON Parser"),
            ),
    )
}

fn run_action(action: JsonAction, panel: &Rc<RefCell<JsonPanel>>, cx: &mut App) {
    match action {
        JsonAction::CopyOutput => {
            let output = panel
                .borrow()
                .output
                .as_ref()
                .map(|entity| entity.read(cx).text())
                .unwrap_or_default();
            if output.is_empty() {
                panel
                    .borrow_mut()
                    .set_status("无可复制内容", StatusTone::Neutral);
                return;
            }
            platform::clipboard::write_text(cx, output);
            panel
                .borrow_mut()
                .set_status("已复制到剪贴板", StatusTone::Success);
        }
        JsonAction::PasteInput => {
            let text = platform::clipboard::read_text(cx).unwrap_or_default();
            if text.trim().is_empty() {
                panel
                    .borrow_mut()
                    .set_status("剪贴板为空", StatusTone::Neutral);
                return;
            }
            let input_entity = {
                let mut state = panel.borrow_mut();
                state.ensure_inputs(cx);
                state.input.clone()
            };
            if let Some(input) = input_entity {
                input.update(cx, |input, input_cx| input.set_text(text.clone(), input_cx));
            }
            apply_mode(JsonMode::Format, panel, cx);
        }
        JsonAction::Clear => {
            let (input_entity, query_entity) = {
                let mut state = panel.borrow_mut();
                state.clear();
                state.ensure_inputs(cx);
                (state.input.clone(), state.query.clone())
            };
            if let Some(input) = input_entity {
                input.update(cx, |input, input_cx| input.clear(input_cx));
            }
            if let Some(query) = query_entity {
                query.update(cx, |input, input_cx| input.clear(input_cx));
            }
            if let Some(output) = panel.borrow().output.clone() {
                output.update(cx, |input, input_cx| input.clear(input_cx));
            }
            panel.borrow_mut().set_status("已清空", StatusTone::Neutral);
        }
        JsonAction::Format => apply_mode(JsonMode::Format, panel, cx),
        JsonAction::Compact => apply_mode(JsonMode::Compact, panel, cx),
        JsonAction::ValidateOnly => apply_mode(JsonMode::Validate, panel, cx),
        JsonAction::Query => apply_mode(JsonMode::Query, panel, cx),
    }
}

fn apply_mode(mode: JsonMode, panel: &Rc<RefCell<JsonPanel>>, cx: &mut App) {
    let (input_text, query_text) = {
        let mut state = panel.borrow_mut();
        state.ensure_inputs(cx);
        let input_text = state
            .input
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default();
        let query_text = state
            .query
            .as_ref()
            .map(|input| input.read(cx).text())
            .unwrap_or_default();
        (input_text, query_text)
    };

    panel
        .borrow_mut()
        .run_async(input_text, query_text, mode, cx.to_async());
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
    use super::{JsonLayoutMode, json_layout_mode, output_title};
    use crate::features::json_parser::service::JsonMode;
    use gpui::px;

    #[test]
    fn uses_side_by_side_layout_above_breakpoint() {
        assert_eq!(json_layout_mode(px(980.0)), JsonLayoutMode::SideBySide);
        assert_eq!(json_layout_mode(px(860.0)), JsonLayoutMode::SideBySide);
    }

    #[test]
    fn uses_stacked_layout_below_breakpoint() {
        assert_eq!(json_layout_mode(px(859.0)), JsonLayoutMode::Stacked);
        assert_eq!(json_layout_mode(px(640.0)), JsonLayoutMode::Stacked);
    }

    #[test]
    fn output_titles_follow_mode_labels() {
        assert_eq!(output_title(JsonMode::Format), "输出 (格式化)");
        assert_eq!(output_title(JsonMode::Compact), "输出 (压缩)");
        assert_eq!(output_title(JsonMode::Validate), "输出 (验证)");
        assert_eq!(output_title(JsonMode::Query), "输出 (查询结果)");
    }
}
