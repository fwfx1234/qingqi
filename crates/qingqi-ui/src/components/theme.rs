//! Theme accessibility layer — provides `cx.theme()` and `Theme::global(cx)` APIs.

use gpui::App;

use crate::token::{Token, tokens};

pub use crate::token::Token as Theme;

impl Theme {
    /// Return a clone of the global theme (compat API for old plugin code).
    #[inline(always)]
    pub fn global(cx: &App) -> Self {
        tokens(cx)
    }

    #[inline(always)]
    pub fn global_mut(cx: &mut App) -> Self {
        tokens(cx)
    }
}

/// `ActiveTheme` trait gives `.theme()` on `App` — returns `Token` by value.
pub trait ActiveTheme {
    fn theme(&self) -> Token;
}

impl ActiveTheme for App {
    #[inline(always)]
    fn theme(&self) -> Token {
        tokens(self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn is_dark(&self) -> bool {
        matches!(self, ThemeMode::Dark)
    }
}
