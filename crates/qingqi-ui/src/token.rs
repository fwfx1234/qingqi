use gpui::{App, Global, Hsla};

#[derive(Clone)]
pub struct Token {
    pub background: Hsla,
    pub surface: Hsla,
    pub surface_hover: Hsla,
    pub surface_active: Hsla,
    pub muted: Hsla,
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
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
    pub list: Hsla,
    pub list_hover: Hsla,
    pub list_active: Hsla,
    pub list_active_border: Hsla,
    pub list_even: Hsla,
    pub list_head: Hsla,
    pub popover: Hsla,
    pub sidebar: Hsla,
    pub primary: Hsla,
    pub blue: Hsla,
    pub panel: Hsla,

    pub is_dark: bool,

    // Additional color/style accessors for plugin compat
    pub mode: crate::components::theme::ThemeMode,
    pub primary_active: Hsla,
    pub primary_hover: Hsla,
    pub secondary_active: Hsla,
    pub sidebar_accent: Hsla,
    pub sidebar_accent_foreground: Hsla,
    pub transparent: Hsla,
}

impl Token {
    pub fn is_dark(&self) -> bool {
        self.is_dark
    }
    pub fn from_theme_name(_name: &str, dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    fn dark() -> Self {
        Self::build(true)
    }
    fn light() -> Self {
        Self::build(false)
    }

    fn build(is_dark: bool) -> Self {
        use gpui::hsla;
        macro_rules! h {
            ($h:expr, $s:expr, $l:expr) => {
                hsla($h, $s, $l, 1.0)
            };
        }
        macro_rules! ha {
            ($h:expr, $s:expr, $l:expr, $a:expr) => {
                hsla($h, $s, $l, $a)
            };
        }
        if is_dark {
            Self {
                background: h!(0.0, 0.0, 0.12),
                surface: h!(0.0, 0.0, 0.16),
                surface_hover: h!(0.0, 0.0, 0.20),
                surface_active: h!(0.0, 0.0, 0.24),
                muted: h!(0.0, 0.0, 0.14),
                foreground: h!(0.0, 0.0, 0.95),
                muted_foreground: h!(0.0, 0.0, 0.60),
                foreground_disabled: h!(0.0, 0.0, 0.40),
                foreground_placeholder: h!(0.0, 0.0, 0.45),
                border: h!(0.0, 0.0, 0.25),
                border_strong: h!(0.0, 0.0, 0.35),
                border_focus: h!(210.0 / 360.0, 0.8, 0.6),
                accent: h!(210.0 / 360.0, 0.8, 0.6),
                success: h!(140.0 / 360.0, 0.7, 0.5),
                warning: h!(35.0 / 360.0, 0.9, 0.55),
                danger: h!(0.0, 0.8, 0.6),
                info: h!(210.0 / 360.0, 0.8, 0.6),
                overlay: ha!(0.0, 0.0, 0.0, 0.5),
                list: h!(0.0, 0.0, 0.10),
                list_hover: h!(0.0, 0.0, 0.14),
                list_active: ha!(210.0 / 360.0, 0.5, 0.5, 0.2),
                list_active_border: h!(210.0 / 360.0, 0.8, 0.6),
                list_even: h!(0.0, 0.0, 0.10),
                list_head: h!(0.0, 0.0, 0.10),
                popover: h!(0.0, 0.0, 0.18),
                sidebar: h!(0.0, 0.0, 0.10),
                primary: h!(210.0 / 360.0, 0.8, 0.6),
                blue: h!(210.0 / 360.0, 0.8, 0.6),
                panel: h!(0.0, 0.0, 0.14),
                mode: crate::components::theme::ThemeMode::Dark,
                primary_active: darken_hsla(h!(210.0 / 360.0, 0.8, 0.6), 0.1),
                primary_hover: lighten_hsla(h!(0.0, 0.0, 0.20), 0.05),
                secondary_active: h!(0.0, 0.0, 0.24),
                sidebar_accent: h!(210.0 / 360.0, 0.8, 0.6),
                sidebar_accent_foreground: gpui::white(),
                transparent: Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: 0.0,
                },
                is_dark: true,
            }
        } else {
            Self {
                background: h!(0.0, 0.0, 1.0),
                surface: h!(0.0, 0.0, 0.98),
                surface_hover: h!(0.0, 0.0, 0.95),
                surface_active: h!(0.0, 0.0, 0.92),
                muted: h!(0.0, 0.0, 0.95),
                foreground: h!(0.0, 0.0, 0.10),
                muted_foreground: h!(0.0, 0.0, 0.45),
                foreground_disabled: h!(0.0, 0.0, 0.60),
                foreground_placeholder: h!(0.0, 0.0, 0.55),
                border: h!(0.0, 0.0, 0.85),
                border_strong: h!(0.0, 0.0, 0.75),
                border_focus: h!(210.0 / 360.0, 0.8, 0.5),
                accent: h!(210.0 / 360.0, 0.8, 0.5),
                success: h!(140.0 / 360.0, 0.6, 0.4),
                warning: h!(35.0 / 360.0, 0.9, 0.5),
                danger: h!(0.0, 0.7, 0.55),
                info: h!(210.0 / 360.0, 0.8, 0.5),
                overlay: ha!(0.0, 0.0, 0.0, 0.3),
                list: h!(0.0, 0.0, 0.97),
                list_hover: h!(0.0, 0.0, 0.94),
                list_active: ha!(210.0 / 360.0, 0.5, 0.3, 0.2),
                list_active_border: h!(210.0 / 360.0, 0.8, 0.5),
                list_even: h!(0.0, 0.0, 0.98),
                list_head: h!(0.0, 0.0, 0.98),
                popover: h!(0.0, 0.0, 1.0),
                sidebar: h!(0.0, 0.0, 0.97),
                primary: h!(210.0 / 360.0, 0.8, 0.5),
                blue: h!(210.0 / 360.0, 0.8, 0.5),
                panel: h!(0.0, 0.0, 0.94),
                mode: crate::components::theme::ThemeMode::Light,
                primary_active: darken_hsla(h!(210.0 / 360.0, 0.8, 0.5), 0.1),
                primary_hover: lighten_hsla(h!(0.0, 0.0, 0.95), 0.02),
                secondary_active: h!(0.0, 0.0, 0.92),
                sidebar_accent: h!(210.0 / 360.0, 0.8, 0.5),
                sidebar_accent_foreground: gpui::white(),
                transparent: Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: 0.0,
                },
                is_dark: false,
            }
        }
    }

    fn default_token() -> Self {
        Self::light()
    }

    // ── Field-access method compat (internal qingqi-ui code uses .field()) ──
    pub fn background(&self) -> Hsla {
        self.background
    }
    pub fn surface(&self) -> Hsla {
        self.surface
    }
    pub fn surface_hover(&self) -> Hsla {
        self.surface_hover
    }
    pub fn surface_active(&self) -> Hsla {
        self.surface_active
    }
    pub fn muted(&self) -> Hsla {
        self.muted
    }
    pub fn foreground(&self) -> Hsla {
        self.foreground
    }
    pub fn foreground_disabled(&self) -> Hsla {
        self.foreground_disabled
    }
    pub fn foreground_placeholder(&self) -> Hsla {
        self.foreground_placeholder
    }
    pub fn border(&self) -> Hsla {
        self.border
    }
    pub fn border_strong(&self) -> Hsla {
        self.border_strong
    }
    pub fn border_focus(&self) -> Hsla {
        self.border_focus
    }
    pub fn accent(&self) -> Hsla {
        self.accent
    }
    pub fn success(&self) -> Hsla {
        self.success
    }
    pub fn warning(&self) -> Hsla {
        self.warning
    }
    pub fn danger(&self) -> Hsla {
        self.danger
    }
    pub fn info(&self) -> Hsla {
        self.info
    }
    pub fn overlay(&self) -> Hsla {
        self.overlay
    }
    pub fn list(&self) -> Hsla {
        self.list
    }
    pub fn list_hover(&self) -> Hsla {
        self.list_hover
    }
    pub fn list_active(&self) -> Hsla {
        self.list_active
    }
    pub fn list_active_border(&self) -> Hsla {
        self.list_active_border
    }
    pub fn list_even(&self) -> Hsla {
        self.list_even
    }
    pub fn list_head(&self) -> Hsla {
        self.list_head
    }
    pub fn popover(&self) -> Hsla {
        self.popover
    }
    pub fn sidebar(&self) -> Hsla {
        self.sidebar
    }
    pub fn primary(&self) -> Hsla {
        self.primary
    }
    pub fn blue(&self) -> Hsla {
        self.blue
    }
    pub fn panel(&self) -> Hsla {
        self.panel
    }

    // ── Compound style methods ──────────────────────────────────────────
    pub fn danger_foreground(&self) -> Hsla {
        gpui::white()
    }
    pub fn danger_hover(&self) -> Hsla {
        lighten_hsla(self.danger, 0.1)
    }
    pub fn danger_active(&self) -> Hsla {
        darken_hsla(self.danger, 0.1)
    }

    pub fn warning_foreground(&self) -> Hsla {
        gpui::black()
    }
    pub fn warning_hover(&self) -> Hsla {
        lighten_hsla(self.warning, 0.1)
    }
    pub fn warning_active(&self) -> Hsla {
        darken_hsla(self.warning, 0.1)
    }

    pub fn success_foreground(&self) -> Hsla {
        gpui::white()
    }
    pub fn success_hover(&self) -> Hsla {
        lighten_hsla(self.success, 0.1)
    }
    pub fn success_active(&self) -> Hsla {
        darken_hsla(self.success, 0.1)
    }

    pub fn info_foreground(&self) -> Hsla {
        gpui::white()
    }
    pub fn info_hover(&self) -> Hsla {
        lighten_hsla(self.info, 0.1)
    }
    pub fn info_active(&self) -> Hsla {
        darken_hsla(self.info, 0.1)
    }

    pub fn primary_foreground(&self) -> Hsla {
        gpui::white()
    }
    pub fn primary_hover(&self) -> Hsla {
        if self.is_dark {
            lighten_hsla(self.primary, 0.1)
        } else {
            darken_hsla(self.primary, 0.05)
        }
    }
    pub fn primary_active(&self) -> Hsla {
        if self.is_dark {
            lighten_hsla(self.primary, 0.2)
        } else {
            darken_hsla(self.primary, 0.1)
        }
    }

    pub fn secondary_foreground(&self) -> Hsla {
        self.muted_foreground
    }
    pub fn secondary(&self) -> Hsla {
        self.surface
    }
    pub fn secondary_hover(&self) -> Hsla {
        self.surface_hover
    }
    pub fn secondary_active(&self) -> Hsla {
        self.surface_active
    }

    pub fn link_hover(&self) -> Hsla {
        lighten_hsla(self.accent, 0.1)
    }
    pub fn link_active(&self) -> Hsla {
        darken_hsla(self.accent, 0.1)
    }
    pub fn link(&self) -> Hsla {
        self.accent
    }

    pub fn scrollbar_thumb(&self) -> Hsla {
        self.muted_foreground.opacity(0.4)
    }
    pub fn scrollbar_thumb_hover(&self) -> Hsla {
        self.muted_foreground.opacity(0.6)
    }
    pub fn scrollbar(&self) -> Hsla {
        Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        }
    }
    pub fn scrollbar_show(&self) -> crate::components::scroll::ScrollbarShow {
        crate::components::scroll::ScrollbarShow::Scrolling
    }
    pub fn transparent(&self) -> Hsla {
        Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        }
    }
    pub fn radius(&self) -> gpui::Pixels {
        gpui::px(6.0)
    }

    pub fn mode(&self) -> crate::components::theme::ThemeMode {
        if self.is_dark {
            crate::components::theme::ThemeMode::Dark
        } else {
            crate::components::theme::ThemeMode::Light
        }
    }

    /// No-op (used to sync system appearance; we build it from saved state).
    pub fn sync_system_appearance(_window: Option<&mut gpui::Window>, _cx: &mut App) {}

    // Additional color aliases for vendor compat
    pub fn switch(&self) -> Hsla {
        self.muted_foreground.opacity(0.5)
    }
    pub fn switch_thumb(&self) -> Hsla {
        gpui::white()
    }
    pub fn sidebar_foreground(&self) -> Hsla {
        self.foreground
    }
    pub fn sidebar_accent(&self) -> Hsla {
        self.primary
    }
    pub fn sidebar_accent_foreground(&self) -> Hsla {
        gpui::white()
    }
    pub fn sidebar_border(&self) -> Hsla {
        self.border
    }
}

fn lighten_hsla(c: Hsla, amount: f32) -> Hsla {
    Hsla {
        l: (c.l + amount).clamp(0.0, 1.0),
        ..c
    }
}

fn darken_hsla(c: Hsla, amount: f32) -> Hsla {
    Hsla {
        l: (c.l - amount).clamp(0.0, 1.0),
        ..c
    }
}

#[derive(Clone)]
pub struct TokenState {
    pub token: Token,
}

impl Global for TokenState {}

pub fn tokens(cx: &App) -> Token {
    cx.try_global::<TokenState>()
        .map(|s| s.token.clone())
        .unwrap_or_else(Token::default_token)
}

pub fn install_tokens(cx: &mut App, dark: bool) {
    let token = Token::from_theme_name("default", dark);
    cx.set_global(TokenState { token });
}

pub fn tokens_mut(cx: &mut App) -> &mut Token {
    &mut cx.global_mut::<TokenState>().token
}
