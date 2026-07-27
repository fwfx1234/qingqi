use super::shared::{api_accent, circle_badge};
use crate::service::EnvDetailTab;
use crate::view::ApiDebuggerView;
use gpui::{
    App, AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, TitlebarOptions,
    Window, WindowBounds, WindowKind, WindowOptions, div, px, size,
};
use qingqi_ui::components::{
    IconName, Root, Sizable, Size,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    theme::Theme,
};
use qingqi_ui::{theme, ui, ui::glass};

pub fn open_env_editor_window(debugger: Entity<ApiDebuggerView>, cx: &mut App) {
    if debugger.read(cx).env_editor_window.is_some() {
        return;
    }

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(560.0), px(520.0)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from("环境管理")),
            ..Default::default()
        }),
        kind: WindowKind::Normal,
        is_resizable: true,
        window_min_size: Some(size(px(480.0), px(420.0))),
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
                "打开环境编辑窗口失败",
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

        let current_env = environments.get(selected)
            .cloned()
            .unwrap_or_else(|| crate::service::ApiEnvironment {
                name: String::from("默认环境"),
                badge: String::from("默"),
                color: 0x338855,
                base_url: String::from("http://127.0.0.1:8000"),
                variables: Vec::new(),
                headers: Vec::new(),
            });
        let app: &App = cx;

        div()
            .size_full()
            .bg(Theme::global(app).popover)
            .font_family(".SystemUIFont")
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .gap(px(14.0))
                    .p(px(16.0))
                    .child(env_list_sidebar(&environments, selected, handle.clone(), app))
                    .child(
                        div()
                            .flex_1()
                            .rounded(px(20.0))
                            .border_1()
                            .border_color(glass::divider(app))
                            .bg(glass::bar(app))
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            .p(px(18.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .flex_col()
                                            .gap(px(4.0))
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(ui::text_primary(app))
                                                    .child("环境配置"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(ui::text_secondary(app))
                                                    .child("编辑当前环境的基本信息与共享设置"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                Button::new("api-env-win-dup-header")
                                                    .ghost()
                                                    .icon(IconName::Copy)
                                                    .with_size(Size::XSmall)
                                                    .tooltip("复制当前环境")
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
                                                Button::new("api-env-win-del-header")
                                                    .ghost()
                                                    .icon(IconName::Delete)
                                                    .with_size(Size::XSmall)
                                                    .tooltip("删除当前环境")
                                                    .on_click({
                                                        let h = handle.clone();
                                                        move |_, _, cx| {
                                                            h.update(cx, |view, cx| {
                                                                view.delete_current_environment(cx);
                                                            });
                                                        }
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(12.0))
                                    .child(compact_field("环境名称", name_input.clone(), app))
                                    .child(compact_field("Base URL", base_url_input.clone(), app)),
                            )
                            .child(
                                div()
                                    .rounded(px(16.0))
                                    .bg(glass::inset(app))
                                    .border_1()
                                    .border_color(glass::divider(app))
                                    .p(px(14.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(circle_badge(&current_env.badge, current_env.color, 16.0))
                                            .child(
                                                div()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                                            .text_color(ui::text_primary(app))
                                                            .child(current_env.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(10.0))
                                                            .text_color(ui::text_secondary(app))
                                                            .child(current_env.base_url.clone()),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(ui::text_secondary(app))
                                            .child("在请求面板中自动使用选中环境的 Base URL、变量和公共 Headers。"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .bg(ui::bg_surface(app))
                                    .rounded(px(10.0))
                                    .p(px(4.0))
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
                                                        .flex_1()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .px(px(12.0))
                                                        .py(px(8.0))
                                                        .rounded(px(8.0))
                                                        .bg(if active {
                                                            ui::bg_surface(app)
                                                        } else {
                                                            Theme::global(app).list
                                                        })
                                                        .text_size(px(11.0))
                                                        .font_weight(if active {
                                                            gpui::FontWeight::SEMIBOLD
                                                        } else {
                                                            gpui::FontWeight::NORMAL
                                                        })
                                                        .text_color(if active {
                                                            ui::text_primary(app)
                                                        } else {
                                                            ui::text_secondary(app)
                                                        })
                                                        .hover(move |mut style| {
                                                            if !active {
                                                                style = style.bg(theme::rgba_with_alpha(
                                                                    ui::text_secondary(app).into(),
                                                                    0.06,
                                                                ));
                                                            }
                                                            style.cursor_pointer()
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
                                    .min_h(px(260.0))
                                    .rounded(px(16.0))
                                    .border_1()
                                    .border_color(glass::divider(app))
                                    .bg(glass::inset(app))
                                    .overflow_hidden()
                                    .child(detail_input),
                            ),
                    ),
            )
            .child(env_bottom_bar(handle.clone(), app))
    }
}

fn env_list_sidebar(
    environments: &[crate::service::ApiEnvironment],
    selected_index: usize,
    handle: Entity<ApiDebuggerView>,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .w(px(200.0))
        .min_h(px(0.0))
        .rounded(px(18.0))
        .border_1()
        .border_color(glass::divider(cx))
        .bg(glass::bar(cx))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .p(px(14.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(ui::text_primary(cx))
                                .child("环境列表"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(ui::text_secondary(cx))
                                .child(format!("{} 个环境", environments.len())),
                        ),
                )
                .child(
                    Button::new("api-env-list-new")
                        .ghost()
                        .icon(IconName::Plus)
                        .with_size(Size::XSmall)
                        .tooltip("新建环境")
                        .on_click({
                            let h = handle.clone();
                            move |_, _, cx| {
                                h.update(cx, |view, _cx| view.create_new_environment());
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .rounded(px(16.0))
                .bg(glass::inset(cx))
                .overflow_hidden()
                .child(
                    div()
                        .flex_1()
                        .flex_col()
                        .gap(px(8.0))
                        .children(environments.iter().enumerate().map({
                            let list_handle = handle.clone();
                            move |(i, env)| {
                                let active = i == selected_index;
                                let h = list_handle.clone();
                                div()
                                    .id(("api-env-list-item", i))
                                    .relative()
                                    .px(px(12.0))
                                    .py(px(10.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .rounded(px(14.0))
                                    .bg(if active {
                                        theme::rgba_with_alpha(api_accent(cx), 0.10)
                                    } else {
                                        ui::bg_surface(cx)
                                    })
                                    .border_1()
                                    .border_color(if active {
                                        theme::rgba_with_alpha(api_accent(cx), 0.14)
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .hover(move |mut style| {
                                        if !active {
                                            style = style.bg(ui::bg_hover(cx));
                                        }
                                        style.cursor_pointer()
                                    })
                                    .child(circle_badge(&env.badge, env.color, 12.0))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_weight(if active {
                                                        gpui::FontWeight::SEMIBOLD
                                                    } else {
                                                        gpui::FontWeight::NORMAL
                                                    })
                                                    .text_color(if active {
                                                        ui::text_primary(cx)
                                                    } else {
                                                        ui::text_secondary(cx)
                                                    })
                                                    .child(env.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .text_color(ui::text_tertiary(cx))
                                                    .overflow_hidden()
                                                    .child(env.base_url.clone()),
                                            ),
                                    )
                                    .on_click(move |_, _, cx| {
                                        h.update(cx, |view, cx| view.select_environment(i, cx));
                                    })
                            }
                        }))
                ),
        )
}

fn env_bottom_bar(handle: Entity<ApiDebuggerView>, cx: &App) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .h(px(40.0))
        .px(px(14.0))
        .border_t_1()
        .border_color(glass::divider(cx))
        .bg(glass::bar(cx))
        .flex()
        .items_center()
        .gap(px(8.0))
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
        .child(div().flex_1())
        .child(
            Button::new("api-env-win-export")
                .ghost()
                .icon(IconName::File)
                .with_size(Size::XSmall)
                .label("导出")
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
                .label("导入")
                .on_click({
                    let h = handle.clone();
                    move |_, _, cx| {
                        h.update(cx, |view, _cx| view.import_environments());
                    }
                }),
        )
}

fn compact_field(label: &'static str, input: Entity<InputState>, cx: &App) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(10.0))
                .text_color(ui::text_tertiary(cx))
                .child(label),
        )
        .child(
            div()
                .h(px(30.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(glass::divider(cx))
                .bg(glass::inset(cx))
                .overflow_hidden()
                .child(api_input(input, 30.0)),
        )
}

fn api_input(input: Entity<InputState>, height: f32) -> Input {
    Input::new(&input)
        .appearance(false)
        .bordered(false)
        .focus_bordered(false)
        .h(px(height))
        .text_size(px(11.0))
}
