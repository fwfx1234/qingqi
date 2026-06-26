use super::shared::{api_accent, transparent_surface};
use crate::service::EditorTab;
use crate::view::ApiDebuggerView;
use crate::view::types::KvRow;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, hsla, prelude::FluentBuilder, px,
};
use gpui_component::{input::{Input, InputState}, theme::Theme};
use gpui_component::{
    IconName, Sizable, Size,
    button::{Button, ButtonVariants},
};
use qingqi_ui::{theme, ui, ui::glass};

pub fn kv_editor_table(
    view: Entity<ApiDebuggerView>,
    tab: EditorTab,
    rows: Vec<KvRow>,
    cx: &App,
) -> impl IntoElement {
    let add_view = view.clone();
    let show_schema_columns = tab == EditorTab::Params;

    div()
        .flex()
        .flex_col()
        .rounded(px(8.0))
        .border_1()
        .border_color(glass::divider(cx))
        .bg(glass::inset(cx))
        .overflow_hidden()
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
                .when(show_schema_columns, |header| {
                    header
                        .child(div().w(px(108.0)).flex_none().child("type"))
                        .child(div().flex_1().min_w(px(0.0)).child("desc"))
                })
                .child(div().w(px(24.0))),
        )
        .children(rows.into_iter().enumerate().map(move |(i, row)| {
            let enabled = row.enabled;
            let key_input = row.key.clone();
            let value_input = row.value.clone();
            let type_input = row.value_type.clone();
            let desc_input = row.description.clone();
            let toggle_view = view.clone();
            let delete_view = view.clone();

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
                                    if let Some(editor) = view.kv_editor_mut(tab) {
                                        editor.toggle(i);
                                    }
                                    view.sync_models(cx);
                                    view.persist_current_tab_state(cx);
                                });
                            }),
                    ),
                )
                .child(kv_cell(key_input, enabled, cx))
                .child(kv_cell(value_input, enabled, cx))
                .when(show_schema_columns, |row| {
                    row.child(kv_cell_fixed(type_input, enabled, cx, 108.0))
                        .child(kv_cell(desc_input, enabled, cx))
                })
                .child(
                    Button::new(("kv-del", i))
                        .ghost()
                        .icon(IconName::Close)
                        .with_size(Size::XSmall)
                        .on_click(move |_, _, cx| {
                            delete_view.update(cx, |view, cx| {
                                if let Some(editor) = view.kv_editor_mut(tab) {
                                    editor.remove_row(i);
                                }
                                view.sync_models(cx);
                                view.persist_current_tab_state(cx);
                            });
                        }),
                )
        }))
        .child(
            div().px(px(10.0)).py(px(7.0)).child(
                Button::new("kv-add-row")
                    .ghost()
                    .icon(IconName::Plus)
                    .label("新增")
                    .with_size(Size::XSmall)
                    .on_click(move |_, window, cx| {
                        add_view.update(cx, |view, cx| {
                            if let Some(editor) = view.kv_editor_mut(tab) {
                                editor.add_row(window, cx);
                            }
                            view.persist_current_tab_state(cx);
                        });
                    }),
            ),
        )
}

fn kv_cell(input: Entity<InputState>, enabled: bool, cx: &App) -> gpui::Div {
    kv_cell_base(input, enabled, cx).flex_1()
}

fn kv_cell_fixed(
    input: Entity<InputState>,
    enabled: bool,
    cx: &App,
    width: f32,
) -> gpui::Div {
    kv_cell_base(input, enabled, cx).w(px(width)).flex_none()
}

fn kv_cell_base(
    input: Entity<InputState>,
    enabled: bool,
    cx: &App,
) -> gpui::Div {
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
