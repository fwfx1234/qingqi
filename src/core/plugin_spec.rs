use gpui::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginAccent {
    Blue,
    Cyan,
    Green,
    Purple,
    Amber,
    Rose,
    Slate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginWindowMode {
    Inline,
    Window,
    List,
}

impl PluginWindowMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Inline => "内嵌",
            Self::Window => "窗口",
            Self::List => "列表",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowSize {
    Fixed { width: f32, height: f32 },
    Ratio { width: f32, height: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSpec {
    pub size: WindowSize,
    pub always_on_top: bool,
}

impl WindowSpec {
    pub const fn fixed(width: f32, height: f32) -> Self {
        Self {
            size: WindowSize::Fixed { width, height },
            always_on_top: false,
        }
    }

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
}

#[derive(Clone, Copy, Debug)]
pub struct PluginVisualSpec {
    pub icon: &'static str,
    pub accent: PluginAccent,
    pub category: PluginCategory,
    pub status: PluginStatus,
    pub mode: PluginWindowMode,
    pub window: WindowSpec,
}

#[derive(Clone, Copy, Debug)]
pub struct PluginStats {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub tertiary: &'static str,
}

#[derive(Clone, Debug)]
pub struct PluginOverviewSection {
    pub title: SharedString,
    pub body: SharedString,
}

impl PluginOverviewSection {
    pub fn new(title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
        }
    }
}
