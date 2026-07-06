//! OtpInput — one-time password input field.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable,
    AppContext as _, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement as _, Render, RenderOnce, SharedString, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, px,
};

use super::{InputEvent, blink_cursor::BlinkCursor};
use crate::token::tokens;

pub struct OtpState {
    focus_handle: FocusHandle,
    value: SharedString,
    blink_cursor: Entity<BlinkCursor>,
    masked: bool,
    length: usize,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<InputEvent> for OtpState {}

impl OtpState {
    pub fn new(length: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let blink_cursor = cx.new(|_| BlinkCursor::new());

        let _subscriptions = vec![
            cx.observe(&blink_cursor, |_, _, cx| cx.notify()),
            cx.on_focus(&focus_handle, window, Self::on_focus),
            cx.on_blur(&focus_handle, window, Self::on_blur),
        ];

        Self {
            length,
            focus_handle,
            value: SharedString::default(),
            blink_cursor,
            masked: false,
            _subscriptions,
        }
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    pub fn set_value(&mut self, value: impl Into<SharedString>, _: &mut Window, cx: &mut Context<Self>) {
        self.value = value.into();
        cx.notify();
    }

    pub fn value(&self) -> &SharedString { &self.value }

    pub fn masked(mut self, masked: bool) -> Self { self.masked = masked; self }

    pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.masked = masked;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, _: &mut Context<Self>) {
        self.focus_handle.focus(window);
    }

    fn on_input_mouse_down(&mut self, _: &MouseDownEvent, window: &mut Window, _: &mut Context<Self>) {
        window.focus(&self.focus_handle);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let mut chars: Vec<char> = self.value.chars().collect();
        let ix = chars.len();
        let key = event.keystroke.key.as_str();

        match key {
            "backspace" => {
                if ix > 0 { chars.remove(ix - 1); }
                window.prevent_default();
                cx.stop_propagation();
            }
            _ => {
                if let Some(c) = key.chars().next() {
                    if !matches!(c, '0'..='9') { return; }
                    if ix >= self.length { return; }
                    chars.push(c);
                }
                window.prevent_default();
                cx.stop_propagation();
            }
        }
        self.value = SharedString::from(chars.iter().collect::<String>());

        if self.value.chars().count() == self.length {
            cx.emit(InputEvent::Change);
        }
        cx.notify()
    }

    fn on_focus(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| cursor.start(cx));
        cx.emit(InputEvent::Focus);
    }

    fn on_blur(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| cursor.stop(cx));
        cx.emit(InputEvent::Blur);
    }

}

impl Focusable for OtpState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for OtpState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let blink_show = self.blink_cursor.read(cx).visible();
        let is_focused = self.focus_handle.is_focused(window);
        let token = tokens(cx);
        let cursor_ix = self.value.chars().count().min(self.length.saturating_sub(1));
        let state = cx.entity();

        div()
            .id(("otp-input", state.entity_id()))
            .track_focus(&self.focus_handle)
            .on_key_down(window.listener_for(&state, OtpState::on_key_down))
            .items_center()
            .gap(px(4.0))
            .children((0..self.length).map(|ix| {
                let c = self.value.chars().nth(ix);
                let is_input_focused = ix == cursor_ix && is_focused;

                div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .border_1()
                    .border_color(token.border)
                    .bg(token.background)
                    .when(is_input_focused, |this| this.border_color(token.border_focus))
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .text_size(px(16.0))
                    .on_mouse_down(MouseButton::Left, window.listener_for(&state, OtpState::on_input_mouse_down))
                    .child(if let Some(ch) = c {
                        if self.masked { "*".to_string() } else { ch.to_string() }
                    } else if is_input_focused && blink_show {
                        "|".to_string()
                    } else {
                        String::new()
                    })
            }))
    }
}

#[derive(IntoElement)]
pub struct OtpInput {
    state: Entity<OtpState>,
    disabled: bool,
}

impl OtpInput {
    pub fn new(state: &Entity<OtpState>) -> Self {
        Self { state: state.clone(), disabled: false }
    }

    pub fn groups(mut self, _n: usize) -> Self { self }
    pub fn disabled(mut self, disabled: bool) -> Self { self.disabled = disabled; self }
}

impl RenderOnce for OtpInput {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id(("otp-input-wrapper", self.state.entity_id()))
            .child(self.state.clone())
    }
}
