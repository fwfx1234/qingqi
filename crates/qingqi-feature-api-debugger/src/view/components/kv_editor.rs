use super::shared::{api_accent, transparent_surface};
use crate::service::EditorTab;
use crate::view::ApiDebuggerView;
use crate::view::types::{KvEditorTarget, KvRow};
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, hsla, prelude::FluentBuilder, px,
};
use qingqi_ui::components::button::{Button, ButtonVariants};
use qingqi_ui::components::input::{Input, InputState};
use qingqi_ui::components::styled::Sizable;
use qingqi_ui::components::styled::Size;
use qingqi_ui::components::theme::Theme;
use qingqi_ui::{icon, theme, ui, ui::glass};

pub fn kv_editor_table(
    view: Entity<ApiDebuggerView>,
    target: KvEditorTarget,
    rows: Vec<KvRow>,
    show_schema_columns: bool,
    show_value_type: bool,
    cx: &App,
) -> impl IntoElement {
    let add_view = view.clone();
    let change_view = view.clone();

    div()
        .flex()
        .flex_col()
        .rounded(px(8.0))
        .border_1()
        .border_color(glass::divider(cx))
        .bg(glass::inset(cx))
        .overflow_hidden()
        .on_key_down(move |_, _, cx| {
            let view = change_view.clone();
            cx.defer(move |cx| {
                view.update(cx, |view, cx| {
                    if target == KvEditorTarget::Tab(EditorTab::Params) {
                        view.sync_url_from_parameter_table(cx);
                    } else {
                        view.update_kv_target(target, cx);
                        view.persist_workspace();
                    }
                });
            });
        })
        .child(
            div()
                .id("kv-table-header")
                .h(px(28.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .bg(glass::bar(cx))
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(10.0))
                .text_color(ui::text_tertiary(cx))
                .child(div().w(px(24.0)))
                .child(div().flex_1().min_w(px(0.0)).child("key"))
                .child(div().flex_1().min_w(px(0.0)).child("value"))
                .when(show_value_type || show_schema_columns, |header| {
                    header
                        .child(div().w(px(108.0)).flex_none().child("type"))
                })
                .when(show_schema_columns, |header| {
                    header
                        .child(div().flex_1().min_w(px(0.0)).child("desc"))
                })
                .child(div().w(px(24.0))),
        )
        .children(rows.into_iter().enumerate().map(move |(i, row)| {
            let enabled = row.enabled;
            let key_input = row.key.clone();
            let value_input = row.value.clone();
            let type_input = row.value_type.clone();
            let type_control_input = row.value_type.clone();
            let desc_input = row.description.clone();
            let file_value_input = row.value.clone();
            let is_file = target
                == KvEditorTarget::Body(crate::service::BodyMode::FormData)
                && type_input
                    .read(cx)
                    .value()
                    .eq_ignore_ascii_case("file");
            let toggle_view = view.clone();
            let delete_view = view.clone();
            let file_view = view.clone();
            let file_path_for_badge = file_value_input.read(cx).value().to_string();

            div()
                .id(("kv-row", i))
                .min_h(px(38.0))
                .px(px(10.0))
                .py(px(4.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .hover(move |s| s.bg(glass::hover_bg(cx)))
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div().w(px(24.0)).flex().justify_center().child(
                        div()
                            .id(("kv-checkbox", i))
                            .w(px(13.0))
                            .h(px(13.0))
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(if enabled {
                                theme::rgba_with_alpha(api_accent(cx), 0.55)
                            } else {
                                glass::divider(cx)
                            })
                            .bg(if enabled {
                                theme::rgba_with_alpha(api_accent(cx), 0.11)
                            } else {
                                transparent_surface(cx)
                            })
                            .text_size(px(9.0))
                            .text_color(if enabled {
                                api_accent(cx).into()
                            } else {
                                hsla(0.0, 0.0, 0.0, 0.0)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .child(if enabled { "✓" } else { "" })
                            .on_click(move |_, _, cx| {
                                toggle_view.update(cx, |view, cx| {
                                    if let Some(editor) = view.kv_editor_target_mut(target) {
                                        editor.toggle(i);
                                    }
                                    if target == KvEditorTarget::Tab(EditorTab::Params) {
                                        view.sync_url_from_parameter_table(cx);
                                    } else {
                                        view.update_kv_target(target, cx);
                                        view.persist_workspace();
                                    }
                                });
                            }),
                    ),
                )
                .child(kv_cell(key_input, enabled, cx))
                .child(kv_cell(value_input, enabled, cx))
                .when(is_file, |row| {
                    row.child(
                        Button::new(("kv-pick-file", i))
                            .ghost()
                            .icon(icon!(folder_open))
                            .with_size(Size::XSmall)
                            .on_click(move |_, _, cx| {
                                let Some(path) = rfd::FileDialog::new().pick_file() else {
                                    return;
                                };
                                let value = path.display().to_string();
                                file_value_input.update(cx, |input, input_cx| {
                                    input.reset_value(value, input_cx)
                                });
                                file_view.update(cx, |view, cx| {
                                    view.sync_models(cx);
                                    view.persist_workspace();
                                });
                            }),
                    )
                })
                .when(is_file, |row| {
                    let file_name = if file_path_for_badge.is_empty() {
                        String::from("未选择")
                    } else {
                        std::path::Path::new(&file_path_for_badge)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&file_path_for_badge)
                            .to_string()
                    };
                    row.child(
                        div()
                            .id(("kv-file-badge", i))
                            .max_w(px(120.0))
                            .px(px(6.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(glass::divider(cx))
                            .bg(glass::inset(cx))
                            .text_size(px(9.0))
                            .text_color(ui::text_secondary(cx))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(file_name),
                    )
                })
                .when(show_value_type, |row| {
                    row.child(body_value_type_control(
                        view.clone(),
                        target,
                        type_control_input,
                        is_file,
                        enabled,
                        i,
                        cx,
                    ))
                })
                .when(show_schema_columns, |row| {
                    row.child(kv_cell_fixed(type_input, enabled, cx, 108.0))
                })
                .when(show_schema_columns, |row| {
                    row
                        .child(kv_cell(desc_input, enabled, cx))
                })
                .child(
                    Button::new(("kv-del", i))
                        .ghost()
                        .icon(icon!(x))
                        .with_size(Size::XSmall)
                        .on_click(move |_, _, cx| {
                            delete_view.update(cx, |view, cx| {
                                if let Some(editor) = view.kv_editor_target_mut(target) {
                                    editor.remove_row(i);
                                }
                                if target == KvEditorTarget::Tab(EditorTab::Params) {
                                    view.sync_url_from_parameter_table(cx);
                                } else {
                                    view.update_kv_target(target, cx);
                                    view.persist_workspace();
                                }
                            });
                        }),
                )
        }))
        .child(
            div().px(px(10.0)).py(px(7.0)).child(
                Button::new("kv-add-row")
                    .ghost()
                    .icon(icon!(plus))
                    .label("新增")
                    .with_size(Size::XSmall)
                    .on_click(move |_, window, cx| {
                        add_view.update(cx, |view, cx| {
                            if let Some(editor) = view.kv_editor_target_mut(target) {
                                editor.add_row(window, cx);
                                if target == KvEditorTarget::Body(crate::service::BodyMode::FormData)
                                {
                                    if let Some(row) = editor.rows.last() {
                                        row.value_type.update(cx, |input, input_cx| {
                                            input.reset_value("text", input_cx)
                                        });
                                    }
                                }
                            }
                            if target == KvEditorTarget::Tab(EditorTab::Params) {
                                view.sync_url_from_parameter_table(cx);
                            } else {
                                view.update_kv_target(target, cx);
                                view.persist_workspace();
                            }
                        });
                    }),
            ),
        )
}

fn body_value_type_control(
    view: Entity<ApiDebuggerView>,
    target: KvEditorTarget,
    input: Entity<InputState>,
    is_file: bool,
    enabled: bool,
    row_index: usize,
    cx: &App,
) -> gpui::Div {
    div()
        .w(px(108.0))
        .h(px(28.0))
        .flex_none()
        .rounded(px(6.0))
        .border_1()
        .border_color(glass::divider(cx))
        .bg(glass::inset(cx))
        .p(px(2.0))
        .flex()
        .gap(px(2.0))
        .children([("Text", false), ("File", true)].into_iter().map(
            move |(label, file)| {
                let selected = is_file == file;
                let view = view.clone();
                let input = input.clone();
                div()
                    .id(("body-value-type", row_index * 2 + usize::from(file)))
                    .flex_1()
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(9.0))
                    .text_color(if selected {
                        Theme::global(cx).foreground
                    } else {
                        ui::text_tertiary(cx)
                    })
                    .bg(if selected {
                        theme::rgba_with_alpha(Theme::global(cx).foreground.into(), 0.08)
                    } else {
                        transparent_surface(cx)
                    })
                    .when(enabled, |item| {
                        item.cursor_pointer()
                            .hover(|style| style.bg(glass::hover_bg(cx)))
                            .on_click(move |_, _, cx| {
                                let value = if file { "file" } else { "text" };
                                input.update(cx, |input, input_cx| {
                                    input.reset_value(value, input_cx)
                                });
                                view.update(cx, |view, cx| {
                                    if let Some(editor) = view.kv_editor_target_mut(target) {
                                        if let Some(row) = editor.rows.get_mut(row_index) {
                                            row.value_type.update(cx, |state, state_cx| {
                                                state.reset_value(value, state_cx)
                                            });
                                        }
                                    }
                                    view.update_kv_target(target, cx);
                                    view.persist_workspace();
                                });
                            })
                    })
                    .child(label)
            },
        ))
}

fn kv_cell(input: Entity<InputState>, enabled: bool, cx: &App) -> gpui::Div {
    kv_cell_base(input, enabled, cx).flex_1()
}

fn kv_cell_fixed(input: Entity<InputState>, enabled: bool, cx: &App, width: f32) -> gpui::Div {
    kv_cell_base(input, enabled, cx).w(px(width)).flex_none()
}

fn kv_cell_base(input: Entity<InputState>, enabled: bool, cx: &App) -> gpui::Div {
    div()
        .min_w(px(0.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(glass::divider(cx))
        .bg(theme::rgba_with_alpha(
            Theme::global(cx).list.into(),
            if enabled { 0.36 } else { 0.18 },
        ))
        .overflow_hidden()
        .when(!enabled, |cell| cell.opacity(0.5))
        .child(
            Input::new(&input)
                .appearance(false)
                .bordered(false)
                .focus_bordered(false)
                .disabled(!enabled)
                .h(px(28.0))
                .text_size(px(11.0)),
        )
}
