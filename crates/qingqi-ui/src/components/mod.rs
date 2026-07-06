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
pub mod tab;
pub mod tooltip;
pub mod scrollbar;

// ── Re-exports ─────────────────────────────────────────────────────────
pub use button::{Button, ButtonVariant, ButtonVariants, ButtonRounded, ButtonCustomVariant};
pub use divider::{Divider, DividerStyle};
pub use icon::{Icon, IconNamed, IconName};
pub use list::ListItem;
pub use scroll::{ScrollableElement, ScrollbarExt};
pub use root::{Root, WindowExt};
pub use sidebar::{Sidebar, SidebarMenu, SidebarMenuItem, Side, Collapsible};
pub use styled::{Size, Sizable, Disableable, Selectable, StyleSized, StyledExt, h_flex, v_flex};
pub use switch::Switch;
pub use tree::{TreeItem, TreeEntry, TreeState, tree};
pub use checkbox::Checkbox;
pub use theme::{ActiveTheme, ThemeMode};
pub use widgets::{Tag, TagVariant, Badge, Progress, DropdownMenu, Slider, SelectItem};
pub use crate::token::Token;
pub use input::{Input, InputState};

/// Initialize function for qingqi-ui (called by app runtime).
pub fn init(_cx: &mut gpui::App) {}
