//! SearchMatcher + SearchPanel — find/replace functionality for input fields.

use std::ops::Range;
use std::rc::Rc;

use aho_corasick::AhoCorasick;
use gpui::{App, Entity, KeyBinding};
use ropey::Rope;

use super::{InputState, SelectUp};

pub(super) const CONTEXT: &str = "QingqiSearchPanel";

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("shift-enter", SelectUp, Some(CONTEXT))]);
}

#[derive(Debug, Clone)]
pub struct SearchMatcher {
    text: Rope,
    pub query: Option<AhoCorasick>,
    pub matched_ranges: Rc<Vec<Range<usize>>>,
    pub current_match_ix: usize,
    pub(crate) replacing: bool,
}

impl SearchMatcher {
    pub fn new() -> Self {
        Self {
            text: "".into(),
            query: None,
            matched_ranges: Rc::new(Vec::new()),
            current_match_ix: 0,
            replacing: false,
        }
    }

    pub fn update(&mut self, text: &Rope) {
        if self.text.eq(text) {
            return;
        }
        self.text = text.clone();
        self.update_matches();
    }

    fn update_matches(&mut self) {
        let mut new_ranges = Vec::new();
        if let Some(query) = &self.query {
            let text = self.text.to_string();
            for query_match in query.stream_find_iter(text.as_bytes()) {
                let query_match = query_match.expect("query match");
                new_ranges.push(query_match.range());
            }
        }
        self.matched_ranges = Rc::new(new_ranges);
        if !self.replacing {
            self.current_match_ix = 0;
            self.replacing = false;
        }
    }

    pub fn update_query(&mut self, query: &str, case_insensitive: bool) {
        if !query.is_empty() {
            self.query = Some(
                AhoCorasick::builder()
                    .ascii_case_insensitive(case_insensitive)
                    .build(&[query.to_string()])
                    .expect("failed to build AhoCorasick query"),
            );
        } else {
            self.query = None;
        }
        self.update_matches();
    }

    pub fn len(&self) -> usize {
        self.matched_ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matched_ranges.is_empty()
    }

    pub(crate) fn label(&self) -> String {
        if self.len() == 0 {
            return "0/0".to_string();
        }
        format!("{}/{}", self.current_match_ix + 1, self.len())
    }

    pub(crate) fn update_cursor_by_offset(&mut self, offset: usize) {
        for (ix, range) in self.matched_ranges.iter().enumerate() {
            self.current_match_ix = ix;
            if range.contains(&offset) || range.end >= offset {
                return;
            }
        }
    }
}

impl Iterator for SearchMatcher {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.matched_ranges.is_empty() {
            return None;
        }
        if self.current_match_ix < self.matched_ranges.len().saturating_sub(1) {
            self.current_match_ix += 1;
        } else {
            self.current_match_ix = 0;
        }
        self.matched_ranges.get(self.current_match_ix).cloned()
    }
}

impl DoubleEndedIterator for SearchMatcher {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.matched_ranges.is_empty() {
            return None;
        }
        if self.current_match_ix == 0 {
            self.current_match_ix = self.matched_ranges.len();
        }
        self.current_match_ix -= 1;
        Some(self.matched_ranges[self.current_match_ix].clone())
    }
}

pub(crate) struct SearchPanel {
    pub open: bool,
    pub case_insensitive: bool,
    pub replace_mode: bool,
    pub query_input: Entity<InputState>,
    pub replace_input: Entity<InputState>,
}

impl SearchPanel {
    pub fn new(query_input: Entity<InputState>, replace_input: Entity<InputState>) -> Self {
        Self {
            open: true,
            case_insensitive: true,
            replace_mode: false,
            query_input,
            replace_input,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matcher_handles_unicode_ranges_and_case() {
        let mut matcher = SearchMatcher::new();
        matcher.update(&Rope::from("Hello 世界 hello"));
        matcher.update_query("hello", true);
        assert_eq!(&*matcher.matched_ranges, &[0..5, 13..18]);
        matcher.update_query("世界", false);
        assert_eq!(&*matcher.matched_ranges, &[6..12]);
    }
}
