//! TextElement — custom Element for rendering the text input field.

use gpui::{
    App, Bounds, Element, ElementId, ElementInputHandler, Entity, GlobalElementId, Hitbox,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseMoveEvent, Pixels, SharedString,
    TextRun, Window, fill, point, px, relative, size,
};

use crate::token::tokens;

use super::{
    InputState, RopeExt as _, TEXT_PADDING, input_font_size, input_line_height, input_text_top,
};

const MASK_CHAR: char = '•';

/// Clamp a byte offset to the previous valid UTF-8 char boundary.
fn clamp_to_char_boundary(text: &str, byte_offset: usize) -> usize {
    let mut offset = byte_offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Map original text to masked display text (one mask char per char).
fn masked_display(text: &str) -> String {
    std::iter::repeat(MASK_CHAR)
        .take(text.chars().count())
        .collect()
}

/// Map original text prefix (up to byte_offset) to masked display prefix.
fn masked_display_prefix(text: &str, byte_offset: usize) -> String {
    let clamped = clamp_to_char_boundary(text, byte_offset);
    let char_count = text[..clamped].chars().count();
    std::iter::repeat(MASK_CHAR).take(char_count).collect()
}

/// Compute the display prefix for measuring visual width up to a byte offset.
/// When `masked` is true, returns a string of mask chars; otherwise the original prefix.
fn display_prefix(text: &str, byte_offset: usize, masked: bool) -> String {
    if masked {
        masked_display_prefix(text, byte_offset)
    } else {
        let offset = byte_offset.min(text.len());
        text[..offset].to_string()
    }
}

#[derive(Clone)]
pub(crate) struct DisplaySegment {
    pub row: usize,
    pub first_in_row: bool,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub(crate) fn display_segments(state: &InputState) -> Vec<DisplaySegment> {
    let mut result = Vec::new();
    for row in 0..state.text.lines_len() {
        let line = state.text.slice_line(row).to_string();
        let line_start = state.text.line_start_offset(row);
        if state.mode.is_offset_folded(line_start) {
            continue;
        }
        let ranges = state
            .soft_wrap_enabled
            .then(|| state.text_wrapper.line(row))
            .flatten()
            .map(|item| item.wrapped_lines.clone())
            .unwrap_or_else(|| vec![0..line.len()]);

        for (local_row, range) in ranges.into_iter().enumerate() {
            let start = clamp_to_char_boundary(&line, range.start);
            let end = clamp_to_char_boundary(&line, range.end.max(start)).min(line.len());
            result.push(DisplaySegment {
                row,
                first_in_row: local_row == 0,
                start: line_start + start,
                end: line_start + end,
                text: line[start..end].to_string(),
            });
        }
    }
    if result.is_empty() {
        result.push(DisplaySegment {
            row: 0,
            first_in_row: true,
            start: 0,
            end: 0,
            text: String::new(),
        });
    }
    result
}

pub struct TextElement {
    pub(crate) state: Entity<InputState>,
    placeholder: SharedString,
}

impl TextElement {
    pub fn new(state: Entity<InputState>) -> Self {
        Self {
            state,
            placeholder: SharedString::default(),
        }
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
        let line_height = input_line_height(window);
        let font_size = input_font_size(window);
        self.state.update(cx, |state, cx| {
            state
                .text_wrapper
                .set_font(window.text_style().font(), font_size, cx);
            let line_number_width = if state.mode.line_number() {
                px(44.0)
            } else {
                px(0.0)
            };
            let wrap_width = if state.soft_wrap_enabled {
                Some((bounds.size.width - TEXT_PADDING * 2. - line_number_width).max(px(1.0)))
            } else {
                None
            };
            state.text_wrapper.set_wrap_width(wrap_width, cx);
            state.text_wrapper.prepare_if_need(&state.text.clone(), cx);
            let scroll_y = -state.scroll_handle.offset().y;
            let first_display = (scroll_y / line_height).floor().max(0.0) as usize;
            let visible_count = ((bounds.size.height / line_height).ceil() as usize).max(1) + 1;
            let segments = display_segments(state);
            let first_row = segments.get(first_display).map_or(0, |segment| segment.row);
            let last_row = segments
                .get((first_display + visible_count).min(segments.len().saturating_sub(1)))
                .map_or(first_row + 1, |segment| segment.row + 1);
            state.last_layout = Some(super::LastLayout {
                line_height,
                line_number_width,
                visible_range: first_row..last_row,
                visible_top: -(scroll_y % line_height),
                visible_range_offset: first_display..first_display + visible_count,
                lines: Vec::new(),
            });
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
        let (
            segments,
            selected_range,
            show_cursor,
            placeholder,
            cursor,
            focus_handle,
            ime_marked_range,
            masked,
            scroll_offset,
            line_number_width,
            matched_ranges,
            highlights,
            diagnostics,
            indent_guides,
            tab_size,
            is_empty,
            multi_line,
        ) = {
            let state = self.state.read(cx);
            (
                display_segments(state),
                state.selected_range,
                state.show_cursor(window, cx),
                state.placeholder.to_string(),
                state.cursor(),
                state.focus_handle.clone(),
                state.ime_marked_range,
                state.masked,
                state.scroll_handle.offset(),
                if state.mode.line_number() {
                    px(44.0)
                } else {
                    px(0.0)
                },
                state
                    .search_matcher
                    .as_ref()
                    .map(|matcher| matcher.matched_ranges.clone())
                    .unwrap_or_default(),
                state.mode.highlights().to_vec(),
                state
                    .mode
                    .diagnostics()
                    .map(|diagnostics| diagnostics.items().to_vec())
                    .unwrap_or_default(),
                state.mode.has_indent_guides(),
                state.mode.tab_size().tab_size.max(1),
                state.text.len() == 0,
                state.mode.is_multi_line(),
            )
        };

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        let token = tokens(cx);
        let line_height = input_line_height(window);
        let font_size = input_font_size(window);
        let text_x = bounds.left() + TEXT_PADDING + line_number_width + scroll_offset.x;
        let first_y = input_text_top(bounds, multi_line, window) + scroll_offset.y;
        let cursor_segment = segments
            .iter()
            .rposition(|segment| cursor >= segment.start && cursor <= segment.end);

        fn shape_line(
            text: &str,
            font_size: Pixels,
            color: gpui::Hsla,
            window: &Window,
        ) -> gpui::ShapedLine {
            let text = text.replace(['\n', '\r'], "");
            let run = TextRun {
                len: text.len(),
                font: window.text_style().font(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_line(text.into(), font_size, &[run], None)
        }

        fn shape_highlighted_line(
            text: &str,
            segment_start: usize,
            highlights: &[super::HighlightSpan],
            font_size: Pixels,
            default_color: gpui::Hsla,
            window: &Window,
        ) -> gpui::ShapedLine {
            let text = text.replace(['\n', '\r'], "");
            let font = window.text_style().font();
            let mut runs = Vec::new();
            let mut cursor = 0;
            for highlight in highlights {
                let start = highlight
                    .range
                    .start
                    .saturating_sub(segment_start)
                    .min(text.len());
                let end = highlight
                    .range
                    .end
                    .saturating_sub(segment_start)
                    .min(text.len());
                let start = clamp_to_char_boundary(&text, start).max(cursor);
                let end = clamp_to_char_boundary(&text, end).max(start);
                if start > cursor {
                    runs.push(TextRun {
                        len: start - cursor,
                        font: font.clone(),
                        color: default_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    });
                }
                if end > start {
                    runs.push(TextRun {
                        len: end - start,
                        font: font.clone(),
                        color: highlight.color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    });
                    cursor = end;
                }
            }
            if cursor < text.len() || runs.is_empty() {
                runs.push(TextRun {
                    len: text.len() - cursor,
                    font,
                    color: default_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                });
            }
            window
                .text_system()
                .shape_line(text.into(), font_size, &runs, None)
        }

        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for (display_row, segment) in segments.iter().enumerate() {
                let y = first_y + line_height * display_row as f32;
                if y + line_height < bounds.top() || y > bounds.bottom() {
                    continue;
                }
                if line_number_width > px(0.0) && segment.first_in_row {
                    let number = (segment.row + 1).to_string();
                    let shaped =
                        shape_line(&number, font_size * 0.85, token.muted_foreground, window);
                    let number_x = bounds.left() + line_number_width - shaped.width - px(8.0);
                    _ = shaped.paint(point(number_x, y), line_height, window, cx);
                }

                if indent_guides && segment.first_in_row {
                    let indent = segment
                        .text
                        .chars()
                        .take_while(|ch| matches!(ch, ' ' | '\t'))
                        .map(|ch| if ch == '\t' { tab_size } else { 1 })
                        .sum::<usize>();
                    for column in (tab_size..indent).step_by(tab_size) {
                        let width =
                            shape_line(&" ".repeat(column), font_size, token.foreground, window)
                                .width;
                        window.paint_quad(fill(
                            Bounds::from_corners(
                                point(text_x + width, y),
                                point(text_x + width + px(1.0), y + line_height),
                            ),
                            token.border.opacity(0.7),
                        ));
                    }
                }

                for range in matched_ranges.iter() {
                    let start = range.start.max(segment.start);
                    let end = range.end.min(segment.end);
                    if start < end {
                        let start_width = shape_line(
                            &display_prefix(&segment.text, start - segment.start, masked),
                            font_size,
                            token.foreground,
                            window,
                        )
                        .width;
                        let end_width = shape_line(
                            &display_prefix(&segment.text, end - segment.start, masked),
                            font_size,
                            token.foreground,
                            window,
                        )
                        .width;
                        window.paint_quad(fill(
                            Bounds::from_corners(
                                point(text_x + start_width, y),
                                point(text_x + end_width, y + line_height),
                            ),
                            token.warning.opacity(0.22),
                        ));
                    }
                }

                let selection_start = selected_range.start.max(segment.start);
                let selection_end = selected_range.end.min(segment.end);
                if selection_start < selection_end {
                    let start_width = shape_line(
                        &display_prefix(&segment.text, selection_start - segment.start, masked),
                        font_size,
                        token.foreground,
                        window,
                    )
                    .width;
                    let end_width = shape_line(
                        &display_prefix(&segment.text, selection_end - segment.start, masked),
                        font_size,
                        token.foreground,
                        window,
                    )
                    .width;
                    window.paint_quad(fill(
                        Bounds::from_corners(
                            point(text_x + start_width, y),
                            point(text_x + end_width, y + line_height),
                        ),
                        token.accent.opacity(0.3),
                    ));
                }

                let display_text = if masked {
                    masked_display(&segment.text)
                } else {
                    segment.text.clone()
                };
                let shaped = if masked || highlights.is_empty() {
                    shape_line(&display_text, font_size, token.foreground, window)
                } else {
                    shape_highlighted_line(
                        &display_text,
                        segment.start,
                        &highlights,
                        font_size,
                        token.foreground,
                        window,
                    )
                };
                _ = shaped.paint(point(text_x, y), line_height, window, cx);

                for diagnostic in diagnostics.iter() {
                    let start = diagnostic.range.start.max(segment.start);
                    let end = diagnostic.range.end.min(segment.end);
                    if start >= end {
                        continue;
                    }
                    let start_width = shape_line(
                        &display_prefix(&segment.text, start - segment.start, masked),
                        font_size,
                        token.foreground,
                        window,
                    )
                    .width;
                    let end_width = shape_line(
                        &display_prefix(&segment.text, end - segment.start, masked),
                        font_size,
                        token.foreground,
                        window,
                    )
                    .width;
                    let color = match diagnostic.severity {
                        super::DiagnosticSeverity::Error => token.danger,
                        super::DiagnosticSeverity::Warning => token.warning,
                        super::DiagnosticSeverity::Information => token.info,
                        super::DiagnosticSeverity::Hint => token.muted_foreground,
                    };
                    window.paint_quad(fill(
                        Bounds::from_corners(
                            point(text_x + start_width, y + line_height - px(1.0)),
                            point(text_x + end_width, y + line_height),
                        ),
                        color,
                    ));
                }

                if let Some(marked) = ime_marked_range {
                    let start = marked.start.max(segment.start);
                    let end = marked.end.min(segment.end);
                    if start < end {
                        let start_width = shape_line(
                            &display_prefix(&segment.text, start - segment.start, masked),
                            font_size,
                            token.foreground,
                            window,
                        )
                        .width;
                        let end_width = shape_line(
                            &display_prefix(&segment.text, end - segment.start, masked),
                            font_size,
                            token.foreground,
                            window,
                        )
                        .width;
                        window.paint_quad(fill(
                            Bounds::from_corners(
                                point(text_x + start_width, y + line_height - px(2.0)),
                                point(text_x + end_width, y + line_height),
                            ),
                            token.foreground,
                        ));
                    }
                }

                if show_cursor && cursor_segment == Some(display_row) {
                    let local_cursor = cursor.saturating_sub(segment.start).min(segment.text.len());
                    let width = shape_line(
                        &display_prefix(&segment.text, local_cursor, masked),
                        font_size,
                        token.foreground,
                        window,
                    )
                    .width;
                    window.paint_quad(fill(
                        Bounds::from_corners(
                            point(text_x + width, y + px(2.0)),
                            point(text_x + width + px(2.0), y + line_height - px(2.0)),
                        ),
                        // `ThemeAdapter::caret` maps to the current theme's foreground.
                        token.foreground,
                    ));
                }
            }

            if is_empty && !placeholder.is_empty() {
                let shaped = shape_line(
                    &placeholder,
                    font_size,
                    token.foreground_placeholder,
                    window,
                );
                _ = shaped.paint(point(text_x, first_y), line_height, window, cx);
            }
        });

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_display_ascii_no_panic() {
        let r = masked_display("hello");
        assert_eq!(r, "•••••");
        assert!(!r.contains('h'));
    }

    #[test]
    fn masked_display_chinese_no_panic() {
        let r = masked_display("你好");
        assert_eq!(r, "••");
        assert!(!r.contains('你'));
    }

    #[test]
    fn masked_display_emoji_no_panic() {
        // 🎉 is a single Rust char (4 bytes), so a🎉b = 3 chars
        let r = masked_display("a🎉b");
        assert_eq!(r, "•••");
        assert!(!r.contains('a'));
        assert!(!r.contains('🎉'));
    }

    #[test]
    fn masked_display_combining_no_panic() {
        // "é" as e + combining acute accent (2 code points, 1 grapheme)
        let r = masked_display("café");
        assert_eq!(r, "••••");
        assert!(!r.contains('c'));
    }

    #[test]
    fn masked_display_empty_no_panic() {
        let r = masked_display("");
        assert_eq!(r, "");
    }

    #[test]
    fn display_prefix_masked_contains_no_original() {
        let text = "Password123";
        for offset in 0..=text.len() {
            let prefix = display_prefix(&text, offset, true);
            for ch in text.chars() {
                assert!(
                    !prefix.contains(ch),
                    "prefix at offset {} leaked char '{}'",
                    offset,
                    ch
                );
            }
            assert!(prefix.chars().all(|c| c == MASK_CHAR));
        }
    }

    #[test]
    fn display_prefix_unmasked_returns_original() {
        let text = "hello 世界 🎉";
        // Iterate only over valid char boundaries (including end).
        let mut offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        offsets.push(text.len());
        for offset in offsets {
            let prefix = display_prefix(&text, offset, false);
            assert_eq!(prefix, text[..offset]);
        }
    }

    #[test]
    fn clamp_offset_in_middle_of_multibyte() {
        let text = "你";
        // '你' is 3 bytes in UTF-8
        let prefix = display_prefix(&text, 1, true);
        assert_eq!(prefix, "");
        let prefix = display_prefix(&text, 2, true);
        assert_eq!(prefix, "");
        let prefix = display_prefix(&text, 3, true);
        assert_eq!(prefix, "•");
    }

    #[test]
    fn clamp_offset_clamps_to_char_boundary() {
        let text = "a你b";
        // 'a' = 1 byte, '你' = 3 bytes, 'b' = 1 byte, total = 5
        // offset 2 falls inside '你' (bytes 1..4)
        let prefix = display_prefix(&text, 2, true);
        assert_eq!(prefix, "•");
        assert!(prefix.chars().all(|c| c == MASK_CHAR));
    }
}
