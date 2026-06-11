//! 终端 Shell overlay：paint 内注册 IME + 预编辑渲染（对齐 Zed TerminalElement）。

use std::ops::Range;

use gpui::*;

use super::terminal_pane::{LINE_HEIGHT_RATIO, TERM_FONT};
use super::terminal_pane_view::TerminalPaneView;
use crate::model::TerminalKind;

const CURSOR_WIDTH: f32 = 1.5;

fn default_fg() -> Hsla {
    hsla(0.0, 0.0, 0.12, 1.0)
}

fn default_bg() -> Hsla {
    hsla(0.0, 0.0, 0.99, 1.0)
}

fn cursor_color() -> Hsla {
    hsla(0.0, 0.0, 0.12, 1.0)
}

pub struct TerminalShellElement {
    view: Entity<TerminalPaneView>,
    focus_handle: FocusHandle,
    font_size: f32,
}

impl TerminalShellElement {
    pub fn new(view: Entity<TerminalPaneView>, focus_handle: FocusHandle, font_size: f32) -> Self {
        Self {
            view,
            focus_handle,
            font_size,
        }
    }
}

impl IntoElement for TerminalShellElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalShellElement {
    type RequestLayoutState = ();
    type PrepaintState = (Bounds<Pixels>, usize, usize, Option<String>, Pixels);

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.position = Position::Absolute;
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.inset.right = px(0.).into();
        style.inset.bottom = px(0.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let pane = self.view.read(cx);
        let line_height = px((self.font_size * LINE_HEIGHT_RATIO).max(15.0));
        let (cursor_row, cursor_col) = pane.cursor_viewport();
        (
            bounds,
            cursor_row,
            cursor_col,
            pane.marked_text().map(str::to_string),
            line_height,
        )
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let grid_bounds = prepaint.0;
        let view = self.view.clone();
        cx.defer(move |cx| {
            view.update(cx, |pane, cx| {
                pane.set_grid_layout(grid_bounds, cx);
            });
        });

        let pane = self.view.read(cx);
        if !pane.input_enabled_for_ime(cx) || !matches!(pane.terminal_kind(), TerminalKind::Shell) {
            return;
        }

        let char_width = measure_char_width(window, px(self.font_size));
        let cursor_h = px(self.font_size);
        let cursor_row = prepaint.1;
        let cursor_col = prepaint.2;
        let cursor_x = char_width * cursor_col as f32;
        let cursor_y = prepaint.4 * cursor_row as f32 + (prepaint.4 - cursor_h) * 0.5;
        let cursor_bounds = Some(Bounds::new(
            prepaint.0.origin + point(cursor_x, cursor_y),
            size(px(CURSOR_WIDTH), cursor_h),
        ));

        window.handle_input(
            &self.focus_handle,
            TerminalInputHandler::new(self.view.clone(), cursor_bounds),
            cx,
        );

        let focused = self.focus_handle.is_focused(window);
        let composing = prepaint.3.as_ref().is_some_and(|s| !s.is_empty());
        if focused && !composing {
            if let Some(ime_bounds) = cursor_bounds {
                window.paint_quad(fill(ime_bounds, cursor_color()));
            }
        }

        let Some(text_to_mark) = prepaint.3.as_ref().filter(|s| !s.is_empty()) else {
            return;
        };
        let Some(ime_bounds) = cursor_bounds else {
            return;
        };

        let ime_position = ime_bounds.origin;
        let font_size = px(self.font_size);
        let underline = UnderlineStyle {
            color: Some(default_fg()),
            thickness: px(1.0),
            wavy: false,
        };
        let font = Font {
            family: TERM_FONT.into(),
            features: Default::default(),
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
            fallbacks: None,
        };
        let shaped_line = window.text_system().shape_line(
            text_to_mark.clone().into(),
            font_size,
            &[TextRun {
                len: text_to_mark.len(),
                font: font.clone(),
                color: default_fg(),
                background_color: None,
                underline: Some(underline),
                strikethrough: None,
            }],
            None,
        );

        let ime_background_bounds = Bounds::new(ime_position, size(shaped_line.width, prepaint.4));
        window.paint_quad(fill(ime_background_bounds, default_bg()));
        let _ = shaped_line.paint(ime_position, prepaint.4, window, cx);
    }
}

fn measure_char_width(window: &Window, font_size: Pixels) -> Pixels {
    let font = Font {
        family: TERM_FONT.into(),
        features: Default::default(),
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
        fallbacks: None,
    };
    window
        .text_system()
        .shape_line(
            "M".into(),
            font_size,
            &[TextRun {
                len: 1,
                font,
                color: default_fg(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
        .width
}

struct TerminalInputHandler {
    view: Entity<TerminalPaneView>,
    cursor_bounds: Option<Bounds<Pixels>>,
}

impl TerminalInputHandler {
    fn new(view: Entity<TerminalPaneView>, cursor_bounds: Option<Bounds<Pixels>>) -> Self {
        Self {
            view,
            cursor_bounds,
        }
    }
}

impl InputHandler for TerminalInputHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&mut self, _window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.view.read(cx).marked_text_range()
    }

    fn text_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _replacement_range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut App,
    ) {
        let text = text.to_string();
        self.view.update(cx, |view, cx| {
            view.clear_marked_text(cx);
            view.commit_text(&text, cx);
        });
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.view.update(cx, |view, cx| {
            view.set_marked_text(new_text.to_string(), cx);
        });
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut App) {
        self.view.update(cx, |view, cx| {
            view.clear_marked_text(cx);
        });
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let mut bounds = self.cursor_bounds?;
        let (char_width, _) = self.view.read(cx).char_metrics_for_ime();
        bounds.origin.x += char_width * range_utf16.start as f32;
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }

    fn apple_press_and_hold_enabled(&mut self) -> bool {
        false
    }
}
