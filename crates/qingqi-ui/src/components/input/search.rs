//! SearchMatcher + SearchPanel — find/replace functionality for input fields.

use std::ops::Range;
use std::rc::Rc;

use aho_corasick::AhoCorasick;
use gpui::{App, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Pixels, Render, Styled, Subscription, Window, actions, div, prelude::FluentBuilder as _};
use ropey::Rope;

use crate::token::tokens;

use super::{Input, InputState};

const CONTEXT: &str = "SearchPanel";

#[derive(Debug, Clone)]
pub struct SearchMatcher {
    text: Rope,
    pub query: Option<AhoCorasick>,
    pub matched_ranges: Rc<Vec<Range<usize>>>,
    pub current_match_ix: usize,
    replacing: bool,
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

    fn label(&self) -> String {
        if self.len() == 0 {
            return "0/0".to_string();
        }
        format!("{}/{}", self.current_match_ix + 1, self.len())
    }

    fn update_cursor_by_offset(&mut self, offset: usize) {
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

pub(super) struct SearchPanel {
    pub open: bool,
    pub case_insensitive: bool,
    pub replace_mode: bool,
    pub matcher: SearchMatcher,
}

impl Default for SearchPanel {
    fn default() -> Self {
        Self {
            open: false,
            case_insensitive: true,
            replace_mode: false,
            matcher: SearchMatcher::new(),
        }
    }
}

impl SearchPanel {
    pub fn next(&mut self) {
        self.matcher.next();
    }

    pub fn prev(&mut self) {
        self.matcher.next_back();
    }
}
