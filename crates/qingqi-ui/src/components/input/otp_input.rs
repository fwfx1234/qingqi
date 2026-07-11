//! Fixed-length one-time password input.

use gpui::{
    AnyElement, App, AppContext as _, Context, Empty, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement as _, Render, RenderOnce, SharedString, StyleRefinement, Styled, Subscription,
    Window, div, prelude::FluentBuilder as _, px,
};

use super::{InputEvent, blink_cursor::BlinkCursor};
use crate::components::{Disableable, Sizable, Size, StyledExt as _, h_flex};
use crate::token::tokens;

pub struct OtpState {
    focus_handle: FocusHandle,
    value: SharedString,
    blink_cursor: Entity<BlinkCursor>,
    masked: bool,
    disabled: bool,
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
            disabled: false,
            _subscriptions,
        }
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = sanitize_value(value.into().as_ref(), self.length).into();
        self
    }

    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = sanitize_value(value.into().as_ref(), self.length);
        if self.value.as_ref() == value {
            return;
        }
        self.value = value.into();
        cx.emit(InputEvent::Change);
        cx.notify();
    }

    pub fn value(&self) -> &SharedString {
        &self.value
    }

    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.masked = masked;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, _: &mut Context<Self>) {
        if !self.disabled {
            self.focus_handle.focus(window);
        }
    }

    pub fn length(&self) -> usize {
        self.length
    }

    fn on_input_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        if !self.disabled {
            window.focus(&self.focus_handle);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let mut value = self.value.to_string();
        let key = event.keystroke.key.as_str();
        let changed = if key == "backspace" {
            value.pop().is_some()
        } else {
            let digit = event
                .keystroke
                .key_char
                .as_deref()
                .and_then(normalize_digit)
                .or_else(|| normalize_digit(key));
            if let Some(digit) = digit
                && value.len() < self.length
            {
                value.push(digit);
                true
            } else {
                false
            }
        };
        if !changed {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        self.blink_cursor.update(cx, |cursor, cx| cursor.pause(cx));
        self.value = value.into();
        cx.emit(InputEvent::Change);
        cx.notify();
    }

    fn on_focus(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.blink_cursor.update(cx, |cursor, cx| cursor.start(cx));
        cx.emit(InputEvent::Focus);
    }

    fn on_blur(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| cursor.stop(cx));
        cx.emit(InputEvent::Blur);
    }
}

fn normalize_digit(value: &str) -> Option<char> {
    value
        .chars()
        .next()
        .and_then(digit_value)
        .and_then(|digit| char::from_digit(digit, 10))
}

fn digit_value(ch: char) -> Option<u32> {
    ch.to_digit(10).or_else(|| {
        let value = (ch as u32).checked_sub('０' as u32)?;
        (value <= 9).then_some(value)
    })
}

fn sanitize_value(value: &str, length: usize) -> String {
    value
        .chars()
        .filter_map(|ch| digit_value(ch).and_then(|digit| char::from_digit(digit, 10)))
        .take(length)
        .collect()
}

impl Focusable for OtpState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for OtpState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[derive(IntoElement)]
pub struct OtpInput {
    state: Entity<OtpState>,
    number_of_groups: usize,
    size: Size,
    disabled: bool,
    style: StyleRefinement,
}

impl OtpInput {
    pub fn new(state: &Entity<OtpState>) -> Self {
        Self {
            state: state.clone(),
            number_of_groups: 2,
            size: Size::Medium,
            disabled: false,
            style: StyleRefinement::default(),
        }
    }

    pub fn groups(mut self, count: usize) -> Self {
        self.number_of_groups = count.max(1);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Disableable for OtpInput {
    fn disabled(self, disabled: bool) -> Self {
        OtpInput::disabled(self, disabled)
    }
}

impl Sizable for OtpInput {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for OtpInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for OtpInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            if state.disabled != self.disabled {
                state.disabled = self.disabled;
                if self.disabled {
                    state.blink_cursor.update(cx, |cursor, cx| cursor.stop(cx));
                }
                cx.notify();
            }
        });

        let state = self.state.read(cx);
        let token = tokens(cx);
        let length = state.length;
        let group_count = self.number_of_groups.min(length.max(1));
        let chunk_size = length.div_ceil(group_count);
        let blink_show = state.blink_cursor.read(cx).visible();
        let focused = state.focus_handle.is_focused(window) && !self.disabled;
        let cursor = state.value.chars().count().min(length.saturating_sub(1));
        let cell_size = match self.size {
            Size::Large => px(44.0),
            Size::Medium => px(32.0),
            Size::Small => px(28.0),
            Size::XSmall => px(24.0),
            Size::Size(value) => value,
        };
        let text_size = cell_size * 0.5;
        let mut groups: Vec<Vec<AnyElement>> = (0..group_count).map(|_| Vec::new()).collect();
        for index in 0..length {
            let group_index = (index / chunk_size).min(group_count - 1);
            let value = state.value.chars().nth(index);
            let active = focused && index == cursor;
            let cell = div()
                .id(("otp-cell", index))
                .flex_none()
                .size(cell_size)
                .border_1()
                .border_color(if active {
                    token.border_focus
                } else {
                    token.border
                })
                .bg(token.surface)
                .text_color(token.foreground)
                .when(self.disabled, |this| this.opacity(0.5))
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .text_size(text_size)
                .when(!self.disabled, |this| {
                    this.cursor_text().on_mouse_down(
                        MouseButton::Left,
                        window.listener_for(&self.state, OtpState::on_input_mouse_down),
                    )
                })
                .child(match value {
                    Some(_) if state.masked => "*".to_string(),
                    Some(value) => value.to_string(),
                    None if active && blink_show => "|".to_string(),
                    None => String::new(),
                })
                .into_any_element();
            groups[group_index].push(cell);
        }

        h_flex()
            .id(("otp-input", self.state.entity_id()))
            .track_focus(&state.focus_handle)
            .when(!self.disabled, |this| {
                this.on_key_down(window.listener_for(&self.state, OtpState::on_key_down))
            })
            .gap(px(20.0))
            .children(
                groups
                    .into_iter()
                    .map(|cells| h_flex().gap(px(4.0)).children(cells)),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_is_normalized_and_truncated() {
        assert_eq!(sanitize_value("a1２3-4", 3), "123");
        assert_eq!(sanitize_value("123", 0), "");
    }
}
