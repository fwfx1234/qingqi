//! Tests for masking of sensitive SSH/FTP credential inputs (FIX-017).

use gpui::{Context, TestAppContext, Window};
use qingqi_ui::components::input::InputState;

fn build_masked_input(window: &mut Window, cx: &mut Context<InputState>) -> InputState {
    let mut state = InputState::new(window, cx)
        .placeholder("password")
        .default_value("".into());
    state.set_masked(true, window, cx);
    state
}

fn build_plain_input(window: &mut Window, cx: &mut Context<InputState>) -> InputState {
    InputState::new(window, cx)
        .placeholder("username")
        .default_value("root".into())
}

#[gpui::test]
fn masked_input_is_masked(cx: &mut TestAppContext) {
    let (entity, cx) = cx.add_window_view(build_masked_input);

    entity.read_with(cx, |state, _cx| {
        assert!(state.is_masked(), "password entity should be masked");
    });
}

#[gpui::test]
fn plain_input_is_not_masked(cx: &mut TestAppContext) {
    let (entity, cx) = cx.add_window_view(build_plain_input);

    entity.read_with(cx, |state, _cx| {
        assert!(
            !state.is_masked(),
            "plain input should not be masked by default"
        );
    });
}

#[gpui::test]
fn value_preserved_after_reset(cx: &mut TestAppContext) {
    let (entity, cx) = cx.add_window_view(build_masked_input);

    entity.update(cx, |state, cx| {
        state.reset_value("secret🔐中文", cx);
    });

    entity.read_with(cx, |state, _cx| {
        assert_eq!(
            state.value().as_ref(),
            "secret🔐中文",
            "value must round-trip unchanged"
        );
    });
}

#[gpui::test]
fn masked_flag_survives_reset(cx: &mut TestAppContext) {
    let (entity, cx) = cx.add_window_view(build_masked_input);

    entity.update(cx, |state, cx| {
        state.reset_value("p@ss", cx);
    });

    entity.read_with(cx, |state, _cx| {
        assert!(
            state.is_masked(),
            "masked flag must survive reset_value and not be reset"
        );
    });
}
