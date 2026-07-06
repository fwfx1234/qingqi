//! Icon + IconName — local replacement for qingqi-ui::icon.

use super::styled::Size;
use super::Sizable;
use gpui::{Styled, App, IntoElement, RenderOnce, SharedString, Hsla, Window, svg};

pub trait IconNamed { fn path(self) -> SharedString; }

#[derive(Clone, Debug)]
pub enum IconName {
    ALargeSmall, ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Asterisk, Bell, BookOpen, Bot,
    Building2, Calendar, CaseSensitive, ChartPie, Check, ChevronDown, ChevronLeft, ChevronRight,
    ChevronsUpDown, ChevronUp, CircleCheck, CircleUser, CircleX, Close, Copy, Dash, Delete,
    Ellipsis, EllipsisVertical, ExternalLink, Eye, EyeOff, File, Folder, FolderClosed, FolderOpen,
    Frame, GalleryVerticalEnd, GitHub, Globe, Heart, HeartOff, Inbox, Info, Inspector,
    LayoutDashboard, Loader, LoaderCircle, Map, Maximize, Menu, Minimize, Minus, Moon, Palette,
    PanelBottom, PanelLeft, PanelRight, Pencil, Phone, Play, Plus, Quote, Refresh, Regex, Settings2,
    Replace, Search, Settings, Space, SquareTerminal, Star, StarOff, StopCircle, Sun, Terminal, Trash,
    Undo, Upload, User, X, ZoomIn, ZoomOut,
    TriangleAlert,
}

impl IconNamed for IconName {
    fn path(self) -> SharedString {
        let name = format!("{:?}", self).to_lowercase();
        let converted = match name.as_str() {
            "alargesmall" => "a-large-small",
            "arrowdown" => "arrow-down", "arrowleft" => "arrow-left",
            "arrowright" => "arrow-right", "arrowup" => "arrow-up",
            "bookopen" => "book-open", "building2" => "building-2",
            "casesensitive" => "case-sensitive", "chartpie" => "chart-pie",
            "chevrondown" => "chevron-down", "chevronleft" => "chevron-left",
            "chevronright" => "chevron-right", "chevronsupdown" => "chevrons-up-down",
            "chevronup" => "chevron-up", "circlecheck" => "circle-check",
            "circleuser" => "circle-user", "circlex" => "circle-x",
            "ellipsisvertical" => "ellipsis-vertical", "externallink" => "external-link",
            "eyeoff" => "eye-off", "eyeon" => "eye",
            "folderclosed" => "folder-closed", "folderopen" => "folder-open",
            "galleryverticalend" => "gallery-vertical-end", "github" => "github-root",
            "heartoff" => "heart-off", "layoutdashboard" => "layout-dashboard",
            "loadercircle" | "loader" => "loader",
            "panelbottom" => "panel-bottom", "panelleft" => "panel-left",
            "panelright" => "panel-right", "squareterminal" => "square-terminal",
            "stopcircle" => "stop-circle",
            "zoomin" => "zoom-in", "zoomout" => "zoom-out",
            "refresh" => "refresh-cw",
            _ => return kebab_from_camel(&name).into(),
        };
        SharedString::from(format!("icons/{}.svg", converted))
    }
}

fn kebab_from_camel(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 { result.push('-'); }
        result.push(c.to_ascii_lowercase());
    }
    format!("icons/{}.svg", result)
}

impl IconNamed for &'static str {
    fn path(self) -> SharedString {
        SharedString::from(if self.starts_with("icons/") { self.to_string() } else { format!("icons/{}.svg", self) })
    }
}

impl IconNamed for String {
    fn path(self) -> SharedString {
        SharedString::from(if self.starts_with("icons/") { self } else { format!("icons/{}.svg", self) })
    }
}

#[derive(IntoElement, Clone)]
pub struct Icon {
    path: SharedString,
    size: Option<Size>,
    color: Option<Hsla>,
}

impl Icon {
    pub fn new(name: impl IconNamed) -> Self { Self::build(name) }
    pub fn build(named: impl IconNamed) -> Self { Self { path: named.path(), size: None, color: None } }
    pub fn text_color(mut self, color: impl Into<Hsla>) -> Self { self.color = Some(color.into()); self }
    pub fn size(mut self, size: impl Into<Size>) -> Self { self.size = Some(size.into()); self }
}

impl From<IconName> for Icon {
    fn from(name: IconName) -> Self { Self::new(name) }
}

impl Sizable for Icon {
    fn with_size(mut self, size: impl Into<Size>) -> Self { self.size = Some(size.into()); self }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let size_px = match self.size {
            Some(Size::Size(p)) => p / gpui::px(1.0),
            _ => 16.0,
        };
        let svg_el = svg().path(self.path.clone()).flex_none();
        let sized = svg_el.w(gpui::px(size_px)).h(gpui::px(size_px));
        if let Some(color) = self.color { sized.text_color(color) } else { sized }
    }
}
