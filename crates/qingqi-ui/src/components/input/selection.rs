//! Text selection and word-boundary logic.

use std::{char, ops::Range};

use gpui::{App, Context, Window};
use ropey::Rope;
use sum_tree::Bias;

use super::InputState;
use super::RopeExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CharType {
    Word,
    Whitespace,
    Newline,
    Other,
}

impl From<char> for CharType {
    fn from(c: char) -> Self {
        match c {
            '_' => CharType::Word,
            c if c.is_ascii_alphanumeric() => CharType::Word,
            c if c == '\n' || c == '\r' => CharType::Newline,
            c if c.is_whitespace() => CharType::Whitespace,
            _ => CharType::Other,
        }
    }
}

impl CharType {
    pub(crate) fn is_connectable(self, c: char) -> bool {
        let other = CharType::from(c);
        match (self, other) {
            (CharType::Word, CharType::Word) => true,
            (CharType::Whitespace, CharType::Whitespace) => true,
            _ => false,
        }
    }
}

impl InputState {
    /// Select the word at the given offset on double-click.
    pub(super) fn select_word(&mut self, offset: usize, _: &mut Window, cx: &mut Context<Self>) {
        let range = match TextSelector::word_range(&self.text, offset) {
            Some(range) => range,
            None => return,
        };

        self.selected_range = (range.start..range.end).into();
        self.selected_word_range = Some(self.selected_range);
        cx.notify()
    }
}

pub(crate) struct TextSelector;

impl TextSelector {
    pub fn word_range(text: &Rope, offset: usize) -> Option<Range<usize>> {
        let offset = text.clip_offset(offset, Bias::Left);
        let ch = text.char_at(offset)?;
        let char_type = CharType::from(ch);

        let mut start = offset;
        let mut end = offset + ch.len_utf8();

        for ch in text.chars_at(start).reversed().take(128) {
            if char_type.is_connectable(ch) {
                start -= ch.len_utf8();
            } else {
                break;
            }
        }

        for ch in text.chars_at(end).take(128) {
            if char_type.is_connectable(ch) {
                end += ch.len_utf8();
            } else {
                break;
            }
        }

        Some(start..end)
    }
}

// CharType classification helper
pub(crate) fn char_type(c: char) -> CharType {
    CharType::from(c)
}

/// Returns true if the character is a word boundary character.
pub(crate) fn is_word_char(c: char) -> bool {
    matches!(CharType::from(c), CharType::Word)
}

/// Check if char is whitespace (but not newline)
pub(crate) fn is_space_char(c: char) -> bool {
    matches!(CharType::from(c), CharType::Whitespace)
}
