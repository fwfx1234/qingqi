use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginCategory {
    Tool,
    System,
    About,
}

impl PluginCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tool => "工具",
            Self::System => "系统",
            Self::About => "关于",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginStatus {
    Ready,
    Background,
    Preview,
}

impl PluginStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ready => "可用",
            Self::Background => "后台",
            Self::Preview => "预览",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginAccent {
    Blue,
    Cyan,
    Green,
    Purple,
    Amber,
    Rose,
    Slate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    Inline,
    Window,
    List,
}

impl ViewMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Inline => "内嵌",
            Self::Window => "窗口",
            Self::List => "列表",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum WindowSize {
    Fixed {
        width: f32,
        height: f32,
    },
    Ratio {
        width: f32,
        height: f32,
    },
    /// Size to the available content area. For inline plugins this means
    /// the launcher panel flexes between a sensible min and max height;
    /// for window plugins this falls back to a default ratio.
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowSpec {
    pub size: WindowSize,
    pub always_on_top: bool,
}

// ── Compatibility aliases for ongoing Manifest → Manifest migration ──

pub type PluginWindowMode = ViewMode;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginVisualSpec {
    pub icon: crate::core::icon::IconRef,
    pub accent: PluginAccent,
    pub category: PluginCategory,
    pub status: PluginStatus,
    pub mode: PluginWindowMode,
    pub window: WindowSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginStats {
    pub primary: std::sync::Arc<str>,
    pub secondary: std::sync::Arc<str>,
    pub tertiary: std::sync::Arc<str>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginOverviewSection {
    pub title: std::sync::Arc<str>,
    pub body: std::sync::Arc<str>,
    pub items: Vec<std::sync::Arc<str>>,
}

impl PluginOverviewSection {
    pub fn new(
        title: impl Into<std::sync::Arc<str>>,
        body: impl Into<std::sync::Arc<str>>,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            items: Vec::new(),
        }
    }
}

impl WindowSpec {
    /// A fixed-size ordinary window (native OS titlebar + close button).
    pub const fn fixed(width: f32, height: f32) -> Self {
        Self {
            size: WindowSize::Fixed { width, height },
            always_on_top: false,
        }
    }

    /// A fixed-size floating panel that stays above other windows and draws
    /// its own close button (e.g. clipboard, anti-peeping).
    pub const fn fixed_topmost(width: f32, height: f32) -> Self {
        Self {
            size: WindowSize::Fixed { width, height },
            always_on_top: true,
        }
    }

    pub const fn ratio(width: f32, height: f32) -> Self {
        Self {
            size: WindowSize::Ratio { width, height },
            always_on_top: false,
        }
    }

    pub const fn auto() -> Self {
        Self {
            size: WindowSize::Auto,
            always_on_top: false,
        }
    }

    pub const fn from_size(size: WindowSize) -> Self {
        Self {
            size,
            always_on_top: false,
        }
    }
}
