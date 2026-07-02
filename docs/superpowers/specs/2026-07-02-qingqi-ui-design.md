# Qingqi UI 组件库设计

**日期:** 2026-07-02  
**状态:** 设计完成，待实现  
**范围:** 用自研组件库替代 gpui-component，移除 vendor/gpui-component 依赖

---

## 1. 背景与目标

### 当前问题

qingqi 重度依赖 `gpui-component`（vendored at `vendor/gpui-component/`），该库：
- 通过 `[patch.crates-io]` 覆盖 crates.io 版本
- 提供 60+ 组件但 qingqi 仅使用约 15 个
- 主题系统与 qingqi 自身的 `ThemeService` 存在冗余
- 增加编译时间和二进制体积

### 目标

1. 在 `qingqi-ui` 中构建自研组件库，彻底移除 gpui-component 依赖
2. 支持完整暗色/亮色主题切换（复用现有 `ThemeService` + JSON 主题）
3. 保持 qingqi 现有 API 兼容性（`TextInput` 等封装层不变）
4. 吸收 gpui-component、Zed UI、Yororen UI 的最佳设计

### 调研来源

| 库 | Stars | 借鉴内容 |
|----|-------|---------|
| gpui-component (vendored) | 12k | 分层架构、Dialog/Sheet/Notification、ContextMenuExt |
| gpui-component (最新) | 12k | FocusTrapElement、WindowExt 模式 |
| Zed UI (`crates/ui/`) | 86k | Color 语义枚举、DynamicSpacing、Modal 结构 |
| Yororen UI | 333 | TextInputCore 共享状态机、ActionHandler trait、Deref 模式 |

---

## 2. 设计 Token 系统

### Token 结构体

```rust
pub struct Token {
    // Surface
    pub background: Hsla,
    pub surface: Hsla,
    pub surface_hover: Hsla,
    pub surface_active: Hsla,
    pub muted: Hsla,
    // Text
    pub foreground: Hsla,
    pub foreground_muted: Hsla,
    pub foreground_disabled: Hsla,
    pub foreground_placeholder: Hsla,
    // Border
    pub border: Hsla,
    pub border_strong: Hsla,
    pub border_focus: Hsla,
    // Status
    pub accent: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
    pub info: Hsla,
    // Overlay
    pub overlay: Hsla,
}
```

### 主题模式

复用现有 `ThemeMode { Light, Dark, System }` + `ThemeService`。Token 根据模式从 JSON 主题文件中读取颜色值。

### 辅助函数

- `tokens(cx) -> &Token` — 从全局 Theme 获取当前 Token
- 保留 `theme.rs` 中的 spacing/radii/font_size 函数

### 文件结构

```
qingqi-ui/src/
  token.rs              # Token 结构体 + tokens() 辅助函数
  theme.rs              # 保留现有 spacing/radii/accent（不变）
```

---

## 3. 分层与弹窗系统

### 分层架构

```
Layer 0: Window Content（正常视图）
Layer 1: Sheet（侧边滑入面板，同一时间只有一个）
Layer 2: Dialog（模态/非模态对话框，支持堆叠）
Layer 3: Popover（左击触发，锚定到元素）
Layer 4: Context Menu（右击触发，跟随光标）
Layer 5: Notification（右上角通知列表）
Layer 6: Tooltip（悬停提示）
```

### 关键设计决策

1. **渲染顺序 = z-index**：GPUI 没有独立 z-index 属性，后渲染的元素自然在最上层。`LayerManager` 作为窗口最后一个 child 渲染。
2. **Dialog 堆叠**：`dialogs: Vec<ActiveDialog>`，新 dialog push 到末尾（最上层）。
3. **Popover 不纳入全局栈**：由触发元素直接管理，使用 `deferred` + `anchored` 原语。
4. **Context Menu 独立层**：与 Popover 分离，右键触发，跟随光标位置。
5. **aux_windows 保留**：SSH 配置文件编辑器、剪贴板设置等独立窗口保持现状。

### LayerManager

```rust
pub struct LayerManager {
    sheets: Vec<ActiveSheet>,
    dialogs: Vec<ActiveDialog>,
    notifications: NotificationList,
}

impl Global for LayerManager;
```

### Dialog API

```rust
window.open_dialog(cx, |dialog, window, cx| {
    dialog.title("确认删除")
        .content(...)
        .primary_button("删除", |_, _, cx| { true })
        .secondary_button("取消", |_, _, cx| { true })
        .overlay(true)
        .overlay_closable(true)
})
```

### Sheet API

```rust
window.open_sheet(cx, Placement::Right, |sheet, window, cx| {
    sheet.title("设置").size(px(400.)).content(...)
})
```

### Notification API

```rust
window.push_notification(
    Notification::success("保存成功").auto_hide(Duration::seconds(3)),
    cx,
);
```

### 文件结构

```
qingqi-ui/src/layer/
  mod.rs                # LayerManager + Global
  dialog.rs             # Dialog + ActiveDialog
  sheet.rs              # Sheet + ActiveSheet
  context_menu.rs       # ContextMenuExt trait + PopupMenu
  notification.rs       # Notification + NotificationList
  popover.rs            # Popover + PopoverBuilder
  tooltip.rs            # Tooltip
```

---

## 4. 输入框（全功能）

### 架构：融合 gpui-component + Yororen UI

| 设计点 | 选择 | 来源 |
|--------|------|------|
| 状态分层 | TextInputCore + TextInputState（Deref 组合） | Yororen |
| Action 抽象 | TextInputActionHandler trait | Yororen |
| 文本存储 | String（非 Rope） | Yororen |
| 键盘动作 | 14 个核心动作 + secondary- 跨平台前缀 | Yororen |
| IME 处理 | EntityInputHandler + UTF-8/UTF-16 镜像 | 两者共有 |
| 光标闪烁 | cursor_blink_epoch 机制 | Yororen |
| 密码/数字 | 独立 PasswordInput / NumberInput | gpui-component |
| 验证 | pattern + validate + max_length | gpui-component |
| 回调 | on_change + on_submit | Yororen |

### TextInputCore（共享状态机）

```rust
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
```

### TextInputState

```rust
pub struct TextInputState {
    pub value: String,
    pub placeholder: SharedString,
    pub max_length: Option<usize>,
    pub on_change: Option<TextChangeCallback>,
    pub on_submit: Option<TextChangeCallback>,
    pub paste_newlines: bool,
    pub disabled: bool,
    pub masked: bool,
    pub pattern: Option<Regex>,
    pub validate: Option<Box<dyn Fn(&str) -> bool>>,
    pub core: TextInputCore,
}

impl Deref for TextInputState {
    type Target = TextInputCore;
    fn deref(&self) -> &TextInputCore { &self.core }
}
```

### TextInputActionHandler Trait

```rust
pub trait TextInputActionHandler: 'static {
    fn value(&self) -> String;
    fn backspace(&mut self, ...) {}
    fn delete(&mut self, ...) {}
    fn left(&mut self, ...) {}
    fn right(&mut self, ...) {}
    fn select_left(&mut self, ...) {}
    fn select_right(&mut self, ...) {}
    fn select_all(&mut self, ...) {}
    fn home(&mut self, ...) {}
    fn end(&mut self, ...) {}
    fn paste(&mut self, ...) {}
    fn copy(&mut self, ...) {}
    fn cut(&mut self, ...) {}
    fn enter(&mut self, ...) {}
    fn escape(&mut self, ...) {}
    fn show_character_palette(&mut self, ...) {}
    fn on_mouse_down(&mut self, ...) {}
    fn on_mouse_up(&mut self, ...) {}
    fn on_mouse_move(&mut self, ...) {}
}
```

### 键盘动作（14 个，跨平台）

使用 `secondary-` 前缀（Cmd on macOS, Ctrl on Linux/Windows）：

```
Backspace, Delete, Enter, Escape,
Left, Right, SelectLeft, SelectRight,
SelectAll, Home, End,
Paste, Cut, Copy, ShowCharacterPalette
```

### 功能清单

- 单行/多行输入
- 密码模式（遮罩 + 切换按钮）
- 数字输入
- 文本选择（鼠标拖拽 + 键盘）
- IME 合成（中文/日文/韩文输入）
- Undo/Redo
- 复制/粘贴/剪切（系统剪贴板）
- 验证（pattern + validate）
- 字符限制（max_length）
- 光标闪烁
- 自动聚焦
- 搜索面板（CodeEditor 模式）
- 自动增长行数（TextArea）

### qingqi TextInput 兼容层

```rust
pub struct TextInput {
    state: Entity<TextInputState>,
    style: TextInputStyle,
    // ... 保持现有字段
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>, value: impl Into<SharedString>) -> Self { ... }
    pub fn text(&self) -> String { ... }
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) { ... }
    pub fn set_multiline(&mut self, multiline: bool, cx: &mut Context<Self>) { ... }
    pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>, cx: &mut Context<Self>) { ... }
    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) { ... }
    pub fn set_monospace(&mut self, monospace: bool, cx: &mut Context<Self>) { ... }
    pub fn select_all_text(&mut self, cx: &mut Context<Self>) { ... }
    pub fn set_key_down_handler(...) { ... }
}
```

### 文件结构

```
qingqi-ui/src/components/input/
  mod.rs                    # 公共导出
  text_input_core.rs        # TextInputCore（共享状态机）
  text_input_state.rs       # TextInputState（Deref 到 Core）
  text_input_element.rs     # 渲染层
  action_handler.rs         # TextInputActionHandler trait + action_handler! macro
  keyboard.rs               # 14 个键盘动作 + init() 注册
  password_input.rs         # PasswordInput
  number_input.rs           # NumberInput
  text_area.rs              # TextArea（多行 + 自动增长）
```

---

## 5. Button 组件

### 变体系统

```rust
pub enum ButtonVariant {
    Primary, Secondary, Ghost, Text,
    Danger, Warning, Success, Info, Link,
}

pub enum ButtonSize {
    XSmall, Small, Medium, Large, XLarge,
}

pub enum ButtonShape {
    Rounded, Square, Circle, Pill,
}
```

### API

```rust
Button::new("submit")
    .variant(ButtonVariant::Primary)
    .size(ButtonSize::Medium)
    .label("提交")
    .on_click(|_, _, cx| { ... })

Button::new("close")
    .icon(IconName::Close)
    .variant(ButtonVariant::Ghost)
    .shape(ButtonShape::Circle)
```

### 文件结构

```
qingqi-ui/src/components/button/
  mod.rs, button.rs, variant.rs, size.rs, shape.rs, group.rs
```

---

## 6. Context Menu / Popover / Tooltip

### Context Menu

```rust
element.context_menu(cx, |menu, window, cx| {
    menu.item("复制").icon(IconName::Copy).on_click(...)
    menu.divider()
    menu.item("删除").variant(MenuItemVariant::Danger).on_click(...)
    menu.submenu("更多", |menu, _, _| { ... })
})
```

### Popover

```rust
Popover::new("id")
    .trigger(button)
    .anchor(Corner::BottomLeft)
    .content(|window, cx| { ... })
    .on_open_change(|is_open, _, _| { ... })
```

### Tooltip

```rust
element.tooltip("提示文字")
element.tooltip_with_side("提示", Side::Bottom)
```

---

## 7. 其他组件

| 组件 | 功能 |
|------|------|
| Icon | SVG 图标 + IconName 枚举 |
| Divider | 水平/垂直分割线，支持虚线和标签 |
| Switch | 开关，支持标签和 tooltip |
| Progress | 进度条，支持自定义颜色 |
| Badge | 状态标记 |
| Label | 文本标签，支持多种尺寸和颜色 |

---

## 8. 迁移计划

### Phase 1: Foundation
- Token 系统 + LayerManager 骨架
- 让 ui/mod.rs 脱离 gpui_component::Theme

### Phase 2: Core Components
- Button + Input + Icon + Label + Divider + Switch
- 覆盖 80% 使用场景

### Phase 3: Overlay System
- Dialog + Sheet + Notification
- 替代现有 overlay_host

### Phase 4: Menu System
- ContextMenu + Popover + Tooltip
- SSH/API Debugger 的核心交互

### Phase 5: Migration
- 逐个 feature crate 迁移，移除 gpui-component 依赖

### Phase 6: Cleanup
- 移除 vendor/gpui-component + workspace Cargo.toml patch

---

## 9. 文件结构总览

```
qingqi-ui/src/
  token.rs                  # Token 系统
  theme.rs                  # 保留现有（不变）
  layer/                    # 分层管理
    mod.rs, dialog.rs, sheet.rs, context_menu.rs,
    notification.rs, popover.rs, tooltip.rs
  components/               # 可复用组件
    button/                 # Button + variants
    input/                  # Input (全功能)
    icon/                   # Icon + IconName
    divider.rs, switch.rs, progress.rs, badge.rs, label.rs
  ui/                       # 逐步废弃 gpui_component::Theme 依赖
    mod.rs, glass.rs, window_chrome.rs, components/
```

---

## 10. 依赖变更

### 移除
```
gpui-component = "0.5.1"     # workspace Cargo.toml
[patch.crates-io]            # workspace Cargo.toml
vendor/gpui-component/       # 目录
```

### 保留
```
gpui = { version = "=0.2.2", features = ["runtime_shaders"] }
regex = "1.12"               # 输入验证
unicode-segmentation = "1.12" # 单词边界
```

### 新增
无 — 所有功能基于 GPUI 原生能力实现。

### 注意事项
- `theme.rs` 中的 `rgba_with_alpha` 函数被 `glass.rs` 使用，迁移时保留或移入 Token 系统
- `glass.rs` 中的毛玻璃效果依赖 `gpui_component::StyledExt`，需替换为等效实现
- `window_chrome.rs` 中的 `Icon/IconName` 导入需替换为自研 Icon 组件
- `settings.rs` 中的 `GroupBox` 需替换为自研容器组件
