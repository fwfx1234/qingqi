use gpui::{Entity, IntoElement, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, hsla, px};
use qingqi_ui::text_input::TextInput;
use qingqi_ui::{theme, ui, ui::glass};
use crate::service::AuthType;
use crate::view::ApiDebuggerView;
use crate::view::types::AuthFormInputs;
use super::shared::section_micro_label;

pub fn auth_form_panel(
    view: Entity<ApiDebuggerView>,
    auth_type: AuthType,
    form: AuthFormInputs,
    dark: bool,
) -> impl IntoElement {
    let body = match auth_type {
        AuthType::None => div()
            .py(px(6.0))
            .text_size(px(11.0))
            .text_color(ui::text_tertiary())
            .child("该请求不附带认证信息。")
            .into_any_element(),
        AuthType::BearerToken => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("Token", form.bearer.clone(), dark))
            .child(auth_hint(
                "发送时自动添加请求头 Authorization: Bearer <token>。",
            ))
            .into_any_element(),
        AuthType::BasicAuth => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("用户名", form.basic_user.clone(), dark))
            .child(labeled_field("密码", form.basic_pass.clone(), dark))
            .child(auth_hint(
                "发送时自动以 Base64 编码为 Authorization: Basic 头。",
            ))
            .into_any_element(),
        AuthType::ApiKey => div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(labeled_field("Key", form.apikey_name.clone(), dark))
            .child(labeled_field("Value", form.apikey_value.clone(), dark))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_micro_label("位置", dark))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(auth_location_button(
                                view.clone(),
                                "Header",
                                false,
                                !form.in_query,
                                dark,
                            ))
                            .child(auth_location_button(
                                view.clone(),
                                "Query",
                                true,
                                form.in_query,
                                dark,
                            )),
                    ),
            )
            .into_any_element(),
    };

    div().flex().flex_col().gap(px(12.0)).child(body)
}

fn auth_hint(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(10.0))
        .text_color(ui::text_tertiary())
        .child(text)
}

fn auth_location_button(
    view: Entity<ApiDebuggerView>,
    label: &'static str,
    query: bool,
    active: bool,
    dark: bool,
) -> impl IntoElement {
    div()
        .id(("api-auth-location", query as usize))
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .text_size(px(10.0))
        .text_color(if active {
            theme::semantic().text_primary
        } else {
            ui::text_tertiary()
        })
        .bg(if active {
            theme::rgba_with_alpha(theme::semantic().text_primary, 0.055)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .hover(move |style| {
            style
                .cursor_pointer()
                .bg(glass::hover_bg(dark))
                .text_color(theme::semantic().text_primary)
        })
        .child(label)
        .on_click(move |_, _, cx| {
            view.update(cx, |view, cx| {
                view.auth_apikey_in_query = query;
                view.sync_models(cx);
                view.persist_current_tab_state(cx);
            });
        })
}

fn labeled_field(label: &'static str, input: Entity<TextInput>, dark: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(section_micro_label(label, dark))
        .child(
            div()
                .h(px(32.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(ui::border_light())
                .bg(theme::rgba_with_alpha(
                    theme::semantic().bg_surface,
                    if dark { 0.34 } else { 0.58 },
                ))
                .overflow_hidden()
                .child(input),
        )
}
