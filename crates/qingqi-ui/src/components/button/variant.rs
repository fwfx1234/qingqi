use gpui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Text,
    Danger,
    Warning,
    Success,
    Info,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
    XLarge,
}

pub struct ButtonCustomVariant {
    pub color: Hsla,
    pub foreground: Hsla,
    pub border: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub shadow: bool,
}

impl ButtonCustomVariant {
    pub fn new(cx: &App) -> Self {
        let token = crate::token::tokens(cx);
        Self {
            color: token.surface,
            foreground: token.foreground,
            border: token.border,
            hover: token.surface_hover,
            active: token.surface_active,
            shadow: false,
        }
    }
    pub fn color(mut self, c: Hsla) -> Self { self.color = c; self }
    pub fn foreground(mut self, c: Hsla) -> Self { self.foreground = c; self }
    pub fn border(mut self, c: Hsla) -> Self { self.border = c; self }
    pub fn hover(mut self, c: Hsla) -> Self { self.hover = c; self }
    pub fn active(mut self, c: Hsla) -> Self { self.active = c; self }
    pub fn shadow(mut self, s: bool) -> Self { self.shadow = s; self }
}

pub fn button_colors(variant: ButtonVariant, cx: &App) -> (Hsla, Hsla, Hsla) {
    let token = crate::token::tokens(cx);
    match variant {
        ButtonVariant::Primary => (token.accent, gpui::white(), token.accent),
        ButtonVariant::Secondary => (token.surface, token.foreground, token.border),
        ButtonVariant::Ghost => {
            let transparent = gpui::hsla(0.0, 0.0, 0.0, 0.0);
            (transparent, token.foreground, transparent)
        }
        ButtonVariant::Text => {
            let transparent = gpui::hsla(0.0, 0.0, 0.0, 0.0);
            (transparent, token.foreground, transparent)
        }
        ButtonVariant::Danger => (token.danger, gpui::white(), token.danger),
        ButtonVariant::Warning => (token.warning, gpui::white(), token.warning),
        ButtonVariant::Success => (token.success, gpui::white(), token.success),
        ButtonVariant::Info => (token.info, gpui::white(), token.info),
        ButtonVariant::Link => {
            let transparent = gpui::hsla(0.0, 0.0, 0.0, 0.0);
            (transparent, token.accent, transparent)
        }
    }
}

pub fn button_height(size: ButtonSize) -> Pixels {
    match size {
        ButtonSize::XSmall => px(24.0),
        ButtonSize::Small => px(30.0),
        ButtonSize::Medium => px(38.0),
        ButtonSize::Large => px(44.0),
        ButtonSize::XLarge => px(52.0),
    }
}

pub fn button_padding(size: ButtonSize) -> Edges<Pixels> {
    let val = match size {
        ButtonSize::XSmall => px(8.0),
        ButtonSize::Small => px(10.0),
        ButtonSize::Medium => px(12.0),
        ButtonSize::Large => px(16.0),
        ButtonSize::XLarge => px(20.0),
    };
    Edges { left: val, right: val, top: px(0.0), bottom: px(0.0) }
}

pub fn button_font_size(size: ButtonSize) -> Pixels {
    match size {
        ButtonSize::XSmall => px(11.0),
        ButtonSize::Small => px(12.0),
        ButtonSize::Medium => px(13.0),
        ButtonSize::Large => px(14.0),
        ButtonSize::XLarge => px(16.0),
    }
}
