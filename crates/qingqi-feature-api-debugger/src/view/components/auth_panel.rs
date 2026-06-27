use super::shared::section_micro_label;
use crate::service::AuthType;
use crate::view::ApiDebuggerView;
use crate::view::types::AuthFormInputs;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, hsla, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::theme::Theme;
use qingqi_ui::{theme, ui, ui::glass};

pub fn auth_form_panel(
    view: Entity<ApiDebuggerView>,
    auth_type: AuthType,
    form: AuthFormInputs,
    cx: &App,
) -> impl IntoElement {
    let body = match auth_type {
        AuthType::None => div()
            .py(px(6.0))
            .text_size(px(11.0))
            .text_color(ui::text_tertiary(cx))
            .child("该请求不附带认证信息。")
            .into_any_element(),
        AuthType::BearerToken => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("Token", form.bearer.clone(), cx))
            .child(auth_hint(cx))
            .into_any_element(),
        AuthType::BasicAuth => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("用户名", form.basic_user.clone(), cx))
            .child(labeled_field("密码", form.basic_pass.clone(), cx))
            .child(auth_hint(cx))
            .into_any_element(),
        AuthType::ApiKey => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("Key", form.apikey_name.clone(), cx))
            .child(labeled_field("Value", form.apikey_value.clone(), cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_micro_label("位置", cx))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(auth_location_button(
                                view.clone(),
                                "Header",
                                false,
                                !form.in_query,
                                cx,
                            ))
                            .child(auth_location_button(
                                view.clone(),
                                "Query",
                                true,
                                form.in_query,
                                cx,
                            )),
                    ),
            )
            .into_any_element(),
    };

    div().flex().flex_col().gap(px(12.0)).child(body)
}

fn auth_hint(cx: &App) -> impl IntoElement {
    div()
        .text_size(px(10.0))
        .text_color(ui::text_tertiary(cx))
        .child("发送时自动添加请求头 Authorization: Bearer <token>。")
}

fn auth_location_button(
    view: Entity<ApiDebuggerView>,
    label: &'static str,
    query: bool,
    active: bool,
    cx: &App,
) -> impl IntoElement {
    div()
        .id(("api-auth-location", query as usize))
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .text_size(px(10.0))
        .text_color(if active {
            Theme::global(cx).foreground
        } else {
            ui::text_tertiary(cx)
        })
        .bg(if active {
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
        .child(label)
        .on_click(move |_, _, cx| {
            view.update(cx, |view, cx| {
                view.auth_apikey_in_query = query;
                view.sync_models(cx);
                view.persist_workspace();
            });
        })
}

fn labeled_field(label: &'static str, input: Entity<InputState>, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(section_micro_label(label, cx))
        .child(
            div()
                .h(px(32.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light(cx))
                .bg(theme::rgba_with_alpha(
                    Theme::global(cx).list.into(),
                    if Theme::global(cx).is_dark() {
                        0.34
                    } else {
                        0.58
                    },
                ))
                .overflow_hidden()
                .child(api_input(input, 32.0)),
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
