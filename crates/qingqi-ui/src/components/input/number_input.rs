//! Numeric input with keyboard and button stepping.

use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, actions, div,
    prelude::FluentBuilder as _, px,
};

use crate::components::{Disableable, Icon, Sizable, Size, StyledExt as _, h_flex};
use crate::icon;

use super::{Input, InputState, MaskPattern};

actions!(qingqi_number_input, [Increment, Decrement]);

const CONTEXT: &str = "QingqiNumberInput";

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", Increment, Some(CONTEXT)),
        KeyBinding::new("down", Decrement, Some(CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepAction {
    Decrement,
    Increment,
}

pub enum NumberInputEvent {
    Step(StepAction),
}

impl EventEmitter<NumberInputEvent> for InputState {}

#[derive(IntoElement)]
pub struct NumberInput {
    state: Entity<InputState>,
    placeholder: SharedString,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    appearance: bool,
    disabled: bool,
    size: Size,
    min: Option<f64>,
    max: Option<f64>,
    step: f64,
    style: StyleRefinement,
}

impl NumberInput {
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            placeholder: SharedString::default(),
            prefix: None,
            suffix: None,
            appearance: true,
            disabled: false,
            size: Size::Medium,
            min: None,
            max: None,
            step: 1.0,
            style: StyleRefinement::default(),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        if step.is_finite() && step > 0.0 {
            self.step = step;
        }
        self
    }

    fn update_configuration(&self, cx: &mut App) {
        let placeholder = self.placeholder.clone();
        let min = self.min;
        let max = self.max;
        let step = self.step;
        let disabled = self.disabled;
        self.state.update(cx, |state, cx| {
            let mut changed = false;
            if state.placeholder != placeholder {
                state.placeholder = placeholder;
                changed = true;
            }
            if state.disabled != disabled {
                state.disabled = disabled;
                changed = true;
            }
            if state.number_min != min || state.number_max != max || state.number_step != step {
                state.number_min = min;
                state.number_max = max;
                state.number_step = step;
                changed = true;
            }
            if state.mask_pattern.is_none() {
                state.mask_pattern = MaskPattern::number(None);
                changed = true;
            }
            if changed {
                cx.notify();
            }
        });
    }

    fn step_state(
        state: &Entity<InputState>,
        action: StepAction,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, cx| {
            state.focus(window, cx);
            state.on_number_input_step(action, window, cx);
        });
    }
}

impl InputState {
    fn on_action_increment(&mut self, _: &Increment, window: &mut Window, cx: &mut Context<Self>) {
        self.on_number_input_step(StepAction::Increment, window, cx);
    }

    fn on_action_decrement(&mut self, _: &Decrement, window: &mut Window, cx: &mut Context<Self>) {
        self.on_number_input_step(StepAction::Decrement, window, cx);
    }

    fn on_number_input_step(
        &mut self,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        let current = self.unmask_value();
        let Some(value) = step_value(
            current.as_ref(),
            action,
            self.number_step,
            self.number_min,
            self.number_max,
        ) else {
            return;
        };
        self.set_value(&value, window, cx);
        cx.emit(NumberInputEvent::Step(action));
    }
}

fn step_value(
    value: &str,
    action: StepAction,
    step: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> Option<String> {
    fn fraction_digits(value: &str) -> usize {
        value
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len())
    }

    let current = value.trim().parse::<f64>().ok();
    let signed_step = match action {
        StepAction::Increment => step,
        StepAction::Decrement => -step,
    };
    let mut next = current.unwrap_or(0.0) + signed_step;
    let mut digits = fraction_digits(value).max(fraction_digits(&step.to_string()));
    if let Some(min) = min
        && next < min
    {
        next = min;
        digits = digits.max(fraction_digits(&min.to_string()));
    }
    if let Some(max) = max
        && next > max
    {
        next = max;
        digits = digits.max(fraction_digits(&max.to_string()));
    }
    if let Some(current) = current {
        let moved = match action {
            StepAction::Increment => next > current,
            StepAction::Decrement => next < current,
        };
        if !moved {
            return None;
        }
    }
    Some(format!("{next:.digits$}"))
}

impl Focusable for NumberInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}

impl Sizable for NumberInput {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Disableable for NumberInput {
    fn disabled(self, disabled: bool) -> Self {
        NumberInput::disabled(self, disabled)
    }
}

impl Styled for NumberInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for NumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.update_configuration(cx);
        let token = crate::token::tokens(cx);
        let decrement_state = self.state.clone();
        let increment_state = self.state.clone();
        let button_size = match self.size {
            Size::Large => px(44.0),
            Size::Medium => px(36.0),
            Size::Small => px(28.0),
            Size::XSmall => px(24.0),
            Size::Size(value) => value,
        };
        let make_button = |id, icon: Icon, state: Entity<InputState>, action| {
            div()
                .id(id)
                .flex_none()
                .size(button_size)
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(token.border)
                .when(self.appearance, |this| this.bg(token.surface))
                .when(self.disabled, |this| this.opacity(0.5))
                .when(!self.disabled, |this| {
                    this.cursor_pointer().on_click(move |_, window, cx| {
                        Self::step_state(&state, action, window, cx);
                    })
                })
                .child(icon.size(px(15.0)))
        };

        h_flex()
            .id(("number-input", self.state.entity_id()))
            .key_context(CONTEXT)
            .on_action(window.listener_for(&self.state, InputState::on_action_increment))
            .on_action(window.listener_for(&self.state, InputState::on_action_decrement))
            .w_full()
            .rounded(px(6.0))
            .refine_style(&self.style)
            .child(make_button(
                "number-input-decrement",
                icon!(minus),
                decrement_state,
                StepAction::Decrement,
            ))
            .child(
                Input::new(&self.state)
                    .appearance(self.appearance)
                    .bordered(false)
                    .focus_bordered(false)
                    .with_size(self.size)
                    .disabled(self.disabled)
                    .when_some(self.prefix, |input, prefix| input.prefix(prefix))
                    .when_some(self.suffix, |input, suffix| input.suffix(suffix)),
            )
            .child(make_button(
                "number-input-increment",
                icon!(plus),
                increment_state,
                StepAction::Increment,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_preserves_decimal_precision() {
        assert_eq!(
            step_value("0.1", StepAction::Increment, 0.2, None, None),
            Some("0.3".to_string())
        );
    }

    #[test]
    fn step_respects_boundaries() {
        assert_eq!(
            step_value("10", StepAction::Increment, 1.0, None, Some(10.0)),
            None
        );
        assert_eq!(
            step_value("", StepAction::Decrement, 2.0, Some(0.0), None),
            Some("0".to_string())
        );
    }
}
