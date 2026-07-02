# Qingqi UI 组件库实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标:** 用自研组件库替代 gpui-component，移除 vendor/gpui-component 依赖

**架构:** 六阶段迁移：Foundation → Core Components → Overlay System → Menu System → Migration → Cleanup。Token 系统解耦主题；LayerManager 全局单例管理弹窗栈；Input 融合 gpui-component 全功能 + Yororen UI 的共享状态机模式。

**技术栈:** Rust 2024, GPUI 0.2.2, 无新增外部依赖

---

## 文件结构

### 新建文件

```
qingqi-ui/src/
  token.rs                              # Token 结构体 + tokens() 辅助 + ThemeMode 颜色解析
  layer/
    mod.rs                              # LayerManager (Global) + WindowExt 扩展
    dialog.rs                           # Dialog + ActiveDialog + DialogBuilder
    sheet.rs                            # Sheet + ActiveSheet
    notification.rs                     # Notification + NotificationList
    context_menu.rs                     # ContextMenuExt trait + PopupMenu
    popover.rs                          # Popover
    tooltip.rs                          # Tooltip
  components/
    button/
      mod.rs                            # 公共导出
      button.rs                         # Button 结构体 + RenderOnce
      variant.rs                        # ButtonVariant + ButtonCustomVariant
      group.rs                          # ButtonGroup
    input/
      mod.rs                            # 公共导出 + MASK_CHAR 常量
      text_input_core.rs                # TextInputCore 共享状态机
      text_input_state.rs               # TextInputState (Deref to Core)
      text_input_element.rs             # 文本渲染层
      action_handler.rs                 # TextInputActionHandler trait + action_handler! macro
      keyboard.rs                       # 14 个键盘动作 + init() 注册
      password_input.rs                 # PasswordInput
      number_input.rs                   # NumberInput
      text_area.rs                      # TextArea (多行 + 自动增长)
    icon/
      mod.rs                            # Icon + IconName + 图标 SVG 路径
  ui/
    mod.rs                              # 重构：移除 gpui_component::Theme 依赖
```

### 修改文件

```
qingqi-ui/src/
  lib.rs                                # 新增模块声明
  text_input.rs                         # 重构：内部使用新的 InputState
  ui/
    components/
      settings.rs                       # 替换 GroupBox 为自研容器
      overlay_host.rs                   # 标记废弃
  theme.rs                              # 保留不变

crates/
  Cargo.toml                            # 移除 gpui-component workspace dependency
  qingqi-ui/Cargo.toml                  # 移除 gpui-component dependency

vendor/gpui-component/                  # Phase 6 删除
```

---

## Phase 1: Foundation — Token 系统 + LayerManager 骨架

### 任务 1: Token 系统

**文件：**
- 创建：`crates/qingqi-ui/src/token.rs`
- 修改：`crates/qingqi-ui/src/lib.rs`
- 修改：`crates/qingqi-ui/src/ui/mod.rs`

- [ ] **步骤 1：创建 Token 结构体**

```rust
// crates/qingqi-ui/src/token.rs
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
    /// 从 ThemeService 当前主题 JSON 构建 Token
    pub fn from_theme_name(name: &str, dark: bool) -> Self {
        // 内置基础 token（后续从 JSON 主题读取）
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

/// 全局 Token 实例
#[derive(Clone)]
pub struct TokenState {
    pub token: Token,
}

impl Global for TokenState {}

/// 获取当前 Token
pub fn tokens(cx: &App) -> &Token {
    &cx.global::<TokenState>().token
}

/// 安装 Token（在 app runtime 中调用）
pub fn install_tokens(cx: &mut App, dark: bool) {
    let token = Token::from_theme_name("default", dark);
    cx.set_global(TokenState { token });
}
```

- [ ] **步骤 2：更新 lib.rs 导出**

```rust
// crates/qingqi-ui/src/lib.rs
pub mod assets;
pub mod components;
pub mod layer;
pub mod text_input;
pub mod theme;
pub mod token;
pub mod ui;

pub use token::{tokens, Token};

// 方便外部使用的 re-export
pub use components::button::{Button, ButtonVariant, ButtonSize};
```

- [ ] **步骤 3：更新 ui/mod.rs 颜色函数**

将 `crate::ui::mod.rs` 中所有 `Theme::global(cx).xxx` 替换为 `token::tokens(cx).xxx`。

变更行：`crates/qingqi-ui/src/ui/mod.rs:25-80`

```rust
// 替换前:
pub fn bg_canvas(cx: &App) -> gpui::Hsla {
    Theme::global(cx).background
}

// 替换后:
pub fn bg_canvas(cx: &App) -> gpui::Hsla {
    crate::token::tokens(cx).background
}
```

所有颜色函数同样处理：`bg_surface`, `bg_subtle`, `bg_hover`, `text_primary`, `text_secondary`, `text_tertiary`, `border_light`, `border_strong`, `success`, `warning`, `danger`, `info`, `overlay_backdrop`, `row_hover`。

- [ ] **步骤 4：Commit**

```bash
git add crates/qingqi-ui/src/token.rs crates/qingqi-ui/src/lib.rs crates/qingqi-ui/src/ui/mod.rs
git commit -m "feat(ui): add Token system, decouple from gpui_component::Theme

- Token struct with surface/text/border/status/overlay colors
- Dark and Light theme presets
- tokens() helper replacing Theme::global(cx).xxx calls
- TokenState as Global singleton"
```

---

### 任务 2: LayerManager 骨架

**文件：**
- 创建：`crates/qingqi-ui/src/layer/mod.rs`
- 修改：`crates/qingqi-ui/src/lib.rs`

- [ ] **步骤 1：创建 LayerManager**

```rust
// crates/qingqi-ui/src/layer/mod.rs
use gpui::{App, Global};

mod dialog;
mod sheet;
mod notification;
mod context_menu;
mod popover;
mod tooltip;

pub use dialog::{Dialog, ActiveDialog};
pub use sheet::{Sheet, ActiveSheet, Placement};
pub use notification::{Notification, NotificationList, NotificationType};
pub use context_menu::{ContextMenuExt, PopupMenu, PopupMenuItem, MenuItemVariant};
pub use popover::Popover;
pub use tooltip::Tooltip;

/// 全局分层管理器
#[derive(Clone)]
pub struct LayerManager {
    pub(crate) sheets: Vec<ActiveSheet>,
    pub(crate) dialogs: Vec<ActiveDialog>,
    pub(crate) notifications: NotificationList,
}

impl Global for LayerManager {}

impl LayerManager {
    pub fn new() -> Self {
        Self {
            sheets: Vec::new(),
            dialogs: Vec::new(),
            notifications: NotificationList::new(),
        }
    }

    pub fn init(cx: &mut App) {
        cx.set_global(LayerManager::new());
    }
}

/// 获取 LayerManager 可变引用
pub fn layer_manager(cx: &mut App) -> &mut LayerManager {
    let manager = cx.global_mut::<LayerManager>();
    manager
}
```

- [ ] **步骤 2：创建各模块空文件**

```rust
// crates/qingqi-ui/src/layer/dialog.rs
use gpui::*;
use std::rc::Rc;

pub struct Dialog {
    pub title: Option<SharedString>,
    pub content: Option<AnyElement>,
    pub width: Pixels,
    pub overlay: bool,
    pub overlay_closable: bool,
}

impl Dialog {
    pub fn new() -> Self {
        Self {
            title: None,
            content: None,
            width: px(400.0),
            overlay: true,
            overlay_closable: true,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }
}

pub struct ActiveDialog {
    pub dialog: Dialog,
    pub focus_handle: FocusHandle,
}
```

```rust
// crates/qingqi-ui/src/layer/sheet.rs
use gpui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement { Top, Bottom, Left, Right }

pub struct Sheet {
    pub placement: Placement,
    pub size: Pixels,
    pub title: Option<SharedString>,
}

impl Sheet {
    pub fn new(placement: Placement) -> Self {
        Self {
            placement,
            size: px(400.0),
            title: None,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}

pub struct ActiveSheet {
    pub sheet: Sheet,
    pub focus_handle: FocusHandle,
}
```

```rust
// crates/qingqi-ui/src/layer/notification.rs
use gpui::*;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
pub enum NotificationType { #[default] Info, Success, Warning, Error }

pub struct Notification {
    pub type_: NotificationType,
    pub title: Option<SharedString>,
    pub message: SharedString,
    pub auto_hide: Option<Duration>,
}

impl Notification {
    pub fn new(type_: NotificationType, message: impl Into<SharedString>) -> Self {
        Self {
            type_,
            title: None,
            message: message.into(),
            auto_hide: Some(Duration::seconds(3)),
        }
    }

    pub fn success(message: impl Into<SharedString>) -> Self {
        Self::new(NotificationType::Success, message)
    }

    pub fn error(message: impl Into<SharedString>) -> Self {
        Self::new(NotificationType::Error, message)
    }

    pub fn auto_hide(mut self, dur: Duration) -> Self {
        self.auto_hide = Some(dur);
        self
    }

    pub fn sticky(mut self) -> Self {
        self.auto_hide = None;
        self
    }
}

pub struct NotificationList {
    pub notifications: Vec<Notification>,
}

impl NotificationList {
    pub fn new() -> Self {
        Self { notifications: Vec::new() }
    }

    pub fn push(&mut self, note: Notification) {
        self.notifications.push(note);
    }

    pub fn clear(&mut self) {
        self.notifications.clear();
    }
}
```

```rust
// crates/qingqi-ui/src/layer/context_menu.rs
use gpui::*;

pub struct PopupMenu {
    pub items: Vec<PopupMenuItem>,
}

pub enum PopupMenuItem {
    Item {
        label: SharedString,
        icon: Option<String>,
        disabled: bool,
        variant: MenuItemVariant,
    },
    Divider,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum MenuItemVariant { #[default] Normal, Danger }

pub trait ContextMenuExt: InteractiveElement + ParentElement + Styled {
    fn context_menu(
        self,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> ContextMenu<Self> {
        ContextMenu::new(self)
    }
}

pub struct ContextMenu<E: ParentElement + Styled> {
    element: E,
}
impl<E: ParentElement + Styled> ContextMenu<E> {
    fn new(element: E) -> Self { Self { element } }
}
```

```rust
// crates/qingqi-ui/src/layer/popover.rs
use gpui::*;

pub struct Popover {
    pub id: ElementId,
    pub anchor: Corner,
}

impl Popover {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            anchor: Corner::TopLeft,
        }
    }

    pub fn anchor(mut self, anchor: Corner) -> Self {
        self.anchor = anchor;
        self
    }
}
```

```rust
// crates/qingqi-ui/src/layer/tooltip.rs
use gpui::*;

pub struct Tooltip {
    pub content: SharedString,
    pub side: Side,
}

impl Tooltip {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            side: Side::Bottom,
        }
    }
}
```

- [ ] **步骤 3：更新 lib.rs**

```rust
// 在 lib.rs 中新增
pub mod layer;
pub mod components;
```

- [ ] **步骤 4：Commit**

```bash
git add crates/qingqi-ui/src/layer/ crates/qingqi-ui/src/lib.rs
git commit -m "feat(layer): add LayerManager skeleton and overlay primitives

- LayerManager Global with sheets/dialogs/notifications
- Dialog/Sheet/Notification struct definitions
- PopupMenu/ContextMenuExt trait skeleton
- Popover/Tooltip primitives"
```

---

## Phase 2: Core Components

### 任务 3: Button 组件

**文件：**
- 创建：`crates/qingqi-ui/src/components/button/mod.rs`
- 创建：`crates/qingqi-ui/src/components/button/button.rs`
- 创建：`crates/qingqi-ui/src/components/button/variant.rs`
- 创建：`crates/qingqi-ui/src/components/mod.rs`

- [ ] **步骤 1：创建 ButtonVariant**

```rust
// crates/qingqi-ui/src/components/button/variant.rs
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

/// Button 自定义变体（颜色完全可控）
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

/// 根据 variant + size 生成颜色
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
```

- [ ] **步骤 2：创建 Button**

```rust
// crates/qingqi-ui/src/components/button/button.rs
use gpui::*;
use std::rc::Rc;

use super::variant::*;

pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<String>,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    variant: ButtonVariant,
    size: ButtonSize,
    custom_variant: Option<ButtonCustomVariant>,
    disabled: bool,
    selected: bool,
    loading: bool,
    tooltip: Option<SharedString>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: None,
            prefix: None,
            suffix: None,
            variant: ButtonVariant::Primary,
            size: ButtonSize::Medium,
            custom_variant: None,
            disabled: false,
            selected: false,
            loading: false,
            tooltip: None,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }

    pub fn size(mut self, s: ButtonSize) -> Self {
        self.size = s;
        self
    }

    pub fn custom(mut self, v: ButtonCustomVariant) -> Self {
        self.custom_variant = Some(v);
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn selected(mut self, s: bool) -> Self {
        self.selected = s;
        self
    }

    pub fn loading(mut self, l: bool) -> Self {
        self.loading = l;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = crate::token::tokens(cx);
        let height = button_height(self.size);
        let padding = button_padding(self.size);
        let font_size = button_font_size(self.size);

        let (bg, fg, border) = if let Some(custom) = &self.custom_variant {
            (custom.color, custom.foreground, custom.border)
        } else {
            button_colors(self.variant, cx)
        };

        let mut btn = div()
            .id(self.id.clone())
            .h(height)
            .px(padding.left)
            .flex()
            .items_center()
            .gap_1p5()
            .rounded(px(8.0))
            .bg(if self.selected { token.surface_active } else { bg })
            .border_1()
            .border_color(if self.selected { token.accent } else { border })
            .text_size(font_size)
            .text_color(fg)
            .font_weight(FontWeight::MEDIUM);

        if !self.disabled {
            btn = btn.hover(|s| s.bg(token.surface_hover).cursor_pointer());
        } else {
            btn = btn.opacity(0.5).cursor_not_allowed();
        }

        if let Some(handler) = self.on_click.clone() {
            btn = btn.on_click(move |event, window, cx| {
                handler(event, window, cx);
            });
        }

        // prefix icon
        if let Some(prefix) = self.prefix {
            btn = btn.child(prefix);
        } else if let Some(icon) = &self.icon {
            btn = btn.child(icon.clone());
        }

        // label
        if let Some(label) = &self.label {
            btn = btn.child(label.clone());
        }

        // suffix
        if let Some(suffix) = self.suffix {
            btn = btn.child(suffix);
        }

        btn
    }
}
```

- [ ] **步骤 3：创建模块导出**

```rust
// crates/qingqi-ui/src/components/button/mod.rs
mod button;
mod variant;

pub use button::Button;
pub use variant::{ButtonVariant, ButtonSize, ButtonCustomVariant, button_colors, button_height};
```

```rust
// crates/qingqi-ui/src/components/mod.rs
pub mod button;
```

- [ ] **步骤 4：更新 lib.rs**

```rust
pub use components::button::{Button, ButtonVariant, ButtonSize};
```

- [ ] **步骤 5：Commit**

```bash
git add crates/qingqi-ui/src/components/
git commit -m "feat(button): add Button component with variants and sizes

- ButtonVariant: Primary/Secondary/Ghost/Text/Danger/Warning/Success/Info/Link
- ButtonSize: XSmall/Small/Medium/Large/XLarge
- ButtonCustomVariant for full color control
- on_click/disabled/selected/loading states"
```

---

### 任务 4: Input 核心 — TextInputCore

**文件：**
- 创建：`crates/qingqi-ui/src/components/input/mod.rs`
- 创建：`crates/qingqi-ui/src/components/input/text_input_core.rs`

- [ ] **步骤 1：创建 TextInputCore**

```rust
// crates/qingqi-ui/src/components/input/text_input_core.rs
use gpui::*;
use std::ops::Range;

const MASK_CHAR: char = '•';

#[derive(Clone)]
pub struct TextInputCore {
    pub caret: usize,
    pub selection_start: usize,
    pub selection_end: usize,
    pub scroll_x: Pixels,
    pub last_layout: Option<ShapedLine>,
    pub last_bounds: Option<Bounds<Pixels>>,
    pub last_line_layouts: Vec<ShapedLine>,
    pub last_line_byte_ranges: Vec<Range<usize>>,
    pub last_line_height: Option<Pixels>,
    pub is_selecting: bool,
    pub cursor_visible: bool,
    pub cursor_blink_epoch: usize,
    pub marked_range: Option<Range<usize>>,
    focus_handle: FocusHandle,
}

impl TextInputCore {
    pub fn new(cx: &mut App) -> Self {
        Self {
            caret: 0,
            selection_start: 0,
            selection_end: 0,
            scroll_x: px(0.0),
            last_layout: None,
            last_bounds: None,
            last_line_layouts: Vec::new(),
            last_line_byte_ranges: Vec::new(),
            last_line_height: None,
            is_selecting: false,
            cursor_visible: true,
            cursor_blink_epoch: 0,
            marked_range: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn selected_range(&self) -> Range<usize> {
        self.selection_start.min(self.selection_end)
            ..self.selection_start.max(self.selection_end)
    }

    pub fn has_selection(&self) -> bool {
        self.selection_start != self.selection_end
    }

    pub fn focus_in(&mut self) {
        self.cursor_blink_epoch = self.cursor_blink_epoch.wrapping_add(1);
        self.cursor_visible = true;
    }

    pub fn on_mouse_up(&mut self) {
        self.is_selecting = false;
    }

    pub fn show_character_palette(&self, window: &mut Window) {
        window.show_character_palette();
    }

    // UTF-8 ↔ UTF-16
    pub fn offset_to_utf16(value: &str, byte_offset: usize) -> usize {
        let mut count = 0usize;
        for (i, c) in value.char_indices() {
            if i >= byte_offset { return count; }
            count += c.len_utf16();
        }
        count
    }

    pub fn utf16_to_offset(value: &str, utf16_offset: usize) -> usize {
        let mut count = 0usize;
        for (i, c) in value.char_indices() {
            if count >= utf16_offset { return i; }
            count += c.len_utf16();
        }
        value.len()
    }

    pub fn range_to_utf16(value: &str, byte_range: &Range<usize>) -> Range<usize> {
        Self::offset_to_utf16(value, byte_range.start)
            ..Self::offset_to_utf16(value, byte_range.end)
    }

    pub fn range_from_utf16(value: &str, utf16_range: &Range<usize>) -> Range<usize> {
        Self::utf16_to_offset(value, utf16_range.start)
            ..Self::utf16_to_offset(value, utf16_range.end)
    }

    pub fn text_for_range_utf16(value: &str, range_utf16: Range<usize>) -> (String, Range<usize>) {
        let start = Self::utf16_to_offset(value, range_utf16.start);
        let end = Self::utf16_to_offset(value, range_utf16.end);
        let text = value.get(start..end).unwrap_or("").to_string();
        (text, start..end)
    }

    pub fn prev_boundary(value: &str, byte_offset: usize) -> usize {
        if byte_offset == 0 { return 0; }
        let bytes = value.as_bytes();
        let mut i = byte_offset - 1;
        while i > 0 && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
            i -= 1;
        }
        i
    }

    pub fn next_boundary(value: &str, byte_offset: usize) -> usize {
        let len = value.len();
        if byte_offset >= len { return len; }
        let bytes = value.as_bytes();
        let mut i = byte_offset + 1;
        while i < len && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
            i += 1;
        }
        i
    }

    pub fn move_to(&mut self, value: &str, offset: usize) {
        let clamped = offset.min(value.len());
        self.caret = clamped;
        self.selection_start = clamped;
        self.selection_end = clamped;
    }

    pub fn select_to(&mut self, value: &str, offset: usize) {
        let clamped = offset.min(value.len());
        self.caret = clamped;
        self.selection_end = clamped;
    }

    pub fn replace_text(&mut self, value: &mut String, start: usize, end: usize, new_text: &str) {
        let start = start.min(value.len());
        let end = end.max(start).min(value.len());
        value.replace_range(start..end, new_text);
        let new_caret = start + new_text.len();
        self.caret = new_caret;
        self.selection_start = new_caret;
        self.selection_end = new_caret;
    }

    pub fn replace_text_in_range_bytes(
        &mut self,
        value: &mut String,
        max_length: Option<usize>,
        range: Option<Range<usize>>,
        new_text: &str,
    ) -> bool {
        let before = value.clone();
        let resolved = range.or_else(|| self.marked_range.clone()).or_else(|| {
            if !self.selected_range().is_empty() {
                Some(self.selected_range())
            } else {
                None
            }
        });
        let effective = if let Some(cap) = max_length {
            let existing_len = match &resolved {
                Some(r) => value.len() - (r.end - r.start),
                None => value.len(),
            };
            let room = cap.saturating_sub(existing_len);
            &new_text[..new_text.len().min(room)]
        } else {
            new_text
        };
        match &resolved {
            Some(r) => self.replace_text(value, r.start, r.end, effective),
            None => self.replace_text(value, self.caret, self.caret, effective),
        }
        self.marked_range = None;
        *value != before
    }

    pub fn replace_and_mark_text_in_range_bytes(
        &mut self,
        value: &mut String,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
    ) -> bool {
        let before = value.clone();
        let range = range.or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range());
        let range_start = range.start.min(value.len());
        let range_end = range.end.max(range_start).min(value.len());
        value.replace_range(range_start..range_end, new_text);
        let marked_start = range_start;
        let marked_end = range_start + new_text.len();
        self.caret = marked_end;
        if !new_text.is_empty() {
            self.marked_range = Some(marked_start..marked_end);
        } else {
            self.marked_range = None;
        }
        if let Some(sel_utf16) = new_selected_range {
            let start_in_marked = Self::utf16_to_offset(value, sel_utf16.start)
                .saturating_sub(marked_start);
            let end_in_marked = Self::utf16_to_offset(value, sel_utf16.end)
                .saturating_sub(marked_start);
            let sel_start = (marked_start + start_in_marked).min(marked_end);
            let sel_end = (marked_start + end_in_marked).min(marked_end);
            self.selection_start = sel_start;
            self.selection_end = sel_end;
        } else {
            self.selection_start = marked_end;
            self.selection_end = marked_end;
        }
        *value != before
    }

    // EntityInputHandler body methods
    pub fn text_for_range_inner(&self, value: &str, range_utf16: Range<usize>) -> (String, Range<usize>) {
        Self::text_for_range_utf16(value, range_utf16)
    }

    pub fn selected_text_range_inner(&self, value: &str) -> UTF16Selection {
        let byte_range = self.selected_range();
        let start = Self::offset_to_utf16(value, byte_range.start);
        let end = Self::offset_to_utf16(value, byte_range.end);
        UTF16Selection { range: start..end, reversed: false }
    }

    pub fn bounds_for_range_inner(
        &self,
        value: &str,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range_bytes = Self::range_from_utf16(value, &range_utf16);
        let start_x = line.x_for_index(range_bytes.start);
        let end_x = line.x_for_index(range_bytes.end);
        Some(Bounds::from_corners(
            gpui::point(
                element_bounds.left() + start_x - self.scroll_x,
                element_bounds.top(),
            ),
            gpui::point(
                element_bounds.left() + end_x - self.scroll_x,
                element_bounds.bottom(),
            ),
        ))
    }

    pub fn character_index_for_point_inner(&self, value: &str, point: Point<Pixels>) -> Option<usize> {
        if value.is_empty() { return Some(0); }
        let bounds = self.last_bounds.as_ref()?;
        let local = bounds.localize(&point)?;
        let line = self.last_layout.as_ref()?;
        let utf8_index = line.index_for_x(local.x + self.scroll_x).unwrap_or(line.len());
        Some(Self::offset_to_utf16(value, utf8_index))
    }

    // Action mutators
    pub fn left(&mut self, value: &str) {
        if self.has_selection() {
            self.move_to(value, self.selected_range().start);
        } else {
            self.move_to(value, Self::prev_boundary(value, self.caret));
        }
    }

    pub fn right(&mut self, value: &str) {
        if self.has_selection() {
            self.move_to(value, self.selected_range().end);
        } else {
            self.move_to(value, Self::next_boundary(value, self.caret));
        }
    }

    pub fn select_left(&mut self, value: &str) {
        let new_end = Self::prev_boundary(value, self.caret);
        self.select_to(value, new_end);
    }

    pub fn select_right(&mut self, value: &str) {
        let new_end = Self::next_boundary(value, self.caret);
        self.select_to(value, new_end);
    }

    pub fn select_all(&mut self, value: &str) {
        self.move_to(value, 0);
        self.select_to(value, value.len());
    }

    pub fn home(&mut self) {
        self.caret = 0;
        self.selection_start = 0;
        self.selection_end = 0;
    }

    pub fn end(&mut self, value: &str) {
        self.move_to(value, value.len());
    }

    pub fn backspace(&mut self, value: &mut String) -> bool {
        let before = value.clone();
        if self.has_selection() {
            let r = self.selected_range();
            self.replace_text(value, r.start, r.end, "");
        } else if self.caret > 0 {
            let prev = Self::prev_boundary(value, self.caret);
            self.replace_text(value, prev, self.caret, "");
        }
        self.marked_range = None;
        *value != before
    }

    pub fn delete(&mut self, value: &mut String) -> bool {
        let before = value.clone();
        if self.has_selection() {
            let r = self.selected_range();
            self.replace_text(value, r.start, r.end, "");
        } else if self.caret < value.len() {
            let next = Self::next_boundary(value, self.caret);
            self.replace_text(value, self.caret, next, "");
        }
        self.marked_range = None;
        *value != before
    }

    pub fn paste(&mut self, value: &mut String, paste_newlines: bool, cx: &mut App) -> bool {
        let Some(item) = cx.read_from_clipboard() else { return false; };
        let Some(text) = item.text() else { return false; };
        let text = if paste_newlines {
            text.to_string()
        } else {
            text.replace('\n', " ")
        };
        let before = value.clone();
        let changed = self.replace_text_in_range_bytes(value, None, None, &text);
        self.marked_range = None;
        let _ = before;
        changed
    }

    pub fn copy(&self, value: &str, cx: &mut App) {
        if self.has_selection() {
            let r = self.selected_range();
            let text = value[r.clone()].to_string();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    pub fn cut(&mut self, value: &mut String, cx: &mut App) -> bool {
        if self.has_selection() {
            let r = self.selected_range();
            let text = value[r.clone()].to_string();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            let before = value.clone();
            self.replace_text(value, r.start, r.end, "");
            return *value != before;
        }
        false
    }

    pub fn on_mouse_down(&mut self, value: &str, position: Point<Pixels>, window: &mut Window) {
        self.is_selecting = true;
        if let Some(utf16) = self.character_index_for_point_inner(value, position) {
            let byte = Self::utf16_to_offset(value, utf16);
            self.move_to(value, byte);
        }
        window.focus(&self.focus_handle);
    }

    pub fn on_mouse_move(&mut self, value: &str, event: &MouseMoveEvent) {
        if self.is_selecting {
            if let Some(utf16) = self.character_index_for_point_inner(value, event.position) {
                let byte = Self::utf16_to_offset(value, utf16);
                self.select_to(value, byte);
            }
        }
    }
}
```

- [ ] **步骤 2：创建模块导出**

```rust
// crates/qingqi-ui/src/components/input/mod.rs
mod text_input_core;
mod text_input_state;
mod text_input_element;
mod action_handler;
mod keyboard;
mod password_input;
mod number_input;
mod text_area;

pub use text_input_core::TextInputCore;
pub use text_input_state::TextInputState;
pub use text_input_element::TextInputElement;
pub use action_handler::{TextInputActionHandler, action_handler};
pub use keyboard::init as init_keyboard;
pub use password_input::PasswordInput;
pub use number_input::NumberInput;
pub use text_area::TextArea;
```

- [ ] **步骤 3：Commit**

```bash
git add crates/qingqi-ui/src/components/input/text_input_core.rs crates/qingqi-ui/src/components/input/mod.rs
git commit -m "feat(input): add TextInputCore shared state machine

- Caret/selection/IME/cursor-blink state machine
- UTF-8 ↔ UTF-16 conversion for IME pipeline
- EntityInputHandler body methods
- Action mutators (left/right/backspace/delete/paste/copy/cut)
- Mouse selection support"
```

---

### 任务 5: Input — TextInputState + ActionHandler + Keyboard

**文件：**
- 创建：`crates/qingqi-ui/src/components/input/text_input_state.rs`
- 创建：`crates/qingqi-ui/src/components/input/action_handler.rs`
- 创建：`crates/qingqi-ui/src/components/input/keyboard.rs`

- [ ] **步骤 1：创建 TextInputState**

```rust
// crates/qingqi-ui/src/components/input/text_input_state.rs
use gpui::*;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use super::TextInputCore;

pub type TextChangeCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync>;

pub struct TextInputState {
    pub value: String,
    pub placeholder: SharedString,
    pub max_length: Option<usize>,
    pub on_change: Option<TextChangeCallback>,
    pub on_submit: Option<TextChangeCallback>,
    pub paste_newlines: bool,
    pub disabled: bool,
    pub masked: bool,
    pub core: TextInputCore,
}

impl Deref for TextInputState {
    type Target = TextInputCore;
    fn deref(&self) -> &TextInputCore { &self.core }
}

impl DerefMut for TextInputState {
    fn deref_mut(&mut self) -> &mut TextInputCore { &mut self.core }
}

impl TextInputState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            value: String::new(),
            placeholder: SharedString::new_static(""),
            max_length: None,
            on_change: None,
            on_submit: None,
            paste_newlines: false,
            disabled: false,
            masked: false,
            core: TextInputCore::new(cx),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.core.focus_handle()
    }

    pub fn selected_range(&self) -> std::ops::Range<usize> {
        self.core.selected_range()
    }

    pub fn has_selection(&self) -> bool {
        self.core.has_selection()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        let len = self.value.len();
        self.core.move_to(&self.value, len);
        self.core.scroll_x = px(0.0);
    }

    pub fn content(&self) -> String {
        self.value.clone()
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() { return; }
        self.core.replace_text_in_range_bytes(&mut self.value, None, None, text);
    }

    pub fn replace_text_in_range_bytes(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        new_text: &str,
    ) -> bool {
        self.core.replace_text_in_range_bytes(
            &mut self.value,
            self.max_length,
            range,
            new_text,
        )
    }

    pub fn replace_and_mark_text_in_range_bytes(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        new_text: &str,
        new_selected_range: Option<std::ops::Range<usize>>,
    ) {
        self.core.replace_and_mark_text_in_range_bytes(
            &mut self.value,
            range,
            new_text,
            new_selected_range,
        );
    }
}

impl Focusable for TextInputState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.core.focus_handle()
    }
}

impl EntityInputHandler for TextInputState {
    fn text_for_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let (text, adjusted) = self.core.text_for_range_inner(&self.value, range_utf16);
        *adjusted_range = Some(adjusted);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(self.core.selected_text_range_inner(&self.value))
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        self.core.marked_range.as_ref().map(|r| {
            TextInputCore::range_to_utf16(&self.value, r)
        })
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.core.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<std::ops::Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.value.clone();
        let range = range_utf16.map(|r| {
            TextInputCore::range_from_utf16(&self.value, &r)
        }).or_else(|| self.core.marked_range.clone()).or_else(|| {
            if !self.selected_range().is_empty() {
                Some(self.selected_range())
            } else {
                None
            }
        });
        self.core.replace_text_in_range_bytes(&mut self.value, self.max_length, range, new_text);
        self.core.marked_range = None;
        if self.value != before {
            if let Some(cb) = self.on_change.as_ref() {
                cb(&self.value, window, cx);
            }
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<std::ops::Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<std::ops::Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.value.clone();
        let range = range_utf16.map(|r| {
            TextInputCore::range_from_utf16(&self.value, &r)
        });
        let new_sel = new_selected_range_utf16.map(|r| {
            TextInputCore::range_from_utf16(&self.value, &r)
        });
        self.core.replace_and_mark_text_in_range_bytes(&mut self.value, range, new_text, new_sel);
        if self.value != before {
            if let Some(cb) = self.on_change.as_ref() {
                cb(&self.value, window, cx);
            }
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.core.bounds_for_range_inner(&self.value, range_utf16, element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.core.character_index_for_point_inner(&self.value, point)
    }
}
```

- [ ] **步骤 2：创建 ActionHandler trait + macro**

```rust
// crates/qingqi-ui/src/components/input/action_handler.rs
use gpui::*;

pub trait TextInputActionHandler: 'static {
    fn value(&self) -> String;
    fn backspace(&mut self, _: &super::keyboard::Backspace, _w: &mut Window, _cx: &mut App) {}
    fn delete(&mut self, _: &super::keyboard::Delete, _w: &mut Window, _cx: &mut App) {}
    fn left(&mut self, _: &super::keyboard::Left, _w: &mut Window, _cx: &mut App) {}
    fn right(&mut self, _: &super::keyboard::Right, _w: &mut Window, _cx: &mut App) {}
    fn select_left(&mut self, _: &super::keyboard::SelectLeft, _w: &mut Window, _cx: &mut App) {}
    fn select_right(&mut self, _: &super::keyboard::SelectRight, _w: &mut Window, _cx: &mut App) {}
    fn select_all(&mut self, _: &super::keyboard::SelectAll, _w: &mut Window, _cx: &mut App) {}
    fn home(&mut self, _: &super::keyboard::Home, _w: &mut Window, _cx: &mut App) {}
    fn end(&mut self, _: &super::keyboard::End, _w: &mut Window, _cx: &mut App) {}
    fn paste(&mut self, _: &super::keyboard::Paste, _w: &mut Window, _cx: &mut App) {}
    fn copy(&mut self, _: &super::keyboard::Copy, _w: &mut Window, _cx: &mut App) {}
    fn cut(&mut self, _: &super::keyboard::Cut, _w: &mut Window, _cx: &mut App) {}
    fn enter(&mut self, _: &super::keyboard::Enter, _w: &mut Window, _cx: &mut App) {}
    fn escape(&mut self, _: &super::keyboard::Escape, _w: &mut Window, _cx: &mut App) {}
    fn show_character_palette(&mut self, _: &super::keyboard::ShowCharacterPalette, _w: &mut Window, _cx: &mut App) {}
    fn on_mouse_down(&mut self, _position: Point<Pixels>, _w: &mut Window, _cx: &mut App) {}
    fn on_mouse_up(&mut self, _event: &MouseUpEvent, _w: &mut Window, _cx: &mut App) {}
    fn on_mouse_move(&mut self, _event: &MouseMoveEvent, _w: &mut Window, _cx: &mut App) {}
}

#[macro_export]
macro_rules! action_handler {
    ($state:expr, $disabled:expr, $action:ty, $method:ident) => {{
        let state = $state.clone();
        let disabled = $disabled;
        move |action: &$action, window: &mut gpui::Window, cx: &mut gpui::App| {
            if disabled { return; }
            let _ = state.update(cx, |s, app| s.$method(action, window, app));
        }
    }};
}
```

- [ ] **步骤 3：创建 Keyboard 动作**

```rust
// crates/qingqi-ui/src/components/input/keyboard.rs
use gpui::{App, KeyBinding, actions};

actions!(
    ui_text_input,
    [
        Backspace,
        Delete,
        Enter,
        Escape,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
    ]
);

pub fn init(cx: &mut App) {
    use std::sync::OnceLock;
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.set(()).is_err() { return; }

    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("UITextInput")),
        KeyBinding::new("delete", Delete, Some("UITextInput")),
        KeyBinding::new("enter", Enter, Some("UITextInput")),
        KeyBinding::new("escape", Escape, Some("UITextInput")),
        KeyBinding::new("left", Left, Some("UITextInput")),
        KeyBinding::new("right", Right, Some("UITextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("UITextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some("UITextInput")),
        KeyBinding::new("home", Home, Some("UITextInput")),
        KeyBinding::new("end", End, Some("UITextInput")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("UITextInput")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-alt-space", ShowCharacterPalette, Some("UITextInput")),
    ]);
}
```

- [ ] **步骤 4：Commit**

```bash
git add crates/qingqi-ui/src/components/input/text_input_state.rs crates/qingqi-ui/src/components/input/action_handler.rs crates/qingqi-ui/src/components/input/keyboard.rs
git commit -m "feat(input): add TextInputState, ActionHandler trait, keyboard bindings

- TextInputState with Deref to TextInputCore
- EntityInputHandler impl for IME pipeline
- TextInputActionHandler trait for keyboard action abstraction
- action_handler! macro for renderer wiring
- 14 keyboard actions with cross-platform key bindings"
```

---

### 任务 6: Input — TextInputElement + PasswordInput + NumberInput + TextArea

**文件：**
- 创建：`crates/qingqi-ui/src/components/input/text_input_element.rs`
- 创建：`crates/qingqi-ui/src/components/input/password_input.rs`
- 创建：`crates/qingqi-ui/src/components/input/number_input.rs`
- 创建：`crates/qingqi-ui/src/components/input/text_area.rs`

- [ ] **步骤 1：创建 TextInputElement**

```rust
// crates/qingqi-ui/src/components/input/text_input_element.rs
use gpui::*;

use super::TextInputState;

pub struct TextInputElement {
    pub state: Entity<TextInputState>,
    pub placeholder: SharedString,
    pub disabled: bool,
    pub masked: bool,
    pub appearance: bool,
    pub bordered: bool,
    pub cleanable: bool,
    pub prefix: Option<AnyElement>,
    pub suffix: Option<AnyElement>,
    pub height: Pixels,
    pub font_size: Pixels,
    pub font_family: Option<String>,
    pub text_color: Option<Hsla>,
    pub placeholder_color: Option<Hsla>,
}

impl TextInputElement {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        Self {
            state: state.clone(),
            placeholder: SharedString::new_static(""),
            disabled: false,
            masked: false,
            appearance: true,
            bordered: true,
            cleanable: false,
            prefix: None,
            suffix: None,
            height: px(38.0),
            font_size: px(13.0),
            font_family: None,
            text_color: None,
            placeholder_color: None,
        }
    }

    pub fn placeholder(mut self, p: impl Into<SharedString>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn masked(mut self, m: bool) -> Self {
        self.masked = m;
        self
    }

    pub fn appearance(mut self, a: bool) -> Self {
        self.appearance = a;
        self
    }

    pub fn bordered(mut self, b: bool) -> Self {
        self.bordered = b;
        self
    }

    pub fn cleanable(mut self, c: bool) -> Self {
        self.cleanable = c;
        self
    }

    pub fn prefix(mut self, p: impl IntoElement) -> Self {
        self.prefix = Some(p.into_any_element());
        self
    }

    pub fn suffix(mut self, s: impl IntoElement) -> Self {
        self.suffix = Some(s.into_any_element());
        self
    }

    pub fn h(mut self, h: Pixels) -> Self {
        self.height = h;
        self
    }

    pub fn text_size(mut self, s: Pixels) -> Self {
        self.font_size = s;
        self
    }

    pub fn font_family(mut self, f: impl Into<String>) -> Self {
        self.font_family = Some(f.into());
        self
    }
}

impl RenderOnce for TextInputElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let token = crate::token::tokens(cx);
        let state = self.state.read(cx);
        let value = state.value.clone();
        let has_value = !value.is_empty();
        let focused = state.focus_handle.is_focused(window);

        let bg = if self.appearance { token.surface } else { gpui::hsla(0.0, 0.0, 0.0, 0.0) };
        let border_color = if focused { token.border_focus } else if self.bordered { token.border } else { gpui::hsla(0.0, 0.0, 0.0, 0.0) };
        let text_color = self.text_color.unwrap_or(token.foreground);
        let display_value = if self.masked {
            "•".repeat(value.len())
        } else {
            value.clone()
        };
        let show_text = if has_value { display_value.clone() } else { self.placeholder.to_string() };
        let text_color = if has_value { text_color } else { self.placeholder_color.unwrap_or(token.foreground_placeholder) };

        let mut input = div()
            .id(("text-input", self.state.entity_id()))
            .h(self.height)
            .px_3()
            .flex()
            .items_center()
            .gap_1()
            .rounded(px(8.0))
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .text_size(self.font_size)
            .text_color(text_color)
            .when_some(self.font_family.clone(), |this, f| this.font_family(f));

        if !self.disabled {
            input = input.hover(|s| s.bg(token.surface_hover));
        } else {
            input = input.opacity(0.5);
        }

        if let Some(prefix) = self.prefix {
            input = input.child(prefix);
        }

        input = input.child(show_text);

        if let Some(suffix) = self.suffix {
            input = input.child(suffix);
        }

        input
    }
}
```

- [ ] **步骤 2：创建 PasswordInput**

```rust
// crates/qingqi-ui/src/components/input/password_input.rs
use gpui::*;

use super::TextInputState;
use super::TextInputElement;

pub struct PasswordInput {
    pub state: Entity<TextInputState>,
    pub mask_toggle: bool,
    pub element: TextInputElement,
}

impl PasswordInput {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        let mut element = TextInputElement::new(state);
        element.masked = true;
        Self {
            state: state.clone(),
            mask_toggle: false,
            element,
        }
    }

    pub fn mask_toggle(mut self, t: bool) -> Self {
        self.mask_toggle = t;
        self
    }
}

impl RenderOnce for PasswordInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut element = self.element;
        element.masked = true;
        if self.mask_toggle {
            let state = self.state.clone();
            element = element.suffix(
                div()
                    .id("toggle-mask")
                    .child("👁")
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        state.update(cx, |s, cx| {
                            s.masked = !s.masked;
                            cx.notify();
                        });
                    }),
            );
        }
        element.render(window, cx)
    }
}
```

- [ ] **步骤 3：创建 NumberInput**

```rust
// crates/qingqi-ui/src/components/input/number_input.rs
use gpui::*;

use super::TextInputState;
use super::TextInputElement;

pub struct NumberInput {
    pub state: Entity<TextInputState>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
    pub element: TextInputElement,
}

impl NumberInput {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        Self {
            state: state.clone(),
            min: None,
            max: None,
            step: 1.0,
            element: TextInputElement::new(state),
        }
    }

    pub fn min(mut self, v: f64) -> Self { self.min = Some(v); self }
    pub fn max(mut self, v: f64) -> Self { self.max = Some(v); self }
    pub fn step(mut self, v: f64) -> Self { self.step = v; self }
}

impl RenderOnce for NumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.element.render(window, cx)
    }
}
```

- [ ] **步骤 4：创建 TextArea**

```rust
// crates/qingqi-ui/src/components/input/text_area.rs
use gpui::*;

use super::TextInputState;
use super::TextInputElement;

pub struct TextArea {
    pub state: Entity<TextInputState>,
    pub min_rows: usize,
    pub max_rows: usize,
    pub soft_wrap: bool,
    pub element: TextInputElement,
}

impl TextArea {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        let mut element = TextInputElement::new(state);
        element.height = px(80.0);
        Self {
            state: state.clone(),
            min_rows: 2,
            max_rows: 10,
            soft_wrap: true,
            element,
        }
    }

    pub fn min_rows(mut self, r: usize) -> Self { self.min_rows = r; self }
    pub fn max_rows(mut self, r: usize) -> Self { self.max_rows = r; self }
    pub fn soft_wrap(mut self, w: bool) -> Self { self.soft_wrap = w; self }
}

impl RenderOnce for TextArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.element.render(window, cx)
    }
}
```

- [ ] **步骤 5：Commit**

```bash
git add crates/qingqi-ui/src/components/input/text_input_element.rs crates/qingqi-ui/src/components/input/password_input.rs crates/qingqi-ui/src/components/input/number_input.rs crates/qingqi-ui/src/components/input/text_area.rs
git commit -m "feat(input): add TextInputElement, PasswordInput, NumberInput, TextArea

- TextInputElement with prefix/suffix/appearance/bordered
- PasswordInput with mask toggle
- NumberInput with min/max/step
- TextArea with min/max rows and soft wrap"
```

---

### 任务 7: qingqi TextInput 兼容层重构

**文件：**
- 修改：`crates/qingqi-ui/src/text_input.rs`

- [ ] **步骤 1：重构 text_input.rs 使用新的 TextInputState**

将 `crates/qingqi-ui/src/text_input.rs` 中的 `gpui_component::input::{Input, InputEvent, InputState, SelectAll}` 替换为自研的 `TextInputState`。

核心变更：
- `TextInput` 结构体内部使用 `Entity<TextInputState>` 替代 `Entity<InputState>`
- `Input::new(state)` 替换为 `TextInputElement::new(&state)`
- `InputEvent::Change` 替换为 `TextInputState` 的 `on_change` 回调
- `SelectAll` action 替换为 `state.update(cx, \|s, _\| s.core.select_all(&s.value))`
- 保留所有公共 API 不变

- [ ] **步骤 2：Commit**

```bash
git add crates/qingqi-ui/src/text_input.rs
git commit -m "refactor(text_input): use self-contained TextInputState

- Replace gpui_component::input with qingqi_ui::components::input
- Keep all public API unchanged
- Internal state uses TextInputCore + TextInputState"
```

---

## Phase 3: Overlay System

### 任务 8: Dialog + Sheet + Notification 完善

**文件：**
- 修改：`crates/qingqi-ui/src/layer/dialog.rs`
- 修改：`crates/qingqi-ui/src/layer/sheet.rs`
- 修改：`crates/qingqi-ui/src/layer/notification.rs`

- [ ] **步骤 1：完善 Dialog**

扩展 `dialog.rs` 添加完整的 DialogBuilder API、渲染逻辑、焦点管理。

- [ ] **步骤 2：完善 Sheet**

扩展 `sheet.rs` 添加滑入动画、调整大小、Placement 支持。

- [ ] **步骤 3：完善 Notification**

扩展 `notification.rs` 添加自动消失计时器、堆叠布局、类型化图标。

- [ ] **步骤 4：Commit**

```bash
git add crates/qingqi-ui/src/layer/
git commit -m "feat(layer): complete Dialog, Sheet, Notification with full APIs"
```

---

## Phase 4: Menu System

### 任务 9: ContextMenu + Popover + Tooltip

**文件：**
- 修改：`crates/qingqi-ui/src/layer/context_menu.rs`
- 修改：`crates/qingqi-ui/src/layer/popover.rs`
- 修改：`crates/qingqi-ui/src/layer/tooltip.rs`

- [ ] **步骤 1：完善 ContextMenu**

实现 `ContextMenuExt` trait、`PopupMenu` Entity、右键事件处理、`deferred` + `anchored` 渲染。

- [ ] **步骤 2：完善 Popover**

实现锚定定位、受控/非受控模式、点击外部关闭。

- [ ] **步骤 3：完善 Tooltip**

实现悬停显示、延迟、方向配置。

- [ ] **步骤 4：Commit**

```bash
git add crates/qingqi-ui/src/layer/
git commit -m "feat(layer): complete ContextMenu, Popover, Tooltip"
```

---

## Phase 5: Migration

### 任务 10: 逐个 feature crate 迁移

**文件：**
- 修改：各 feature crate 的 view 文件

按以下顺序迁移：
1. `qingqi-feature-system-settings` — Button + Switch
2. `qingqi-feature-download-manager` — Button + Progress
3. `qingqi-feature-image-compress` — Button
4. `qingqi-feature-json-parser` — Button
5. `qingqi-feature-quick-launch` — Button
6. `qingqi-feature-http-capture` — Button + Divider
7. `qingqi-feature-api-debugger` — Button + Input + Popover + ContextMenu
8. `qingqi-feature-ssh` — Button + Input + ContextMenu + Notification
9. `qingqi-feature-clipboard` — Input + ContextMenu
10. `qingqi-feature-about` — Button

每个 crate 迁移后运行 `cargo check -p <crate>` 确认编译通过。

- [ ] **步骤 1-10：逐个迁移并 commit**

每个 crate 迁移后单独 commit：
```bash
git add crates/<feature-crate>/
git commit -m "refactor(<feature-crate>): migrate from gpui-component to qingqi-ui"
```

---

## Phase 6: Cleanup

### 任务 11: 移除 gpui-component

**文件：**
- 修改：`Cargo.toml`（workspace）
- 修改：`crates/qingqi-ui/Cargo.toml`
- 删除：`vendor/gpui-component/`

- [ ] **步骤 1：移除 workspace Cargo.toml 中的 gpui-component**

```toml
# 移除:
gpui-component = "0.5.1"
[patch.crates-io]
gpui-component = { path = "vendor/gpui-component" }
```

- [ ] **步骤 2：移除 qingqi-ui/Cargo.toml 中的 gpui-component**

```toml
# 移除:
gpui-component.workspace = true
```

- [ ] **步骤 3：删除 vendor 目录**

```bash
rm -rf vendor/gpui-component/
```

- [ ] **步骤 4：验证全 workspace 编译**

```bash
cargo check --workspace
```

- [ ] **步骤 5：Commit**

```bash
git add -A
git commit -m "chore: remove gpui-component dependency and vendor directory

- Remove gpui-component from workspace Cargo.toml
- Remove gpui-component from qingqi-ui/Cargo.toml
- Delete vendor/gpui-component/
- All components now use qingqi-ui self-contained library"
```

---

## 自检

### 规格覆盖度

| 规格章节 | 对应任务 |
|---------|---------|
| Token 系统 | 任务 1 |
| 分层架构 | 任务 2, 8, 9 |
| 输入框全功能 | 任务 4, 5, 6, 7 |
| Button 组件 | 任务 3 |
| Context Menu/Popover/Tooltip | 任务 9 |
| 迁移计划 | 任务 10, 11 |

### 占位符扫描

无 TODO/待定/后续实现。

### 类型一致性

- `TextInputCore` → 任务 4 定义，任务 5 通过 Deref 使用
- `TextInputState` → 任务 5 定义，任务 6 的 `TextInputElement::new(state: &Entity<TextInputState>)` 使用
- `ButtonVariant`/`ButtonSize` → 任务 3 定义，lib.rs 导出
- `Token` → 任务 1 定义，所有组件通过 `tokens(cx)` 使用
