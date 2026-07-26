//! Input — the builder-style element that wraps an [`InputState`].

use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, DefiniteLength, Entity, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, RenderOnce, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, px, relative,
};

use super::{InputState, LINE_HEIGHT_REMS, input_line_height};
use crate::components::{Disableable, Icon, Sizable, Size, StyledExt as _};
use crate::icon;

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
    disabled: Option<bool>,
    bordered: bool,
    focus_bordered: bool,
    size: Size,
    on_submit: Option<Arc<dyn Fn(&str, &mut Window, &mut Context<InputState>)>>,
    on_blur: Option<Arc<dyn Fn(&mut Window, &mut Context<InputState>)>>,
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
            disabled: None,
            bordered: true,
            focus_bordered: true,
            size: Size::Medium,
            on_submit: None,
            on_blur: None,
        }
    }

    pub fn on_blur(mut self, f: impl Fn(&mut Window, &mut Context<InputState>) + 'static) -> Self {
        self.on_blur = Some(Arc::new(f));
        self
    }

    pub fn on_submit(
        mut self,
        f: impl Fn(&str, &mut Window, &mut Context<InputState>) + 'static,
    ) -> Self {
        self.on_submit = Some(Arc::new(f));
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
        self.disabled = Some(disabled);
        self
    }
}

impl Styled for Input {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for Input {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Disableable for Input {
    fn disabled(self, disabled: bool) -> Self {
        Input::disabled(self, disabled)
    }
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.on_submit.is_some() {
            let on_submit = self.on_submit.unwrap();
            cx.update_entity(&self.state, |state, _cx| {
                state.on_submit = Some(on_submit);
            });
        }
        if self.on_blur.is_some() {
            let on_blur = self.on_blur.unwrap();
            cx.update_entity(&self.state, |state, _cx| {
                state.on_blur = Some(on_blur);
            });
        }
        if let Some(disabled) = self.disabled {
            cx.update_entity(&self.state, |state, cx| {
                if state.disabled != disabled {
                    state.disabled = disabled;
                    cx.notify();
                }
            });
        }

        let token = crate::token::tokens(cx);
        let bg = if self.appearance {
            token.surface
        } else {
            gpui::hsla(0.0, 0.0, 0.0, 0.0)
        };
        let (focused, show_clear_button, disabled, masked, loading, rows, focus_handle) = {
            let state = self.state.read(cx);
            (
                state.focus_handle.is_focused(window),
                self.cleanable && !state.is_empty() && !state.disabled && !state.loading,
                state.disabled,
                state.masked,
                state.loading,
                state.mode.rows(),
                state.focus_handle.clone(),
            )
        };

        let border_color = if focused && self.focus_bordered {
            token.border_focus
        } else {
            token.border
        };

        let search_panel = render_search_panel(&self.state, cx);
        let content = div()
            .flex_1()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(self.state.clone());
        let clear_state = self.state.clone();
        let mask_state = self.state.clone();
        let size_height = match self.size {
            Size::Large => px(44.0),
            Size::Medium => px(36.0),
            Size::Small => px(28.0),
            Size::XSmall => px(24.0),
            Size::Size(value) => value,
        };
        let default_height = if rows > 1 {
            input_line_height(window) * rows as f32 + px(16.0)
        } else {
            size_height
        };

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
            .on_action(window.listener_for(&self.state, InputState::select_left))
            .on_action(window.listener_for(&self.state, InputState::select_right))
            .on_action(window.listener_for(&self.state, InputState::select_up))
            .on_action(window.listener_for(&self.state, InputState::select_down))
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
            .on_action(window.listener_for(&self.state, InputState::indent_inline))
            .on_action(window.listener_for(&self.state, InputState::outdent_inline))
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
            .on_action(window.listener_for(&self.state, InputState::on_action_escape))
            .on_action(window.listener_for(&self.state, InputState::go_to_definition))
            .on_action(window.listener_for(&self.state, InputState::toggle_code_actions))
            .on_key_down(window.listener_for(&self.state, InputState::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&self.state, InputState::on_mouse_down),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&self.state, InputState::on_mouse_up),
            )
            .on_mouse_move(window.listener_for(&self.state, InputState::on_mouse_move))
            .on_scroll_wheel(window.listener_for(&self.state, InputState::on_scroll_wheel))
            .flex()
            .relative()
            .w_full()
            .line_height(LINE_HEIGHT_REMS)
            .when(!disabled, |this| this.cursor_text())
            .when(self.height.is_none(), |this| this.h(default_height))
            .when_some(self.height, |this, height| this.h(height))
            .items_center()
            .when(self.appearance, |this| {
                this.bg(bg).rounded(px(6.0)).when(self.bordered, |this| {
                    this.border_color(border_color).border_1()
                })
            })
            .items_center()
            .gap(px(6.0))
            .when_some(self.prefix, |this, prefix| this.child(prefix))
            .child(content)
            .when(loading, |this| this.child(icon!(loader).size(px(16.0))))
            .when(self.mask_toggle, |this| {
                this.child(
                    div()
                        .id(("input-mask-toggle", self.state.entity_id()))
                        .size(px(24.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_click(move |_, window, cx| {
                            mask_state.update(cx, |state, cx| {
                                state.set_masked(!state.is_masked(), window, cx);
                            });
                        })
                        .child(if masked { icon!(eye_off) } else { icon!(eye) }),
                )
            })
            .when(show_clear_button, |this| {
                this.child(
                    div()
                        .id(("input-clear", self.state.entity_id()))
                        .size(px(24.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_click(move |_, window, cx| {
                            clear_state.update(cx, |state, cx| {
                                state.clean(window, cx);
                                state.focus(window, cx);
                            });
                        })
                        .child(icon!(x)),
                )
            })
            .when_some(self.suffix, |this, suffix| this.child(suffix))
            .when_some(search_panel, |this, panel| this.child(panel))
            .refine_style(&self.style)
    }
}

fn render_search_panel(state: &Entity<InputState>, cx: &mut App) -> Option<AnyElement> {
    let (query_input, replace_input, case_insensitive, replace_mode, label) = {
        let state = state.read(cx);
        let panel = state.search_panel.as_ref()?;
        if !panel.open {
            return None;
        }
        (
            panel.query_input.clone(),
            panel.replace_input.clone(),
            panel.case_insensitive,
            panel.replace_mode,
            state
                .search_matcher
                .as_ref()
                .map_or_else(|| "0/0".to_string(), |matcher| matcher.label()),
        )
    };
    let token = crate::token::tokens(cx);
    let case_state = state.clone();
    let mode_state = state.clone();
    let previous_state = state.clone();
    let next_state = state.clone();
    let replace_state = state.clone();
    let replace_all_state = state.clone();
    let close_state = state.clone();
    let icon_button = |id, icon: Icon| {
        div()
            .id(id)
            .flex_none()
            .size(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .cursor_pointer()
            .child(icon.size(px(14.0)))
    };

    Some(
        div()
            .id(("input-search-panel", state.entity_id()))
            .key_context(super::search::CONTEXT)
            .absolute()
            .top(px(4.0))
            .right(px(4.0))
            .w_full()
            .max_w(px(380.0))
            .p(px(4.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .bg(token.surface)
            .border_1()
            .border_color(token.border)
            .rounded(px(6.0))
            .shadow_sm()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(3.0))
                    .child(
                        div().flex_1().child(
                            Input::new(&query_input)
                                .small()
                                .h(px(28.0))
                                .prefix(icon!(search).size(px(14.0))),
                        ),
                    )
                    .child(div().min_w(px(38.0)).text_size(px(11.0)).child(label))
                    .child(
                        icon_button("input-search-case", icon!(case_sensitive))
                            .when(!case_insensitive, |this| this.bg(token.surface_active))
                            .on_click(move |_, _, cx| {
                                case_state.update(cx, |state, cx| {
                                    let (query, case_sensitive) = {
                                        let Some(panel) = &mut state.search_panel else {
                                            return;
                                        };
                                        panel.case_insensitive = !panel.case_insensitive;
                                        (
                                            panel.query_input.read(cx).value(),
                                            !panel.case_insensitive,
                                        )
                                    };
                                    state.set_search_query(query.as_ref(), case_sensitive, cx);
                                });
                            }),
                    )
                    .child(
                        icon_button("input-search-previous", icon!(chevron_up)).on_click(
                            move |_, _, cx| {
                                previous_state.update(cx, |state, cx| {
                                    state.previous_match(cx);
                                });
                            },
                        ),
                    )
                    .child(
                        icon_button("input-search-next", icon!(chevron_down)).on_click(
                            move |_, _, cx| {
                                next_state.update(cx, |state, cx| {
                                    state.next_match(cx);
                                });
                            },
                        ),
                    )
                    .child(
                        icon_button("input-search-replace-mode", icon!(replace))
                            .when(replace_mode, |this| this.bg(token.surface_active))
                            .on_click(move |_, _, cx| {
                                mode_state.update(cx, |state, cx| {
                                    if let Some(panel) = &mut state.search_panel {
                                        panel.replace_mode = !panel.replace_mode;
                                    }
                                    cx.notify();
                                });
                            }),
                    )
                    .child(icon_button("input-search-close", icon!(x)).on_click(
                        move |_, window, cx| {
                            close_state.update(cx, |state, cx| {
                                if let Some(panel) = &mut state.search_panel {
                                    panel.open = false;
                                }
                                state.focus(window, cx);
                                cx.notify();
                            });
                        },
                    )),
            )
            .when(replace_mode, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&replace_input).small().h(px(28.0))),
                        )
                        .child(
                            div()
                                .id("input-search-replace-current")
                                .h(px(26.0))
                                .px(px(7.0))
                                .flex()
                                .items_center()
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .text_size(px(11.0))
                                .child("Replace")
                                .on_click(move |_, window, cx| {
                                    replace_state.update(cx, |state, cx| {
                                        let replacement = state
                                            .search_panel
                                            .as_ref()
                                            .map(|panel| panel.replace_input.read(cx).value())
                                            .unwrap_or_default();
                                        state.replace_current_match(
                                            replacement.as_ref(),
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        )
                        .child(
                            div()
                                .id("input-search-replace-all")
                                .h(px(26.0))
                                .px(px(7.0))
                                .flex()
                                .items_center()
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .text_size(px(11.0))
                                .child("All")
                                .on_click(move |_, window, cx| {
                                    replace_all_state.update(cx, |state, cx| {
                                        let replacement = state
                                            .search_panel
                                            .as_ref()
                                            .map(|panel| panel.replace_input.read(cx).value())
                                            .unwrap_or_default();
                                        state.replace_all_matches(replacement.as_ref(), window, cx);
                                    });
                                }),
                        ),
                )
            })
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use gpui::{Context, EntityInputHandler as _, IntoElement, Render, TestAppContext};

    use super::*;

    struct InputHost {
        state: Entity<InputState>,
    }

    impl Render for InputHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(Input::new(&self.state))
        }
    }

    #[gpui::test]
    fn default_render_preserves_entity_disabled_state(cx: &mut TestAppContext) {
        let (host, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| {
                let mut state = InputState::new(window, cx);
                state.set_disabled(true, cx);
                state
            });
            InputHost { state }
        });

        cx.update(|_window, _cx| {});
        let state = host.read_with(cx, |host, _cx| host.state.clone());
        state.read_with(cx, |state, _cx| assert!(state.disabled));
    }

    #[gpui::test]
    fn mounts_and_edits_without_root_or_installed_tokens(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::components::init(cx);
            crate::components::init(cx);
            let token = crate::token::tokens(cx);
            assert!(!token.is_dark());
        });
        let (host, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx));
            InputHost { state }
        });
        let state = host.read_with(cx, |host, _| host.state.clone());
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.focus(window, cx);
                state.replace_text_in_range(None, "独立💝", window, cx);
            });
        });
        state.read_with(cx, |state, _| {
            assert_eq!(state.value().as_ref(), "独立💝");
        });
    }
}
