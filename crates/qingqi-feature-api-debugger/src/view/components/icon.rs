use gpui::{Component, Styled, div, px, AnyElement, IntoElement, ParentElement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconName {
    ChevronDown,
    ChevronRight,
    Folder,
    FolderOpen,
    FolderClosed,
    SquareTerminal,
    Plus,
    Close,
    ArrowRight,
    File,
    TriangleAlert,
    Copy,
    CaseSensitive,
}

impl IconName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChevronDown => "▼",
            Self::ChevronRight => "\u{25B6}",
            Self::Folder => "\u{1F4C1}",
            Self::FolderOpen => "\u{1F4C2}",
            Self::FolderClosed => "\u{1F4C1}",
            Self::SquareTerminal => "\u{25A0}",
            Self::Plus => "+",
            Self::Close => "\u{2715}",
            Self::ArrowRight => "\u{2192}",
            Self::File => "\u{1F4C4}",
            Self::TriangleAlert => "\u{26A0}",
            Self::Copy => "\u{1F4CB}",
            Self::CaseSensitive => "Aa",
        }
    }
}

pub fn icon(name: IconName) -> qingqi_ui::components::icon::Icon {
    qingqi_ui::components::icon::Icon::new(name.as_str())
}

pub fn icon_div(name: IconName, size: f32) -> gpui::Div {
    let icon = qingqi_ui::components::icon::Icon::new(name.as_str())
        .text_size(px(size));
    div().child(Component::new(icon))
}

pub fn icon_any_element(name: IconName, size: f32) -> AnyElement {
    let icon = qingqi_ui::components::icon::Icon::new(name.as_str())
        .text_size(px(size));
    div().child(Component::new(icon)).into_any_element()
}
