//! Theme JSON loader — parses qingqi-ui style theme config files into `Token`.
//!
//! Maintains compatibility with the existing theme JSON format used by qingqi-app.

use gpui::{App, Hsla};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ThemeFile {
    pub name: Option<String>,
    pub themes: Vec<ThemeEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ThemeEntry {
    pub name: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub colors: HashMap<String, String>,
}

fn parse_hex(hex: &str) -> Option<Hsla> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&h[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&h[4..6], 16).ok()? as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return Some(Hsla {
            h: 0.0,
            s: 0.0,
            l,
            a: 1.0,
        });
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + (if g < b { 6.0 } else { 0.0 })
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    Some(Hsla {
        h: h / 6.0,
        s,
        l,
        a: 1.0,
    })
}

/// Apply color overrides (dot-separated keys) onto a Token.
pub fn apply_token_overrides(token: &mut crate::token::Token, colors: &HashMap<String, String>) {
    macro_rules! set_c {
        ($key:expr, $field:ident) => {
            if let Some(color_str) = colors.get($key) {
                if let Some(c) = parse_hex(color_str) { token.$field = c; }
            }
        };
        ($key:expr, $($field:ident).+) => {
            if let Some(color_str) = colors.get($key) {
                if let Some(c) = parse_hex(color_str) { token.$($field).+ = c; }
            }
        };
    }

    set_c!("foreground", foreground);
    set_c!("background", background);
    set_c!("border", border);
    set_c!("input.border", border);

    set_c!("muted.background", muted);
    set_c!("muted.foreground", muted_foreground);

    set_c!("primary.background", primary);
    set_c!("primary.active.background", surface_active);
    set_c!("accent.background", accent);
    set_c!("accent.foreground", foreground);
    set_c!("accent", accent);
    set_c!("panel.background", panel);

    set_c!("danger.background", danger);
    set_c!("warning.background", warning);
    set_c!("success.background", success);
    set_c!("info.background", info);

    set_c!("foreground.disabled", foreground_disabled);
    set_c!("foreground.placeholder", foreground_placeholder);

    set_c!("list.active.background", list_active);
    set_c!("list.active.border", list_active_border);
    set_c!("list.even.background", list_even);
    set_c!("list.head.background", list_head);
    set_c!("list.hover.background", list_hover);

    set_c!("popover.background", popover);
    set_c!("popover.foreground", foreground);

    set_c!("sidebar.background", sidebar);
}

pub fn token_from_entry(entry: &ThemeEntry) -> crate::token::Token {
    let is_dark = entry.mode == "dark";
    let mut token = crate::token::Token::from_theme_name("default", is_dark);
    apply_token_overrides(&mut token, &entry.colors);
    token
}

pub fn load_theme_file(json: &str) -> Vec<(String, crate::token::Token)> {
    let parsed: ThemeFile = match serde_json::from_str(json) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let simple_name = parsed.name.clone().unwrap_or_default();
    parsed
        .themes
        .iter()
        .filter_map(|entry| {
            let token = token_from_entry(entry);
            let mode_cap = entry.mode.capitalize_first();
            let display = format!("{} {}", simple_name, mode_cap);
            Some((display, token))
        })
        .collect()
}

pub fn resolve_variant<'a>(
    themes: &'a [(String, crate::token::Token)],
    name: &str,
    is_dark: bool,
) -> Option<&'a crate::token::Token> {
    let suffix = if is_dark { "Dark" } else { "Light" };
    let full = format!("{} {}", name, suffix);
    themes
        .iter()
        .find(|(n, _)| n.as_str() == full.as_str())
        .map(|(_, t)| t)
}

pub fn list_base_names(themes: &[(String, crate::token::Token)]) -> Vec<String> {
    let mut names: Vec<String> = themes
        .iter()
        .map(|(n, _)| {
            n.strip_suffix(" Light")
                .or_else(|| n.strip_suffix(" Dark"))
                .unwrap_or(n)
                .to_string()
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

pub fn apply_tokens(cx: &mut App, dark: bool) {
    crate::token::install_tokens(cx, dark);
}

pub fn apply_custom_token(token: crate::token::Token, cx: &mut App) {
    cx.set_global(crate::token::TokenState { token });
}

trait CapitalizeExt {
    fn capitalize_first(&self) -> String;
}
impl CapitalizeExt for str {
    fn capitalize_first(&self) -> String {
        let mut c = self.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }
}
