//! Built-in, SVG-backed Lucide icons for GPUI.

use super::{Sizable, styled::Size};
use gpui::{
    App, Hsla, IntoElement, Pixels, RenderOnce, SharedString, Styled, Window,
    prelude::FluentBuilder, px, svg,
};

const DEFAULT_ICON_SIZE: Pixels = px(16.0);

#[derive(Clone, Debug, IntoElement)]
pub struct Icon {
    path: SharedString,
    size: Pixels,
    color: Option<Hsla>,
}

impl Icon {
    #[doc(hidden)]
    pub fn from_lucide_path(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            size: DEFAULT_ICON_SIZE,
            color: None,
        }
    }

    /// Create an Icon from an IconName enum variant.
    pub fn new(name: IconName) -> Self {
        let path = match name {
            IconName::ChevronDown => "lucide/chevron-down.svg",
            IconName::ChevronUp => "lucide/chevron-up.svg",
            IconName::ChevronRight => "lucide/chevron-right.svg",
            IconName::ChevronLeft => "lucide/chevron-left.svg",
            IconName::ArrowRight => "lucide/arrow-right.svg",
            IconName::ArrowLeft => "lucide/arrow-left.svg",
            IconName::ArrowUp => "lucide/arrow-up.svg",
            IconName::ArrowDown => "lucide/arrow-down.svg",
            IconName::Copy => "lucide/copy.svg",
            IconName::Delete => "lucide/trash-2.svg",
            IconName::Plus => "lucide/plus.svg",
            IconName::Minus => "lucide/minus.svg",
            IconName::File => "lucide/file.svg",
            IconName::FolderOpen => "lucide/folder-open.svg",
            IconName::Folder => "lucide/folder.svg",
            IconName::Settings => "lucide/settings.svg",
            IconName::Search => "lucide/search.svg",
            IconName::Refresh => "lucide/refresh-cw.svg",
            IconName::Save => "lucide/save.svg",
            IconName::Upload => "lucide/upload.svg",
            IconName::Download => "lucide/download.svg",
            IconName::Close => "lucide/x.svg",
            IconName::Check => "lucide/check.svg",
            IconName::Edit => "lucide/pencil.svg",
            IconName::Trash => "lucide/trash.svg",
            IconName::Info => "lucide/info.svg",
            IconName::Warning => "lucide/alert-triangle.svg",
            IconName::Error => "lucide/alert-circle.svg",
            IconName::Success => "lucide/check-circle.svg",
            IconName::Help => "lucide/help-circle.svg",
            IconName::User => "lucide/user.svg",
            IconName::Lock => "lucide/lock.svg",
            IconName::Unlock => "lucide/unlock.svg",
            IconName::Eye => "lucide/eye.svg",
            IconName::EyeOff => "lucide/eye-off.svg",
            IconName::Star => "lucide/star.svg",
            IconName::Heart => "lucide/heart.svg",
            IconName::Share => "lucide/share.svg",
            IconName::Link => "lucide/link.svg",
            IconName::Unlink => "lucide/link-2-off.svg",
            IconName::ExternalLink => "lucide/external-link.svg",
            IconName::Code => "lucide/code.svg",
            IconName::Terminal => "lucide/terminal.svg",
            IconName::Play => "lucide/play.svg",
            IconName::Pause => "lucide/pause.svg",
            IconName::Stop => "lucide/square.svg",
            IconName::Skip => "lucide/skip-forward.svg",
            IconName::Rewind => "lucide/rewind.svg",
            IconName::Filter => "lucide/filter.svg",
            IconName::Sort => "lucide/arrow-up-down.svg",
            IconName::Menu => "lucide/menu.svg",
            IconName::More => "lucide/more-horizontal.svg",
            IconName::Home => "lucide/home.svg",
            IconName::Mail => "lucide/mail.svg",
            IconName::Phone => "lucide/phone.svg",
            IconName::Calendar => "lucide/calendar.svg",
            IconName::Clock => "lucide/clock.svg",
            IconName::Map => "lucide/map.svg",
            IconName::MapPin => "lucide/map-pin.svg",
            IconName::Image => "lucide/image.svg",
            IconName::Video => "lucide/video.svg",
            IconName::Audio => "lucide/volume-2.svg",
            IconName::Camera => "lucide/camera.svg",
            IconName::Mic => "lucide/mic.svg",
            IconName::Print => "lucide/printer.svg",
            IconName::Scan => "lucide/scan.svg",
            IconName::Wifi => "lucide/wifi.svg",
            IconName::Bluetooth => "lucide/bluetooth.svg",
            IconName::Battery => "lucide/battery.svg",
            IconName::Signal => "lucide/signal.svg",
            IconName::Globe => "lucide/globe.svg",
            IconName::Shield => "lucide/shield.svg",
            IconName::Key => "lucide/key.svg",
            IconName::Hash => "lucide/hash.svg",
            IconName::AtSign => "lucide/at-sign.svg",
            IconName::Percent => "lucide/percent.svg",
            IconName::PlusCircle => "lucide/plus-circle.svg",
            IconName::MinusCircle => "lucide/minus-circle.svg",
            IconName::XCircle => "lucide/x-circle.svg",
            IconName::CheckCircle => "lucide/check-circle.svg",
            IconName::AlertCircle => "lucide/alert-circle.svg",
            IconName::AlertTriangle => "lucide/alert-triangle.svg",
            IconName::InfoCircle => "lucide/info.svg",
            IconName::HelpCircle => "lucide/help-circle.svg",
            IconName::RefreshCw => "lucide/refresh-cw.svg",
            IconName::RefreshCcw => "lucide/refresh-ccw.svg",
            IconName::RotateCw => "lucide/rotate-cw.svg",
            IconName::RotateCcw => "lucide/rotate-ccw.svg",
            IconName::ZoomIn => "lucide/zoom-in.svg",
            IconName::ZoomOut => "lucide/zoom-out.svg",
            IconName::Maximize => "lucide/maximize.svg",
            IconName::Minimize => "lucide/minimize.svg",
            IconName::Expand => "lucide/expand.svg",
            IconName::Collapse => "lucide/collapse.svg",
            IconName::Move => "lucide/move.svg",
            IconName::CornerUpLeft => "lucide/corner-up-left.svg",
            IconName::CornerUpRight => "lucide/corner-up-right.svg",
            IconName::CornerDownLeft => "lucide/corner-down-left.svg",
            IconName::CornerDownRight => "lucide/corner-down-right.svg",
        };
        Self::from_lucide_path(path)
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn text_color(self, color: impl Into<Hsla>) -> Self {
        self.color(color)
    }

    pub fn asset_path(&self) -> &str {
        self.path.as_ref()
    }
}

/// Enum representing all available Lucide icon names.
#[derive(Clone, Debug)]
pub enum IconName {
    ChevronDown,
    ChevronUp,
    ChevronRight,
    ChevronLeft,
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    ArrowDown,
    Copy,
    Delete,
    Plus,
    Minus,
    File,
    FolderOpen,
    Folder,
    Settings,
    Search,
    Refresh,
    Save,
    Upload,
    Download,
    Close,
    Check,
    Edit,
    Trash,
    Info,
    Warning,
    Error,
    Success,
    Help,
    User,
    Lock,
    Unlock,
    Eye,
    EyeOff,
    Star,
    Heart,
    Share,
    Link,
    Unlink,
    ExternalLink,
    Code,
    Terminal,
    Play,
    Pause,
    Stop,
    Skip,
    Rewind,
    Filter,
    Sort,
    Menu,
    More,
    Home,
    Mail,
    Phone,
    Calendar,
    Clock,
    Map,
    MapPin,
    Image,
    Video,
    Audio,
    Camera,
    Mic,
    Print,
    Scan,
    Wifi,
    Bluetooth,
    Battery,
    Signal,
    Globe,
    Shield,
    Key,
    Hash,
    AtSign,
    Percent,
    PlusCircle,
    MinusCircle,
    XCircle,
    CheckCircle,
    AlertCircle,
    AlertTriangle,
    InfoCircle,
    HelpCircle,
    RefreshCw,
    RefreshCcw,
    RotateCw,
    RotateCcw,
    ZoomIn,
    ZoomOut,
    Maximize,
    Minimize,
    Expand,
    Collapse,
    Move,
    CornerUpLeft,
    CornerUpRight,
    CornerDownLeft,
    CornerDownRight,
}

/// Convenience macro to create an Icon from a Lucide icon name.
#[macro_export]
macro_rules! icon {
    ($name:ident) => {
        $crate::components::Icon::from_lucide_path($crate::__private::lucide_path!($name))
    };
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        svg()
            .path(self.path)
            .flex_none()
            .size(self.size)
            .when_some(self.color, |icon, color| icon.text_color(color))
    }
}

impl Sizable for Icon {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = match size.into() {
            Size::Size(size) => size,
            Size::XSmall => px(12.0),
            Size::Small => px(14.0),
            Size::Medium => DEFAULT_ICON_SIZE,
            Size::Large => px(20.0),
        };
        self
    }
}

impl From<IconName> for Icon {
    fn from(name: IconName) -> Self {
        Self::new(name)
    }
}
