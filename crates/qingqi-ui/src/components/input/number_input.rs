//! NumberInput — input field with increment/decrement buttons.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, RenderOnce,
    StyleRefinement, Styled, Window, actions, prelude::FluentBuilder as _,
};

use super::{Input, InputState};

actions!(qingqi_number_input, [Increment, Decrement]);

const CONTEXT: &str = "QingqiNumberInput";

pub fn init(cx: &mut App) {
    cx.bind_keys(vec![
        KeyBinding::new("up", Increment, Some(CONTEXT)),
        KeyBinding::new("down", Decrement, Some(CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
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
    disabled: bool,
    style: StyleRefinement,
}

impl NumberInput {
    pub fn new(state: &Entity<InputState>) -> Self {
        Self { state: state.clone(), disabled: false, style: StyleRefinement::default() }
    }

    pub fn disabled(mut self, disabled: bool) -> Self { self.disabled = disabled; self }
}

impl Focusable for NumberInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}

impl Styled for NumberInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for NumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.clone();
        gpui::div()
            .id(("number-input", self.state.entity_id()))
            .key_context(CONTEXT)
            .on_action(window.listener_for(&state, |s, _: &Increment, window, cx| {
                s.focus(window, cx);
                cx.emit(NumberInputEvent::Step(StepAction::Increment));
            }))
            .on_action(window.listener_for(&state, |s, _: &Decrement, window, cx| {
                s.focus(window, cx);
                cx.emit(NumberInputEvent::Step(StepAction::Decrement));
            }))
            .flex_1()
            .child(Input::new(&self.state).disabled(self.disabled))
    }
}
