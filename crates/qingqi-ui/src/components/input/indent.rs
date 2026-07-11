//! Tab size configuration and indent/outdent operations.

use gpui::{Context, SharedString, Window};
use ropey::RopeSlice;

use super::{InputState, mode::InputMode};

/// Tab size configuration for the input field.
#[derive(Debug, Copy, Clone)]
pub struct TabSize {
    pub tab_size: usize,
    pub hard_tabs: bool,
}

impl Default for TabSize {
    fn default() -> Self {
        Self {
            tab_size: 2,
            hard_tabs: false,
        }
    }
}

impl TabSize {
    pub(super) fn to_string(&self) -> SharedString {
        if self.hard_tabs {
            "\t".into()
        } else {
            " ".repeat(self.tab_size).into()
        }
    }

    pub fn indent_count(&self, line: &RopeSlice) -> usize {
        let mut count = 0;
        for ch in line.chars() {
            match ch {
                '\t' => count += self.tab_size,
                ' ' => count += 1,
                _ => break,
            }
        }
        count
    }
}

impl InputState {
    pub(super) fn indent_inline(
        &mut self,
        _: &super::IndentInline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.indent(&super::Indent {}, window, cx);
    }

    pub(super) fn outdent_inline(
        &mut self,
        _: &super::OutdentInline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.outdent(&super::Outdent {}, window, cx);
    }

    pub fn set_tab_size(&mut self, tab_size: TabSize) {
        match &mut self.mode {
            InputMode::PlainText { tab, .. } => *tab = tab_size,
            InputMode::CodeEditor { tab, .. } => *tab = tab_size,
            _ => {}
        }
    }

    pub(super) fn indent(
        &mut self,
        _: &super::Indent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        if !self.mode.is_indentable() {
            cx.propagate();
            return;
        }

        let tab_indent = self.mode.tab_size().to_string();
        let selected_range = self.selected_range;
        let mut added_len = 0;
        let is_selected = !selected_range.is_empty();

        if is_selected {
            let start_offset = self.start_of_line_of_selection(window, cx);
            let mut offset = start_offset;
            let selected_text = self
                .text_for_utf16_range(
                    self.range_to_utf16(&(offset..selected_range.end)),
                    &mut None,
                    window,
                    cx,
                )
                .unwrap_or_default();

            for line in selected_text.split('\n') {
                self.replace_text_in_range_silent(
                    Some(self.range_to_utf16(&(offset..offset))),
                    &tab_indent,
                    window,
                    cx,
                );
                added_len += tab_indent.len();
                offset += line.len() + tab_indent.len() + 1;
            }

            if is_selected {
                self.selected_range = (start_offset..selected_range.end + added_len).into();
            } else {
                self.selected_range =
                    (selected_range.start + added_len..selected_range.end + added_len).into();
            }
        } else {
            let offset = self.selected_range.start;
            self.replace_text_in_range_silent(
                Some(self.range_to_utf16(&(offset..offset))),
                &tab_indent,
                window,
                cx,
            );
            added_len = tab_indent.len();
            self.selected_range =
                (selected_range.start + added_len..selected_range.end + added_len).into();
        }
    }

    pub(super) fn outdent(
        &mut self,
        _: &super::Outdent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        if !self.mode.is_indentable() {
            cx.propagate();
            return;
        }

        let tab_indent = self.mode.tab_size().to_string();
        let selected_range = self.selected_range;
        let mut removed_len = 0;
        let is_selected = !selected_range.is_empty();

        if is_selected {
            let start_offset = self.start_of_line_of_selection(window, cx);
            let mut offset = start_offset;
            let selected_text = self
                .text_for_utf16_range(
                    self.range_to_utf16(&(offset..selected_range.end)),
                    &mut None,
                    window,
                    cx,
                )
                .unwrap_or_default();

            for line in selected_text.split('\n') {
                if line.starts_with(tab_indent.as_ref()) {
                    self.replace_text_in_range_silent(
                        Some(self.range_to_utf16(&(offset..offset + tab_indent.len()))),
                        "",
                        window,
                        cx,
                    );
                    removed_len += tab_indent.len();
                    offset += line.len().saturating_sub(tab_indent.len()) + 1;
                } else {
                    offset += line.len() + 1;
                }
            }

            if is_selected {
                self.selected_range =
                    (start_offset..selected_range.end.saturating_sub(removed_len)).into();
            } else {
                self.selected_range = (selected_range.start.saturating_sub(removed_len)
                    ..selected_range.end.saturating_sub(removed_len))
                    .into();
            }
        } else {
            let start_offset = self.selected_range.start;
            let offset = self.start_of_line_of_selection(window, cx);
            let offset = self.offset_from_utf16(self.offset_to_utf16(offset));

            if self
                .text
                .slice(offset..self.text.len())
                .to_string()
                .starts_with(tab_indent.as_ref())
            {
                self.replace_text_in_range_silent(
                    Some(self.range_to_utf16(&(offset..offset + tab_indent.len()))),
                    "",
                    window,
                    cx,
                );
                removed_len = tab_indent.len();
                let new_offset = start_offset.saturating_sub(removed_len);
                self.selected_range = (new_offset..new_offset).into();
            }
        }
    }
}
