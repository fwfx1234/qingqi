use super::dropdown::{DropdownItem, dropdown_list};
use crate::service::{ApiEnvironment, ApiRequest, HttpMethod};
use crate::view::ApiDebuggerView;
use gpui::{App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use gpui_component::popover::Popover;
use gpui_component::theme::Theme;
use gpui_component::{
    Icon, IconName, Sizable, Size,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use qingqi_ui::{theme, ui, ui::glass};

pub fn action_bar(
    view: Entity<ApiDebuggerView>,
    _request: ApiRequest,
    environment: ApiEnvironment,
    path_input: Entity<InputState>,
    in_flight: bool,
    cx: &App,
    current_method: HttpMethod,
    show_popover: bool,
) -> impl IntoElement {
    div()
        .px(px(10.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(glass::divider(cx))
        .bg(glass::bar(cx))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child({
            let view = view.clone();
            let current = current_method;
            Popover::new("api-method-popover")
                .anchor(gpui::Corner::TopLeft)
                .appearance(false)
                .open(show_popover)
                .on_open_change({
                    let v = view.clone();
                    move |is_open, _, cx| {
                        v.update(cx, |view, _cx| view.show_method_popover = *is_open);
                    }
                })
                .trigger({
                    let method_color =
                        theme::http_method_color(current.label(), Theme::global(cx).is_dark());
                    Button::new("api-method-trigger")
                        .ghost()
                        .w(px(76.0))
                        .h(px(30.0))
                        .rounded(px(5.0))
                        .bg(theme::rgba_with_alpha(method_color, 0.10))
                        .child(
                            div()
                                .px(px(8.0))
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .font_family("SF Mono")
                                        .text_size(px(11.0))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(method_color)
                                        .whitespace_nowrap()
                                        .flex_1()
                                        .child(current.label()),
                                )
                                .child(
                                    Icon::new(IconName::ChevronDown)
                                        .size(px(10.0))
                                        .text_color(method_color),
                                ),
                        )
                })
                .content(move |_state, _window, _cx| {
                    let curr = current;
                    let v = view.clone();
                    let accent = Theme::global(_cx).primary;
                    let bg = Theme::global(_cx).list;
                    let border = ui::border_light(_cx);
                    dropdown_list(
                        HttpMethod::all()
                            .into_iter()
                            .map(|method| {
                                let mc = theme::http_method_color(
                                    method.label(),
                                    Theme::global(_cx).is_dark(),
                                );
                                DropdownItem::new(
                                    div().flex().items_center().child(
                                        div()
                                            .font_family("SF Mono")
                                            .text_size(px(12.0))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(mc)
                                            .whitespace_nowrap()
                                            .child(method.label()),
                                    ),
                                )
                                .active(method == curr)
                                .on_select({
                                    let v = v.clone();
                                    let m = method;
                                    move |_, cx| {
                                        v.update(cx, |view, cx| {
                                            view.set_method(m, cx);
                                            view.show_method_popover = false;
                                        });
                                    }
                                })
                            })
                            .collect(),
                        accent,
                        bg,
                        border,
                    )
                })
        })
        .child({
            let url_view = view.clone();
            div()
                .flex_1()
                .h(px(32.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(glass::divider(cx))
                .bg(glass::inset(cx))
                .px(px(10.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .flex_shrink_0()
                        .font_family("SF Mono")
                        .text_size(px(11.0))
                        .text_color(ui::text_tertiary(cx))
                        .child(environment.base_url),
                )
                .child(div().flex_1().min_w(px(0.0)).child(api_input(path_input, 30.0)))
                .on_key_down(move |event, _window, cx| {
                    if event.keystroke.key == "enter" {
                        url_view.update(cx, |view, cx| view.send_request(cx));
                    }
                })
        })
        .child(if in_flight {
            Button::new("api-cancel-btn")
                .danger()
                .label("取消")
                .with_size(Size::Small)
                .on_click(move |_, _, cx| {
                    view.update(cx, |view, cx| view.cancel_request(cx));
                })
                .into_any_element()
        } else {
            Button::new("api-send-btn")
                .primary()
                .icon(IconName::ArrowRight)
                .label("发送")
                .with_size(Size::Small)
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| view.send_request(cx));
                    }
                })
                .into_any_element()
        })
}

fn api_input(input: Entity<InputState>, height: f32) -> Input {
    Input::new(&input)
        .appearance(false)
        .bordered(false)
        .focus_bordered(false)
        .h(px(height))
        .text_size(px(11.0))
}
