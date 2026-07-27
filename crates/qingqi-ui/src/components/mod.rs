// Local component implementations (migrated from vendor/qingqi-ui).
pub mod button;
pub mod divider;
pub mod icon;
pub mod list;
pub mod root;
pub mod scroll;
pub mod sidebar;
pub mod styled;
pub mod switch;
pub mod theme;
pub mod tree;
pub mod widgets;

// Pre-existing components.
pub mod checkbox;
pub mod input;
pub mod input_compat;
pub mod label;
pub mod radio;
pub mod scrollbar;
pub mod tab;
pub mod tooltip;

// ── Re-exports ─────────────────────────────────────────────────────────
pub use crate::token::Token;
pub use button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariant, ButtonVariants};
pub use checkbox::Checkbox;
pub use divider::{Divider, DividerStyle};
pub use icon::{Icon, IconName};
pub use input::{Input, InputState};
pub use list::ListItem;
pub use root::{Root, WindowExt};
pub use scroll::{ScrollableElement, ScrollbarExt};
pub use sidebar::{Collapsible, Side, Sidebar, SidebarMenu, SidebarMenuItem};
pub use styled::{Disableable, Selectable, Sizable, Size, StyleSized, StyledExt, h_flex, v_flex};
pub use switch::Switch;
pub use theme::{ActiveTheme, ThemeMode};
pub use tree::{TreeEntry, TreeItem, TreeState, tree};
pub use widgets::{Badge, DropdownMenu, Progress, SelectItem, Slider, Tag, TagVariant};

/// Initialize qingqi-ui global actions (called once by the app runtime).
pub fn init(cx: &mut gpui::App) {
    input::init(cx);
}
