use gpui::{
    App, Entity, IntoElement, InteractiveElement, ParentElement,
    StatefulInteractiveElement, Styled, div, hsla, px, prelude::FluentBuilder,
};
use gpui_component::theme::Theme;
use gpui_component::{Sizable, Size, button::{Button, ButtonVariants}};
use qingqi_ui::{theme, ui, ui::glass};
use qingqi_ui::text_input::TextInput;
use crate::service::{AuthType, BodyMode, EditorTab};
use crate::view::ApiDebuggerView;
use crate::view::types::{AuthFormInputs, KvRow};
use super::shared::{section_micro_label, transparent_surface};

pub fn editor_panel(
    view: Entity<ApiDebuggerView>,
    editor_tab: EditorTab,
    text_input: Option<Entity<TextInput>>,
    kv_rows: Vec<KvRow>,
    auth_form: AuthFormInputs,
    body_mode: BodyMode,
    auth_type: AuthType,
    cx: &App,
) -> impl IntoElement {
    let label = editor_tab.label();
    let tabs_view = view.clone();
    let subtoolbar_view = view.clone();
    let mode_row = match editor_tab {
        EditorTab::Body => {
            let bm_view = subtoolbar_view.clone();
            let modes = BodyMode::all();
            let mut row = div()
                .px(px(10.0))
                .py(px(4.0))
                .flex()
                .items_center()
                .gap(px(4.0));
            for (i, mode) in modes.iter().enumerate() {
                let label = mode.label();
                let is_active = mode == &body_mode;
                let bm_click = bm_view.clone();
                let mode_val = mode.as_str().to_string();
                row = row.child(
                    div()
                        .id(("api-body-mode-btn", i))
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded(px(4.0))
                        .text_size(px(10.0))
                        .text_color(if is_active {
                            Theme::global(cx).foreground
                        } else {
                            ui::text_tertiary(cx)
                        })
                        .bg(if is_active {
                            theme::rgba_with_alpha(Theme::global(cx).foreground.into(), 0.055)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(move |style| {
                            style
                                .cursor_pointer()
                                .bg(glass::hover_bg(cx))
                                .text_color(Theme::global(cx).foreground)
                        })
                        .on_click(move |_, _window, cx| {
                            bm_click.update(cx, |view, cx| {
                                view.sync_models(cx);
                                view.body_mode = BodyMode::from_db(&mode_val);
                                view.persist_current_tab_state(cx);
                            });
                        })
                        .child(label),
                );
            }
            row = row.child(div().flex_1());
            match body_mode {
                BodyMode::Json => {
                    let fmt_view = bm_view.clone();
                    row = row.child(
                        Button::new("api-body-format-json")
                            .ghost()
                            .label("格式化")
                            .with_size(Size::XSmall)
                            .on_click(move |_, _, cx| {
                                fmt_view.update(cx, |view, cx| view.format_json_body(cx));
                            }),
                    );
                }
                BodyMode::Binary => {
                    let pick_view = bm_view.clone();
                    row = row.child(
                        Button::new("api-body-pick-file")
                            .ghost()
                            .label("选择文件")
                            .with_size(Size::XSmall)
                            .on_click(move |_, _, cx| {
                                pick_view.update(cx, |view, cx| view.pick_binary_file(cx));
                            }),
                    );
                }
                _ => {}
            }
            row.into_any_element()
        }
        EditorTab::Auth => {
            let au_view = subtoolbar_view.clone();
            let types = AuthType::all();
            let mut row = div()
                .px(px(10.0))
                .py(px(4.0))
                .flex()
                .items_center()
                .gap(px(4.0));
            for (i, at) in types.iter().enumerate() {
                let label = at.label();
                let is_active = at == &auth_type;
                let au_click = au_view.clone();
                let at_val = at.as_str().to_string();
                row = row.child(
                    div()
                        .id(("api-auth-type-btn", i))
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded(px(4.0))
                        .text_size(px(10.0))
                        .text_color(if is_active {
                            Theme::global(cx).foreground
                        } else {
                            ui::text_tertiary(cx)
                        })
                        .bg(if is_active {
                            theme::rgba_with_alpha(Theme::global(cx).foreground.into(), 0.055)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(move |style| {
                            style
                                .cursor_pointer()
                                .bg(glass::hover_bg(cx))
                                .text_color(Theme::global(cx).foreground)
                        })
                        .on_click(move |_, _window, cx| {
                            au_click.update(cx, |view, cx| {
                                view.auth_type = AuthType::from_db(&at_val);
                                view.sync_models(cx);
                                view.persist_current_tab_state(cx);
                            });
                        })
                        .child(label),
                );
            }
            row.into_any_element()
        }
        _ => div().into_any_element(),
    };

    let editor_tabs = [
        EditorTab::Params,
        EditorTab::Path,
        EditorTab::Body,
        EditorTab::Headers,
        EditorTab::Cookies,
        EditorTab::Auth,
        EditorTab::PreOps,
        EditorTab::PostOps,
    ];

    div()
        .flex_1()
        .min_h(px(300.0))
        .bg(glass::panel(cx))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.0))
                .border_b_1()
                .border_color(glass::divider(cx))
                .bg(glass::bar(cx))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
                .children(
                    editor_tabs
                        .into_iter()
                        .enumerate()
                        .map(move |(index, tab)| {
                            editor_tab_button(
                                tabs_view.clone(),
                                index,
                                tab,
                                tab == editor_tab,
                                cx,
                            )
                        }),
                ),
        )
        .child(mode_row)
        .child(
            div()
                .id("api-editor-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(4.0))
                .p(px(12.0))
                .child(match editor_tab {
                    EditorTab::Params
                    | EditorTab::Headers
                    | EditorTab::Path
                    | EditorTab::Cookies => {
                        super::kv_editor::kv_editor_table(view.clone(), editor_tab, kv_rows, cx)
                            .into_any_element()
                    }
                    EditorTab::Auth => {
                        super::auth_panel::auth_form_panel(view.clone(), auth_type, auth_form, cx)
                            .into_any_element()
                    }
                    _ => {
                        let input = text_input.expect("non-KV editor tab must have a text input");
                        let hint = if editor_tab == EditorTab::Body {
                            match body_mode {
                                BodyMode::FormData => {
                                    Some("每行一个字段：key=value；文件用 key=@/path/to/file。")
                                }
                                BodyMode::FormUrlEncoded => {
                                    Some("每行一个字段：key=value（发送时拼接为 a=1&b=2）。")
                                }
                                BodyMode::Binary => Some("填写文件路径，或点击右上角“选择文件”。"),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(section_micro_label(label, cx))
                            .when_some(hint, |column, text| column.child(auth_hint(text, cx)))
                            .child(
                                div()
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(glass::divider(cx))
                                    .bg(glass::inset(cx))
                                    .overflow_hidden()
                                    .child(input),
                            )
                            .into_any_element()
                    }
                }),
        )
}

pub fn editor_tab_button(
    view: Entity<ApiDebuggerView>,
    index: usize,
    tab: EditorTab,
    active: bool,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(("api-editor-tab", index))
        .h(px(30.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .bg(if active {
            theme::rgba_with_alpha(Theme::global(cx).foreground.into(), 0.055)
        } else {
            transparent_surface(cx)
        })
        .text_size(px(11.0))
        .font_weight(if active {
            gpui::FontWeight::SEMIBOLD
        } else {
            gpui::FontWeight::NORMAL
        })
        .text_color(if active {
            Theme::global(cx).foreground
        } else {
            ui::text_tertiary(cx)
        })
        .hover(move |style| {
            style
                .bg(glass::hover_bg(cx))
                .text_color(Theme::global(cx).foreground)
                .cursor_pointer()
        })
        .flex()
        .items_center()
        .child(tab.label())
        .on_click(move |_, _window, cx| {
            view.update(cx, |view, cx| {
                view.sync_models(cx);
                view.persist_current_tab_state(cx);
                view.editor_tab = tab;
                view.persist_current_tab_state(cx);
            });
        })
}

fn auth_hint(text: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(10.0))
        .text_color(ui::text_tertiary(cx))
        .child(text)
}
