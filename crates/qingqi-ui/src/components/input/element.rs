//! TextElement — custom Element for rendering the text input field.

use gpui::{
    App, Bounds, Element, ElementId, ElementInputHandler, Entity, GlobalElementId,
    InspectorElementId, Hitbox, IntoElement, LayoutId, MouseButton, MouseMoveEvent,
    Pixels, SharedString, TextRun, Window, fill, point, px, relative, size,
};

use crate::token::tokens;

use super::{InputState, mode::InputMode};

pub(super) const RIGHT_MARGIN: Pixels = px(10.);
pub(super) const LINE_NUMBER_RIGHT_MARGIN: Pixels = px(10.);

pub struct TextElement {
    pub(crate) state: Entity<InputState>,
    placeholder: SharedString,
}

impl TextElement {
    pub fn new(state: Entity<InputState>) -> Self {
        Self { state, placeholder: SharedString::default() }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some("text-element".into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = window.request_layout(
            gpui::Style {
                flex_grow: 1.0,
                flex_shrink: 1.0,
                size: size(relative(1.).into(), relative(1.).into()),
                ..Default::default()
            },
            None,
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let line_height = window.line_height();
        let last_layout = super::LastLayout {
            line_height,
            line_number_width: px(0.0),
            visible_range: 0..1,
            visible_top: px(0.0),
            visible_range_offset: 0..0,
            lines: Vec::new(),
        };

        self.state.update(cx, |state, _cx| {
            state.last_layout = Some(last_layout);
            state.input_bounds = bounds;
        });

        window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Read state first, then drop the borrow before painting
        let (text, selected_range, show_cursor, placeholder_str, text_len, cursor_offset, focus_handle, ime_marked_range) = {
            let state = self.state.read(cx);
            (
                state.text.to_string(),
                state.selected_range,
                state.show_cursor(&*window, cx),
                state.placeholder.to_string(),
                state.text.len(),
                state.cursor(),
                state.focus_handle.clone(),
                state.ime_marked_range,
            )
        };

        // Register input handler so that key events are routed to this element's EntityInputHandler
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        let token = tokens(cx);
        let line_height = window.line_height();
        let font_size = window.rem_size() * 0.875; // ~14px for 16px rem
        let text_padding = px(8.0);

        // Paint background
        window.paint_quad(fill(bounds, token.surface));

        // Calculate text origin with padding
        let text_origin_x = bounds.origin.x + text_padding;
        let text_origin_y = bounds.origin.y + text_padding;

        // Helper: shape a text line and return the width
        fn measure_text(text: &str, font_size: Pixels, window: &Window, cx: &App, color: gpui::Hsla) -> (gpui::ShapedLine, Pixels) {
            let run = TextRun {
                len: text.len(),
                font: gpui::Font {
                    family: ".SystemUIFont".into(),
                    weight: gpui::FontWeight::default(),
                    style: gpui::FontStyle::Normal,
                    features: gpui::FontFeatures::default(),
                    fallbacks: None,
                },
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = window
                .text_system()
                .shape_line(text.to_string().into(), font_size, &[run], None);
            let width = shaped.width;
            (shaped, width)
        }

        // Paint the text (origin is the top of the line, text is centered within line_height)
        if text_len > 0 {
            let (shaped, _) = measure_text(&text, font_size, window, cx, token.foreground);
            _ = shaped.paint(
                point(text_origin_x, text_origin_y),
                line_height,
                window,
                cx,
            );
        }

        // Paint selection highlight
        if !selected_range.is_empty() {
            let sel_color = token.accent.opacity(0.3);
            let sel_start = selected_range.start;
            let sel_end = selected_range.end;

            let sel_start_x = if sel_start == 0 {
                px(0.0)
            } else {
                let text_before: String = text[..sel_start.min(text.len())].to_string();
                let (_, w) = measure_text(&text_before, font_size, window, cx, token.foreground);
                w
            };

            let sel_end_x = if sel_end == 0 {
                px(0.0)
            } else {
                let text_to: String = text[..sel_end.min(text.len())].to_string();
                let (_, w) = measure_text(&text_to, font_size, window, cx, token.foreground);
                w
            };

            let sel_bounds = Bounds::from_corners(
                point(text_origin_x + sel_start_x, text_origin_y),
                point(text_origin_x + sel_end_x, text_origin_y + line_height),
            );
            window.paint_quad(fill(sel_bounds, sel_color));
        }

        // Paint IME marked range underline (for Chinese/Japanese/Korean composition)
        if let Some(ref marked) = ime_marked_range {
            let mark_start = marked.start;
            let mark_end = marked.end;

            if mark_start < mark_end && mark_end <= text.len() {
                let mark_start_x = if mark_start == 0 {
                    px(0.0)
                } else {
                    let text_before: String = text[..mark_start].to_string();
                    let (_, w) = measure_text(&text_before, font_size, window, cx, token.foreground);
                    w
                };

                let mark_end_x = if mark_end == 0 {
                    px(0.0)
                } else {
                    let text_to: String = text[..mark_end].to_string();
                    let (_, w) = measure_text(&text_to, font_size, window, cx, token.foreground);
                    w
                };

                // Draw underline for IME composition text (at bottom of text line)
                let underline_y = text_origin_y + line_height - px(2.0);
                let underline_bounds = Bounds::from_corners(
                    point(text_origin_x + mark_start_x, underline_y),
                    point(text_origin_x + mark_end_x, underline_y + px(2.0)),
                );
                window.paint_quad(fill(underline_bounds, token.foreground));
            }
        }

        // Paint cursor — always visible when blink_visible is true (cursor is shown)
        if show_cursor {
            let cursor_x = if cursor_offset == 0 {
                px(0.0)
            } else {
                let text_before: String = text[..cursor_offset.min(text.len())].to_string();
                let (_, w) = measure_text(&text_before, font_size, window, cx, token.foreground);
                w
            };
            let cursor_height = line_height * 0.85;
            let cursor_bounds = Bounds::from_corners(
                point(text_origin_x + cursor_x, text_origin_y + (line_height - cursor_height) / 2.),
                point(text_origin_x + cursor_x + px(2.0), text_origin_y + (line_height + cursor_height) / 2.),
            );
            window.paint_quad(fill(cursor_bounds, token.accent));
        }

        // Paint placeholder
        if text_len == 0 && !placeholder_str.is_empty() {
            let placeholder_run = TextRun {
                len: placeholder_str.len(),
                font: gpui::Font {
                    family: ".SystemUIFont".into(),
                    weight: gpui::FontWeight::default(),
                    style: gpui::FontStyle::Normal,
                    features: gpui::FontFeatures::default(),
                    fallbacks: None,
                },
                color: token.foreground_placeholder,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let placeholder_shaped = window.text_system().shape_line(
                placeholder_str.into(),
                font_size,
                &[placeholder_run],
                None,
            );
            _ = placeholder_shaped.paint(
                point(text_origin_x, text_origin_y),
                line_height,
                window,
                cx,
            );
        }

        // Paint mouse drag listeners
        window.on_mouse_event({
            let state = self.state.clone();
            move |event: &MouseMoveEvent, _, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    state.update(cx, |state, cx| {
                        state.on_drag_move(event, window, cx);
                    });
                }
            }
        });
    }
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
