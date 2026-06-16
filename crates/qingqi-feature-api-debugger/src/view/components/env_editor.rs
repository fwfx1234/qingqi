use gpui::{
    App, AppContext, Bounds, Context, Entity, IntoElement, InteractiveElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, TitlebarOptions,
    Window, WindowBounds, WindowKind, WindowOptions, div, px, size,
};
use gpui_component::{IconName, Root, Sizable, Size, button::{Button, ButtonVariants}};
use qingqi_ui::{theme, ui, ui::glass};
use qingqi_ui::text_input::TextInput;
use crate::service::EnvDetailTab;
use crate::view::ApiDebuggerView;
use super::shared::{api_accent, circle_badge, section_micro_label};

pub fn open_env_editor_window(debugger: Entity<ApiDebuggerView>, cx: &mut App) {
    if debugger.read(cx).env_editor_window.is_some() {
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(480.0), px(520.0)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from("环境管理")),
            ..Default::default()
        }),
        kind: WindowKind::Normal,
        is_resizable: true,
        window_min_size: Some(size(px(420.0), px(400.0))),
        ..Default::default()
    };
    let inner = debugger.clone();
    match cx.open_window(options, move |window, cx| {
        let editor = cx.new(|cx| EnvEditorWindow::new(inner, window, cx));
        cx.new(|cx| Root::new(editor, window, cx))
    }) {
        Ok(handle) => {
            debugger.update(cx, |view, cx| {
                view.env_editor_window = Some(handle.into());
                cx.notify();
            });
        }
        Err(error) => {
            tracing::warn!(
                target: "qingqi_api_debugger",
                error = %error,
                "打开环境编辑窗口失败"
            );
        }
    }
}

pub struct EnvEditorWindow {
    debugger_view: Entity<ApiDebuggerView>,
    _observe: Subscription,
}

impl EnvEditorWindow {
    pub fn new(
        debugger_view: Entity<ApiDebuggerView>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&debugger_view, |_, _, cx| cx.notify());
        let view = debugger_view.clone();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = view.update(cx, |view, cx| {
                view.env_editor_window = None;
                view.close_env_editor_window(cx);
            });
            true
        });
        Self {
            debugger_view,
            _observe: observe,
        }
    }
}

impl Render for EnvEditorWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.debugger_view.read(cx);
        let dark = qingqi_ui::theme_mode::is_dark();
        let environments = view.environments.clone();
        let selected = view.selected_environment;
        let detail_tab = view.env_detail_tab;
        let name_input = view.env_name_input.clone();
        let base_url_input = view.env_base_url_input.clone();
        let vars_input = view.env_variables_input.clone();
        let headers_input = view.env_headers_input.clone();
        let handle = self.debugger_view.clone();

        let detail_input = if detail_tab == EnvDetailTab::Variables {
            vars_input.clone()
        } else {
            headers_input.clone()
        };

        div()
            .size_full()
            .bg(theme::semantic().bg_elevated)
            .font_family("Inter, PingFang SC")
            .flex()
            .flex_col()
            .child(env_chips_bar(&environments, selected, handle.clone(), dark))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .p(px(14.0))
                    .child(labeled_field("名称", name_input.clone(), dark))
                    .child(labeled_field("Base URL", base_url_input.clone(), dark))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .children(
                                [EnvDetailTab::Variables, EnvDetailTab::Headers]
                                    .into_iter()
                                    .enumerate()
                                    .map({
                                        let tv = handle.clone();
                                        move |(index, tab)| {
                                            let active = tab == detail_tab;
                                            let tv = tv.clone();
                                            div()
                                                .id(("api-env-win-tab", index))
                                                .px(px(12.0))
                                                .py(px(5.0))
                                                .rounded(px(5.0))
                                                .bg(if active {
                                                    theme::rgba_with_alpha(api_accent(), 0.10)
                                                } else {
                                                    gpui::transparent_black()
                                                })
                                                .text_size(px(11.0))
                                                .font_weight(if active {
                                                    gpui::FontWeight::SEMIBOLD
                                                } else {
                                                    gpui::FontWeight::NORMAL
                                                })
                                                .text_color(if active {
                                                    api_accent()
                                                } else {
                                                    ui::text_secondary()
                                                })
                                                .hover(move |style| {
                                                    style
                                                        .bg(theme::rgba_with_alpha(api_accent(), 0.06))
                                                        .cursor_pointer()
                                                })
                                                .child(tab.label())
                                                .on_click(move |_, window, cx| {
                                                    tv.update(cx, |view, _cx| {
                                                        view.env_detail_tab = tab;
                                                    });
                                                    window.refresh();
                                                })
                                        }
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(140.0))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(glass::divider(dark))
                            .bg(glass::inset(dark))
                            .overflow_hidden()
                            .child(detail_input),
                    ),
            )
            .child(env_bottom_bar(handle.clone(), dark))
    }
}

fn env_chips_bar(
    environments: &[crate::service::ApiEnvironment],
    selected_index: usize,
    handle: Entity<ApiDebuggerView>,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .px(px(10.0))
        .py(px(8.0))
        .border_b_1()
        .border_color(glass::divider(dark))
        .bg(glass::bar(dark))
        .flex()
        .items_center()
        .gap(px(6.0))
        .flex_wrap()
        .children({
            let chip_handle = handle.clone();
            environments
                .iter()
                .enumerate()
                .map(move |(i, env)| {
                    let active = i == selected_index;
                    let h = chip_handle.clone();
                    div()
                        .id(("api-env-chip", i))
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(5.0))
                        .bg(if active {
                            theme::rgba_with_alpha(api_accent(), 0.12)
                        } else {
                            gpui::transparent_black()
                        })
                        .border_1()
                        .border_color(if active {
                            theme::rgba_with_alpha(api_accent(), 0.24)
                        } else {
                            gpui::transparent_black()
                        })
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .hover(move |style| {
                            if !active {
                                style.bg(ui::bg_hover()).cursor_pointer()
                            } else {
                                style.cursor_pointer()
                            }
                        })
                        .child(circle_badge(&env.badge, env.color, 16.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(if active {
                                    gpui::FontWeight::SEMIBOLD
                                } else {
                                    gpui::FontWeight::NORMAL
                                })
                                .text_color(if active {
                                    api_accent()
                                } else {
                                    ui::text_secondary()
                                })
                                .child(env.name.clone()),
                        )
                        .on_click(move |_, _, cx| {
                            h.update(cx, |view, cx| view.select_environment(i, cx));
                        })
                })
        })
        .child(div().flex_1())
        .child(
            Button::new("api-env-win-new")
                .ghost()
                .icon(IconName::Plus)
                .with_size(Size::XSmall)
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, _cx| view.create_new_environment());
                    }
                }),
        )
}

fn env_bottom_bar(
    handle: Entity<ApiDebuggerView>,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .h(px(40.0))
        .px(px(12.0))
        .border_t_1()
        .border_color(glass::divider(dark))
        .bg(glass::bar(dark))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            Button::new("api-env-win-save")
                .primary()
                .label("保存更改")
                .with_size(Size::XSmall)
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, cx| {
                            view.save_environment_changes(cx);
                            view.close_env_editor_window(cx);
                        });
                    }
                }),
        )
        .child(
            Button::new("api-env-win-reset")
                .ghost()
                .label("重置")
                .with_size(Size::XSmall)
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, cx| {
                            view.reset_environment_changes(cx);
                        });
                    }
                }),
        )
        .child(
            Button::new("api-env-win-dup")
                .ghost()
                .label("复制")
                .with_size(Size::XSmall)
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, cx| {
                            view.duplicate_current_environment(cx);
                        });
                    }
                }),
        )
        .child(
            Button::new("api-env-win-del")
                .ghost()
                .label("删除")
                .with_size(Size::XSmall)
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, cx| {
                            view.delete_current_environment(cx);
                        });
                    }
                }),
        )
        .child(div().flex_1())
        .child(
            Button::new("api-env-win-export")
                .ghost()
                .icon(IconName::File)
                .with_size(Size::XSmall)
                .tooltip("导出")
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, _cx| view.export_environments());
                    }
                }),
        )
        .child(
            Button::new("api-env-win-import")
                .ghost()
                .icon(IconName::FolderOpen)
                .with_size(Size::XSmall)
                .tooltip("导入")
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, _cx| view.import_environments());
                    }
                }),
        )
}

fn labeled_field(
    label: &'static str,
    input: Entity<TextInput>,
    dark: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(section_micro_label(label, dark))
        .child(
            div()
                .h(px(32.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(glass::divider(dark))
                .bg(glass::inset(dark))
                .overflow_hidden()
                .child(input),
        )
}
