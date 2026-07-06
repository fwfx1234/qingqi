//! Change tracking for undo/redo history.

use super::cursor::Selection;

/// A single text change record for history (undo/redo).
#[derive(Debug, PartialEq, Clone)]
pub struct Change {
    pub(crate) old_range: Selection,
    pub(crate) old_text: String,
    pub(crate) new_range: Selection,
    pub(crate) new_text: String,
    version: usize,
}

impl Change {
    pub fn new(
        old_range: impl Into<Selection>,
        old_text: &str,
        new_range: impl Into<Selection>,
        new_text: &str,
    ) -> Self {
        Self {
            old_range: old_range.into(),
            old_text: old_text.to_string(),
            new_range: new_range.into(),
            new_text: new_text.to_string(),
            version: 0,
        }
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn set_version(&mut self, version: usize) {
        self.version = version
    }
}

/// A group of changes that should be undone/redone together.
#[derive(Debug, Default)]
pub(crate) struct ChangeGroup {
    pub(crate) changes: Vec<Change>,
}

impl ChangeGroup {
    pub fn new() -> Self {
        Self { changes: Vec::new() }
    }

    pub fn push(&mut self, change: Change) {
        self.changes.push(change);
    }
}
