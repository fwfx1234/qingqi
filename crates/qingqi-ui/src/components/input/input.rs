//! Input — the builder-style element that wraps an [`InputState`].

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, DefiniteLength, Entity, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, RenderOnce, StyleRefinement,
    Styled, Window, div, px, relative,
};
use serde::Deserialize;

use super::InputState;

const LINE_HEIGHT: gpui::Rems = gpui::Rems(1.25);

/// A text input element bound to an [`InputState`].
#[derive(IntoElement)]
pub struct Input {
    state: Entity<InputState>,
    style: StyleRefinement,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    height: Option<DefiniteLength>,
    appearance: bool,
    cleanable: bool,
    mask_toggle: bool,
    disabled: bool,
    bordered: bool,
    focus_bordered: bool,
}

impl Input {
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
            prefix: None,
            suffix: None,
            height: None,
            appearance: true,
            cleanable: false,
            mask_toggle: false,
            disabled: false,
            bordered: true,
            focus_bordered: true,
        }
    }

    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    pub fn h_full(mut self) -> Self {
        self.height = Some(relative(1.));
        self
    }

    pub fn h(mut self, height: impl Into<DefiniteLength>) -> Self {
        self.height = Some(height.into());
        self
    }

    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn focus_bordered(mut self, bordered: bool) -> Self {
        self.focus_bordered = bordered;
        self
    }

    /// Remove all borders and focus outlines
    pub fn outline(mut self) -> Self {
        self.bordered = false;
        self.focus_bordered = false;
        self.style.box_shadow = None;
        self
    }

    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.cleanable = cleanable;
        self
    }

    pub fn mask_toggle(mut self) -> Self {
        self.mask_toggle = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn gap_0(mut self) -> Self {
        // self.style.gap = Some(gpui::Rems(0.).into());
        self
    }

    pub fn rounded_none(mut self) -> Self {
        self
    }

    pub fn shadow_none(self) -> Self { self }
    pub fn small(self) -> Self { self }
    pub fn xsmall(self) -> Self { self }
}

impl Styled for Input {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let token = crate::token::tokens(cx);
        let bg = if self.appearance { token.surface } else { gpui::hsla(0.0, 0.0, 0.0, 0.0) };
        let focused = state.focus_handle.is_focused(window);
        println!("!!! Input::render called, entity_id={:?}, focused={}", self.state.entity_id(), focused);
        let show_clear_button = self.cleanable && !state.is_empty() && !self.disabled;

        let border_color = if focused && self.focus_bordered {
            token.border_focus
        } else {
            token.border
        };

        let focus_handle = state.focus_handle.clone();
        let content = self.state.clone();

        drop(state);

        div()
            .id(("input", self.state.entity_id()))
            .key_context(super::CONTEXT)
            .track_focus(&focus_handle)
            .on_action(window.listener_for(&self.state, InputState::on_action_enter))
            .on_action(window.listener_for(&self.state, InputState::backspace))
            .on_action(window.listener_for(&self.state, InputState::delete))
            .on_action(window.listener_for(&self.state, InputState::undo))
            .on_action(window.listener_for(&self.state, InputState::redo))
            .on_action(window.listener_for(&self.state, InputState::copy))
            .on_action(window.listener_for(&self.state, InputState::cut))
            .on_action(window.listener_for(&self.state, InputState::paste))
            .on_action(window.listener_for(&self.state, InputState::select_all))
            .on_action(window.listener_for(&self.state, InputState::left))
            .on_action(window.listener_for(&self.state, InputState::right))
            .on_action(window.listener_for(&self.state, InputState::up))
            .on_action(window.listener_for(&self.state, InputState::down))
            .on_action(window.listener_for(&self.state, InputState::home))
            .on_action(window.listener_for(&self.state, InputState::end))
            .on_action(window.listener_for(&self.state, InputState::page_up))
            .on_action(window.listener_for(&self.state, InputState::page_down))
            .on_action(window.listener_for(&self.state, InputState::delete_to_beginning_of_line))
            .on_action(window.listener_for(&self.state, InputState::delete_to_end_of_line))
            .on_action(window.listener_for(&self.state, InputState::delete_to_previous_word_start))
            .on_action(window.listener_for(&self.state, InputState::delete_to_next_word_end))
            .on_action(window.listener_for(&self.state, InputState::indent))
            .on_action(window.listener_for(&self.state, InputState::outdent))
            .on_action(window.listener_for(&self.state, InputState::select_to_start))
            .on_action(window.listener_for(&self.state, InputState::select_to_end))
            .on_action(window.listener_for(&self.state, InputState::select_to_start_of_line))
            .on_action(window.listener_for(&self.state, InputState::select_to_end_of_line))
            .on_action(window.listener_for(&self.state, InputState::select_to_previous_word))
            .on_action(window.listener_for(&self.state, InputState::select_to_next_word))
            .on_action(window.listener_for(&self.state, InputState::move_to_start))
            .on_action(window.listener_for(&self.state, InputState::move_to_end))
            .on_action(window.listener_for(&self.state, InputState::move_to_previous_word))
            .on_action(window.listener_for(&self.state, InputState::move_to_next_word))
            .on_action(window.listener_for(&self.state, InputState::show_character_palette))
            .on_action(window.listener_for(&self.state, InputState::on_action_search))
            .on_key_down(window.listener_for(&self.state, InputState::on_key_down))
            .on_mouse_down(MouseButton::Left, window.listener_for(&self.state, InputState::on_mouse_down))
            .on_mouse_up(MouseButton::Left, window.listener_for(&self.state, InputState::on_mouse_up))
            .on_mouse_move(window.listener_for(&self.state, InputState::on_mouse_move))
            .on_scroll_wheel(window.listener_for(&self.state, InputState::on_scroll_wheel))
            .flex_1()
            .line_height(LINE_HEIGHT)
            .cursor_text()
            .when(self.height.is_none(), |this| this.h_full())
            .items_center()
            .when(self.appearance, |this| {
                this.bg(bg)
                    .rounded(px(6.0))
                    .when(self.bordered, |this| {
                        this.border_color(border_color).border_1()
                    })
            })
            .items_center()
            .child(content)
    }
}
