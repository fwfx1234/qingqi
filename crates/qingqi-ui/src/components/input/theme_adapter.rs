//! Theme adapter — maps qingqi-ui's `ActiveTheme` color API to qingqi's `Token` system.

use gpui::{App, Hsla, Pixels, px, black, white};

use crate::token::tokens;

pub struct ThemeAdapter;

impl ThemeAdapter {
    pub fn background(cx: &App) -> Hsla { tokens(cx).background }
    pub fn foreground(cx: &App) -> Hsla { tokens(cx).foreground }
    pub fn muted(cx: &App) -> Hsla { tokens(cx).muted }
    pub fn muted_foreground(cx: &App) -> Hsla { tokens(cx).muted_foreground }
    pub fn secondary_foreground(cx: &App) -> Hsla { tokens(cx).muted_foreground }
    pub fn border(cx: &App) -> Hsla { tokens(cx).border }
    pub fn input(cx: &App) -> Hsla { tokens(cx).border }
    pub fn ring(cx: &App) -> Hsla { tokens(cx).border_focus }
    pub fn accent(cx: &App) -> Hsla { tokens(cx).accent }
    pub fn accent_foreground(cx: &App) -> Hsla { white() }
    pub fn primary(cx: &App) -> Hsla { tokens(cx).primary }
    pub fn blue(cx: &App) -> Hsla { tokens(cx).blue }
    pub fn caret(cx: &App) -> Hsla { tokens(cx).foreground }
    pub fn selection(cx: &App) -> Hsla { tokens(cx).accent.opacity(0.3) }
    pub fn editor_background(cx: &App) -> Hsla { tokens(cx).surface }
    pub fn popover(cx: &App) -> Hsla { tokens(cx).popover }
    pub fn transparent(_cx: &App) -> Hsla { Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 } }
    pub fn radius(_cx: &App) -> Pixels { px(6.0) }
    pub fn radius_half(_cx: &App) -> Pixels { px(3.0) }
    pub fn shadow(_cx: &App) -> bool { true }
    pub fn font_family(_cx: &App) -> String { ".SystemUIFont".to_string() }
}
