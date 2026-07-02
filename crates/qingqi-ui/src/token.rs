use gpui::{App, Hsla, Global};

#[derive(Clone)]
pub struct Token {
    pub background: Hsla,
    pub surface: Hsla,
    pub surface_hover: Hsla,
    pub surface_active: Hsla,
    pub muted: Hsla,
    pub foreground: Hsla,
    pub foreground_muted: Hsla,
    pub foreground_disabled: Hsla,
    pub foreground_placeholder: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub border_focus: Hsla,
    pub accent: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
    pub info: Hsla,
    pub overlay: Hsla,
}

impl Token {
    pub fn from_theme_name(_name: &str, dark: bool) -> Self {
        if dark {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn dark() -> Self {
        use gpui::hsla;
        Self {
            background: hsla(0.0, 0.0, 0.12, 1.0),
            surface: hsla(0.0, 0.0, 0.16, 1.0),
            surface_hover: hsla(0.0, 0.0, 0.20, 1.0),
            surface_active: hsla(0.0, 0.0, 0.24, 1.0),
            muted: hsla(0.0, 0.0, 0.14, 1.0),
            foreground: hsla(0.0, 0.0, 0.95, 1.0),
            foreground_muted: hsla(0.0, 0.0, 0.60, 1.0),
            foreground_disabled: hsla(0.0, 0.0, 0.40, 1.0),
            foreground_placeholder: hsla(0.0, 0.0, 0.45, 1.0),
            border: hsla(0.0, 0.0, 0.25, 1.0),
            border_strong: hsla(0.0, 0.0, 0.35, 1.0),
            border_focus: hsla(210.0 / 360.0, 0.8, 0.6, 1.0),
            accent: hsla(210.0 / 360.0, 0.8, 0.6, 1.0),
            success: hsla(140.0 / 360.0, 0.7, 0.5, 1.0),
            warning: hsla(35.0 / 360.0, 0.9, 0.55, 1.0),
            danger: hsla(0.0, 0.8, 0.6, 1.0),
            info: hsla(210.0 / 360.0, 0.8, 0.6, 1.0),
            overlay: hsla(0.0, 0.0, 0.0, 0.5),
        }
    }

    fn light() -> Self {
        use gpui::hsla;
        Self {
            background: hsla(0.0, 0.0, 1.0, 1.0),
            surface: hsla(0.0, 0.0, 0.98, 1.0),
            surface_hover: hsla(0.0, 0.0, 0.95, 1.0),
            surface_active: hsla(0.0, 0.0, 0.92, 1.0),
            muted: hsla(0.0, 0.0, 0.95, 1.0),
            foreground: hsla(0.0, 0.0, 0.10, 1.0),
            foreground_muted: hsla(0.0, 0.0, 0.45, 1.0),
            foreground_disabled: hsla(0.0, 0.0, 0.60, 1.0),
            foreground_placeholder: hsla(0.0, 0.0, 0.55, 1.0),
            border: hsla(0.0, 0.0, 0.85, 1.0),
            border_strong: hsla(0.0, 0.0, 0.75, 1.0),
            border_focus: hsla(210.0 / 360.0, 0.8, 0.5, 1.0),
            accent: hsla(210.0 / 360.0, 0.8, 0.5, 1.0),
            success: hsla(140.0 / 360.0, 0.6, 0.4, 1.0),
            warning: hsla(35.0 / 360.0, 0.9, 0.5, 1.0),
            danger: hsla(0.0, 0.7, 0.55, 1.0),
            info: hsla(210.0 / 360.0, 0.8, 0.5, 1.0),
            overlay: hsla(0.0, 0.0, 0.0, 0.3),
        }
    }
}

#[derive(Clone)]
pub struct TokenState {
    pub token: Token,
}

impl Global for TokenState {}

pub fn tokens(cx: &App) -> &Token {
    &cx.global::<TokenState>().token
}

pub fn install_tokens(cx: &mut App, dark: bool) {
    let token = Token::from_theme_name("default", dark);
    cx.set_global(TokenState { token });
}
