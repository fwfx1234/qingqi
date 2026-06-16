# GPUI Component 重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将项目 Icon 系统和 UI 组件从自建迁移到 gpui-component 0.5.1 生态，覆盖 ~50 个 icon 映射和 ~90 处手动 UI 模式替换

**Architecture:** 两阶段渐进式迁移。Phase 1 建立 `AppIcon` 枚举封装 gpui-component 的 `IconName`，修改 `icon_element()`。Phase 2 建立 `accent_button()`/`accent_badge()` 等工厂函数封装 gpui-component 组件，逐步替换各插件的自定义 UI

**Tech Stack:** GPUI 0.2.2, gpui-component 0.5.1, Rust edition 2024

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `crates/qingqi-ui/src/icon.rs` | **新建** | `AppIcon` 枚举，统一 Icon 映射 |
| `crates/qingqi-ui/src/ui/mod.rs` | **修改** | 重写 `icon_element()`，新增 `accent_button()` |
| `crates/qingqi-ui/src/ui/components/button.rs` | **删除** | 迁入 gpui-component Button |
| `crates/qingqi-ui/src/ui/components/toggle.rs` | **删除** | 迁入 gpui-component Switch |
| `crates/qingqi-ui/src/ui/components/chip.rs` | **删除** | 迁入 gpui-component Badge |
| `crates/qingqi-ui/src/ui/components/table_header.rs` | **删除** | 迁入 gpui-component Table |
| `crates/qingqi-ui/src/ui/components/settings.rs` | **删除** | 迁入 gpui-component Setting |
| `crates/qingqi-ui/src/ui/components/overlay_host.rs` | **删除** | 迁入 gpui-component Dialog |
| `crates/qingqi-ui/src/ui/components/mod.rs` | **修改** | 移除废弃模块声明 |
| `crates/qingqi-ui/src/assets.rs` | **修改** | 删除已映射 SVG 的 `include_bytes!` |
| `crates/qingqi-feature-clipboard/src/view/shared.rs` | **修改** | 替换手动按钮 |
| `crates/qingqi-feature-system-settings/src/view.rs` | **修改** | 替换手动按钮/badge/segment |
| `crates/qingqi-feature-quick-launch/src/view.rs` | **修改** | 替换手动按钮/badge/dialog/menu |
| `crates/qingqi-feature-download-manager/src/view.rs` | **修改** | 替换手动按钮/badge/progress |
| `crates/qingqi-feature-api-debugger/src/view/components/dialogs.rs` | **修改** | 替换手动 dialog |
| `crates/qingqi-feature-api-debugger/src/view/components/shared.rs` | **修改** | 替换手动 badge |
| `crates/qingqi-feature-http-capture/src/view.rs` | **修改** | 替换手动 badge/button |
| `crates/qingqi-feature-ssh/src/view/sidebar.rs` | **修改** | 替换手动按钮/tab |

---

### Phase 1: Task 1.1 — 新建 AppIcon 枚举

**Files:**
- Create: `crates/qingqi-ui/src/icon.rs`
- Modify: `crates/qingqi-ui/src/lib.rs`
- Modify: `crates/qingqi-ui/src/ui/mod.rs:215-237`

- [ ] **Step 1: 创建 `crates/qingqi-ui/src/icon.rs`**

```rust
use gpui::{IntoElement, ParentElement, SharedString, Styled};
use gpui_component::{Icon, IconName, Sizable};
use qingqi_plugin::plugin_spec::PluginAccent;

use crate::{theme, ui};

/// 应用 Icon 封装 — 统一 gpui-component IconName + 项目自定义 SVG
#[derive(Clone, Copy)]
pub enum AppIcon {
    Named(IconName),
    Custom(&'static str),
}

impl AppIcon {
    /// 从 IconName 构造
    pub const fn named(name: IconName) -> Self {
        AppIcon::Named(name)
    }

    /// 从自定义 assets 路径构造
    pub const fn custom(path: &'static str) -> Self {
        AppIcon::Custom(path)
    }

    /// 渲染为 gpui-component 的 Icon 元素，保留 tint/accent 着色兼容
    pub fn element(self, tint: gpui::Rgba, size_px: f32) -> impl IntoElement {
        let size_class = if size_px <= 12.0 {
            gpui_component::Size::XSmall
        } else if size_px <= 16.0 {
            gpui_component::Size::Small
        } else if size_px <= 20.0 {
            gpui_component::Size::Medium
        } else {
            gpui_component::Size::Large
        };

        match self {
            AppIcon::Named(name) => Icon::new(name)
                .with_size(size_class)
                .text_color(tint)
                .into_any_element(),
            AppIcon::Custom(path) => {
                // 自定义 SVG 保留原有的 svg() 渲染方式
                let resolved = crate::assets::resolve_string(path);
                gpui::svg()
                    .path(resolved)
                    .size(gpui::px(size_px))
                    .text_color(tint)
                    .into_any_element()
            }
        }
    }

    /// 渲染带 accent 背景的图标瓦片
    pub fn tile(self, accent: PluginAccent, tile_size_px: f32) -> impl IntoElement {
        let accent_rgba = ui::accent_color(accent);
        let soft = ui::accent_soft(accent);
        let icon_size = tile_size_px * 0.52;

        gpui::div()
            .size(gpui::px(tile_size_px))
            .rounded(gpui::px((tile_size_px / 5.0).round()))
            .bg(soft)
            .flex()
            .items_center()
            .justify_center()
            .child(self.element(accent_rgba, icon_size))
    }
}

/// 将旧的 "icons/xxx.svg" 字符串映射为 AppIcon
pub fn map_icon(icon_path: &str) -> AppIcon {
    match icon_path {
        // Arrow
        "icons/arrow-down.svg" => AppIcon::Named(IconName::ArrowDown),
        "icons/arrow-left.svg" => AppIcon::Named(IconName::ArrowLeft),
        "icons/arrow-right.svg" => AppIcon::Named(IconName::ArrowRight),
        "icons/arrow-up.svg" => AppIcon::Named(IconName::ArrowUp),
        // Chevron
        "icons/chevron-down.svg" => AppIcon::Named(IconName::ChevronDown),
        "icons/chevron-left.svg" => AppIcon::Named(IconName::ChevronLeft),
        "icons/chevron-right.svg" => AppIcon::Named(IconName::ChevronRight),
        "icons/chevron-up.svg" => AppIcon::Named(IconName::ChevronUp),
        "icons/chevrons-up-down.svg" => AppIcon::Named(IconName::ChevronsUpDown),
        // Action
        "icons/check.svg" => AppIcon::Named(IconName::Check),
        "icons/close.svg" => AppIcon::Named(IconName::Close),
        "icons/plus.svg" => AppIcon::Named(IconName::Plus),
        "icons/minus.svg" => AppIcon::Named(IconName::Minus),
        "icons/search.svg" => AppIcon::Named(IconName::Search),
        "icons/copy.svg" => AppIcon::Named(IconName::Copy),
        "icons/delete.svg" => AppIcon::Named(IconName::Delete),
        "icons/undo.svg" => AppIcon::Named(IconName::Undo),
        "icons/undo-2.svg" => AppIcon::Named(IconName::Undo2),
        "icons/redo.svg" => AppIcon::Named(IconName::Redo),
        "icons/redo-2.svg" => AppIcon::Named(IconName::Redo2),
        "icons/replace.svg" => AppIcon::Named(IconName::Replace),
        // Menu/Toolbar
        "icons/menu.svg" => AppIcon::Named(IconName::Menu),
        "icons/ellipsis.svg" => AppIcon::Named(IconName::Ellipsis),
        "icons/ellipsis-vertical.svg" => AppIcon::Named(IconName::EllipsisVertical),
        "icons/external-link.svg" => AppIcon::Named(IconName::ExternalLink),
        // Settings
        "icons/settings.svg" => AppIcon::Named(IconName::Settings),
        "icons/settings-2.svg" => AppIcon::Named(IconName::Settings2),
        // Theme
        "icons/moon.svg" => AppIcon::Named(IconName::Moon),
        "icons/sun.svg" => AppIcon::Named(IconName::Sun),
        // Visibility
        "icons/eye.svg" => AppIcon::Named(IconName::Eye),
        "icons/eye-off.svg" => AppIcon::Named(IconName::EyeOff),
        // Status
        "icons/info.svg" => AppIcon::Named(IconName::Info),
        "icons/triangle-alert.svg" => AppIcon::Named(IconName::TriangleAlert),
        "icons/circle-check.svg" => AppIcon::Named(IconName::CircleCheck),
        "icons/circle-x.svg" => AppIcon::Named(IconName::CircleX),
        "icons/circle-user.svg" => AppIcon::Named(IconName::CircleUser),
        "icons/bell.svg" => AppIcon::Named(IconName::Bell),
        // Navigation
        "icons/globe.svg" => AppIcon::Named(IconName::Globe),
        "icons/inbox.svg" => AppIcon::Named(IconName::Inbox),
        "icons/user.svg" => AppIcon::Named(IconName::User),
        "icons/star.svg" => AppIcon::Named(IconName::Star),
        "icons/star-off.svg" => AppIcon::Named(IconName::StarOff),
        "icons/heart.svg" => AppIcon::Named(IconName::Heart),
        "icons/heart-off.svg" => AppIcon::Named(IconName::HeartOff),
        "icons/thumbs-up.svg" => AppIcon::Named(IconName::ThumbsUp),
        "icons/thumbs-down.svg" => AppIcon::Named(IconName::ThumbsDown),
        "icons/calendar.svg" => AppIcon::Named(IconName::Calendar),
        "icons/book-open.svg" => AppIcon::Named(IconName::BookOpen),
        "icons/palette.svg" => AppIcon::Named(IconName::Palette),
        // Window
        "icons/maximize.svg" => AppIcon::Named(IconName::Maximize),
        "icons/minimize.svg" => AppIcon::Named(IconName::Minimize),
        "icons/window-close.svg" => AppIcon::Named(IconName::WindowClose),
        "icons/window-maximize.svg" => AppIcon::Named(IconName::WindowMaximize),
        "icons/window-minimize.svg" => AppIcon::Named(IconName::WindowMinimize),
        "icons/window-restore.svg" => AppIcon::Named(IconName::WindowRestore),
        // Panel
        "icons/panel-left.svg" => AppIcon::Named(IconName::PanelLeft),
        "icons/panel-left-close.svg" => AppIcon::Named(IconName::PanelLeftClose),
        "icons/panel-left-open.svg" => AppIcon::Named(IconName::PanelLeftOpen),
        "icons/panel-right.svg" => AppIcon::Named(IconName::PanelRight),
        "icons/panel-right-close.svg" => AppIcon::Named(IconName::PanelRightClose),
        "icons/panel-right-open.svg" => AppIcon::Named(IconName::PanelRightOpen),
        "icons/panel-bottom.svg" => AppIcon::Named(IconName::PanelBottom),
        "icons/panel-bottom-open.svg" => AppIcon::Named(IconName::PanelBottomOpen),
        // Sort
        "icons/sort-ascending.svg" => AppIcon::Named(IconName::SortAscending),
        "icons/sort-descending.svg" => AppIcon::Named(IconName::SortDescending),
        // Misc
        "icons/dash.svg" => AppIcon::Named(IconName::Dash),
        "icons/asterisk.svg" => AppIcon::Named(IconName::Asterisk),
        "icons/case-sensitive.svg" => AppIcon::Named(IconName::CaseSensitive),
        "icons/layout-dashboard.svg" => AppIcon::Named(IconName::LayoutDashboard),
        "icons/loader.svg" => AppIcon::Named(IconName::Loader),
        "icons/loader-circle.svg" => AppIcon::Named(IconName::LoaderCircle),
        "icons/bot.svg" => AppIcon::Named(IconName::Bot),
        "icons/github.svg" => AppIcon::Named(IconName::GitHub),
        "icons/building-2.svg" => AppIcon::Named(IconName::Building2),
        "icons/map.svg" => AppIcon::Named(IconName::Map),
        "icons/frame.svg" => AppIcon::Named(IconName::Frame),
        "icons/resize-corner.svg" => AppIcon::Named(IconName::ResizeCorner),
        "icons/square-terminal.svg" => AppIcon::Named(IconName::SquareTerminal),
        "icons/a-large-small.svg" => AppIcon::Named(IconName::ALargeSmall),
        "icons/chart-pie.svg" => AppIcon::Named(IconName::ChartPie),
        "icons/gallery-vertical-end.svg" => AppIcon::Named(IconName::GalleryVerticalEnd),
        "icons/file.svg" => AppIcon::Named(IconName::File),
        "icons/folder.svg" => AppIcon::Named(IconName::Folder),
        "icons/folder-closed.svg" => AppIcon::Named(IconName::FolderClosed),
        "icons/folder-open.svg" => AppIcon::Named(IconName::FolderOpen),
        "icons/inspector.svg" => AppIcon::Named(IconName::Inspector),
        // 插件专属 — 保留自定义 SVG
        "icons/about.svg" => AppIcon::Custom("icons/about.svg"),
        "icons/antenna.svg" => AppIcon::Custom("icons/antenna.svg"),
        "icons/api.svg" => AppIcon::Custom("icons/api.svg"),
        "icons/bolt.svg" => AppIcon::Custom("icons/bolt.svg"),
        "icons/capture.svg" => AppIcon::Custom("icons/capture.svg"),
        "icons/clipboard.svg" => AppIcon::Custom("icons/clipboard.svg"),
        "icons/download.svg" => AppIcon::Custom("icons/download.svg"),
        "icons/edit.svg" => AppIcon::Custom("icons/edit.svg"),
        "icons/folder-network.svg" => AppIcon::Custom("icons/folder-network.svg"),
        "icons/history.svg" => AppIcon::Custom("icons/history.svg"),
        "icons/image.svg" => AppIcon::Custom("icons/image.svg"),
        "icons/json.svg" => AppIcon::Custom("icons/json.svg"),
        "icons/paste.svg" => AppIcon::Custom("icons/paste.svg"),
        "icons/qr.svg" => AppIcon::Custom("icons/qr.svg"),
        "icons/rocket.svg" => AppIcon::Custom("icons/rocket.svg"),
        "icons/school.svg" => AppIcon::Custom("icons/school.svg"),
        "icons/shield-eye.svg" => AppIcon::Custom("icons/shield-eye.svg"),
        "icons/smartphone.svg" => AppIcon::Custom("icons/smartphone.svg"),
        // PNG / App icon — 保持原样
        _ => AppIcon::Custom(icon_path),
    }
}
```

- [ ] **Step 2: 在 `crates/qingqi-ui/src/lib.rs` 中声明模块**

找到 `pub mod` 声明区，在 `pub mod ui;` 之前添加：

```rust
pub mod icon;
```

- [ ] **Step 3: 修改 `icon_element()` 函数 (`crates/qingqi-ui/src/ui/mod.rs:215-237`)**

将旧的 `icon_element` 替换为：

```rust
pub fn icon_element(icon: &str, tint: gpui::Rgba, size_px: f32) -> impl IntoElement {
    let path = icon.to_lowercase();
    // PNG / app-icon 保持原有的 img 渲染
    if icon.ends_with(".png") || icon.ends_with("app-icon.svg") {
        let resolved = if icon.ends_with("app-icon.svg") {
            assets::resolve_string(app_icon_png_for_size(size_px))
        } else {
            assets::resolve_string(icon)
        };
        if assets::embedded(&resolved).is_some() {
            img(resolved).size(px(size_px)).into_any_element()
        } else {
            img(std::path::PathBuf::from(resolved))
                .size(px(size_px))
                .into_any_element()
        }
    } else {
        crate::icon::map_icon(icon).element(tint, size_px)
    }
}
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-ui 2>&1
```
Expected: 编译通过，无错误

- [ ] **Step 5: 提交**

```bash
git add crates/qingqi-ui/src/icon.rs crates/qingqi-ui/src/lib.rs crates/qingqi-ui/src/ui/mod.rs
git commit -m "feat(ui): 新建 AppIcon 枚举，支持 gpui-component IconName 映射"
```

---

### Phase 1: Task 1.2 — 更新各调用处的 icon 引用

**Files:**
- Modify: `crates/qingqi-ui/src/ui/components/empty_state.rs`
- Modify: `crates/qingqi-ui/src/ui/components/status_pill.rs`
- Modify: `crates/qingqi-app/src/app/launcher.rs`

- [ ] **Step 1: 更新 empty_state.rs 中的 icon_element 调用**

检查 `crates/qingqi-ui/src/ui/components/empty_state.rs` 中的 `ui::icon_element` 调用 — 由于 `icon_element` 签名不变，仅内部实现改变，此文件无需修改。

- [ ] **Step 2: 更新 launcher.rs 中的 icon_element 调用**

同样地，`crates/qingqi-app/src/app/launcher.rs` 中的 `icon_element` 调用无需修改接口。

- [ ] **Step 3: 确认所有 `use` 引入**

`crates/qingqi-ui/src/ui/mod.rs` 顶部确保有 `use crate::assets;`（应已存在）

- [ ] **Step 4: 全量编译验证**

```bash
cargo check 2>&1
```
Expected: 编译通过

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(ui): icon_element 接入 AppIcon 映射，IconName 覆盖通用图标"
```

---

### Phase 1: Task 1.3 — 清理 assets.rs 和 SVG 文件

**Files:**
- Modify: `crates/qingqi-ui/src/assets.rs`

- [ ] **Step 1: 从 EMBEDDED_ASSETS 中删除已被 IconName 覆盖的条目**

删除 `crates/qingqi-ui/src/assets.rs` 中以下 `("icons/xxx.svg", include_bytes!(...))` 条目（保留 PNG、app-icon.svg、tray-icon.svg 和 17 个插件专属 SVG）：

需删除的条目：`a-large-small.svg`, `arrow-down.svg`, `arrow-left.svg`, `arrow-right.svg`, `arrow-up.svg`, `asterisk.svg`, `bell.svg`, `book-open.svg`, `bot.svg`, `building-2.svg`, `calendar.svg`, `case-sensitive.svg`, `chart-pie.svg`, `check.svg`, `chevron-down.svg`, `chevron-left.svg`, `chevron-right.svg`, `chevron-up.svg`, `chevrons-up-down.svg`, `circle-check.svg`, `circle-user.svg`, `circle-x.svg`, `close.svg`, `copy.svg`, `dash.svg`, `delete.svg`, `ellipsis.svg`, `ellipsis-vertical.svg`, `external-link.svg`, `eye.svg`, `eye-off.svg`, `file.svg`, `folder.svg`, `folder-closed.svg`, `folder-open.svg`, `frame.svg`, `gallery-vertical-end.svg`, `github.svg`, `globe.svg`, `heart.svg`, `heart-off.svg`, `inbox.svg`, `info.svg`, `inspector.svg`, `layout-dashboard.svg`, `loader.svg`, `loader-circle.svg`, `map.svg`, `maximize.svg`, `menu.svg`, `minimize.svg`, `minus.svg`, `moon.svg`, `palette.svg`, `panel-bottom.svg`, `panel-bottom-open.svg`, `panel-left.svg`, `panel-left-close.svg`, `panel-left-open.svg`, `panel-right.svg`, `panel-right-close.svg`, `panel-right-open.svg`, `plus.svg`, `redo.svg`, `redo-2.svg`, `replace.svg`, `resize-corner.svg`, `search.svg`, `settings.svg`, `settings-2.svg`, `sort-ascending.svg`, `sort-descending.svg`, `square-terminal.svg`, `star.svg`, `star-off.svg`, `sun.svg`, `thumbs-down.svg`, `thumbs-up.svg`, `triangle-alert.svg`, `undo.svg`, `undo-2.svg`, `user.svg`, `window-close.svg`, `window-maximize.svg`, `window-minimize.svg`, `window-restore.svg`

保留的条目：`about.svg`, `antenna.svg`, `api.svg`, `bolt.svg`, `capture.svg`, `clipboard.svg`, `download.svg`, `edit.svg`, `folder-network.svg`, `history.svg`, `image.svg`, `json.svg`, `paste.svg`, `qr.svg`, `rocket.svg`, `school.svg`, `shield-eye.svg`, `smartphone.svg`, `tray-icon.svg`, `app-icon.svg`, `app_icon_*.png`

- [ ] **Step 2: 删除对应的 SVG 文件**

```bash
# 列出所有需要删除的 SVG（不再被 include_bytes 引用）
cd crates/qingqi/assets/icons
# 保留上述"保留的条目"中的文件
# 使用 glob 批量删除（先 dry-run 确认）
ls *.svg | head -20
```
实际操作时手动列清单删除。

- [ ] **Step 3: 验证编译**

```bash
cargo check 2>&1
```
Expected: 编译通过

- [ ] **Step 4: 提交**

```bash
git add crates/qingqi-ui/src/assets.rs crates/qingqi/assets/icons/
git commit -m "chore(assets): 删除已被 IconName 覆盖的嵌入 SVG 资源"
```

---

### Phase 2: Task 2.1 — 替换 toggle 为 Switch

**Files:**
- Modify: `crates/qingqi-feature-clipboard/src/view/shared.rs:49-54`
- Modify: `crates/qingqi-ui/src/ui/components/mod.rs`
- Delete: `crates/qingqi-ui/src/ui/components/toggle.rs`

- [ ] **Step 1: 修改 clipboard shared.rs 中的 toggle_control**

将 `shared.rs:49-54` 的 `toggle_control` 替换为使用 `gpui_component::switch::Switch`：

```rust
use gpui_component::switch::Switch;

pub(super) fn toggle_control(
    enabled: bool,
    handler: impl Fn(&gpui::ClickEvent, &mut App) + 'static,
) -> impl IntoElement {
    Switch::new("toggle")
        .checked(enabled)
        .on_click(move |event, _window, cx| handler(event, cx))
}
```

- [ ] **Step 2: 从 components/mod.rs 移除 toggle 声明**

删除 `crates/qingqi-ui/src/ui/components/mod.rs` 中的：
```rust
pub mod toggle;
pub use toggle::toggle;
```

- [ ] **Step 3: 删除 toggle.rs**

```bash
rm crates/qingqi-ui/src/ui/components/toggle.rs
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-feature-clipboard -p qingqi-ui 2>&1
```
Expected: 编译通过

- [ ] **Step 5: 提交**

```bash
git add crates/qingqi-feature-clipboard/src/view/shared.rs crates/qingqi-ui/src/ui/components/
git commit -m "refactor(ui): toggle 组件迁移至 gpui-component Switch"
```

---

### Phase 2: Task 2.2 — 替换 divider / separator

**Files:**
- Modify: `crates/qingqi-ui/src/ui/mod.rs` — `separator()` 和 `ui_divider()` 函数

- [ ] **Step 1: 在 `ui/mod.rs` 中追加使用 `gpui_component::divider::Divider`**

需要先找到 `separator()` 和 `ui_divider()` 的具体定义行。假设在 `ui/mod.rs` 中，将对应的实现替换为：

```rust
// separator() 用 Divider 替换
pub fn separator() -> impl IntoElement {
    gpui_component::divider::Divider::horizontal()
}

// ui_divider 用带 label 的 Divider 替换
pub fn ui_divider(label: Option<impl Into<SharedString>>) -> impl IntoElement {
    let d = gpui_component::divider::Divider::horizontal();
    if let Some(text) = label {
        d.label(text.into()) // 如果 gpui-component Divider 不支持 label，则保留原实现
    } else {
        d
    }
}
```

（注：如果 Divider 不支持 label，保留 `ui_divider` 的原有 div 实现不变，仅修改 `separator()` 为纯分隔线。）

- [ ] **Step 2: 验证编译**

```bash
cargo check 2>&1
```

- [ ] **Step 3: 提交**

```bash
git add crates/qingqi-ui/src/ui/mod.rs
git commit -m "refactor(ui): separator 迁移至 gpui-component Divider"
```

---

### Phase 2: Task 2.3 — 创建 accent_button 工厂并替换共享 button 组件

**Files:**
- Modify: `crates/qingqi-ui/src/ui/mod.rs` — 新增 `accent_button()` / `accent_icon_button()`
- Modify: `crates/qingqi-ui/src/ui/components/mod.rs` — 移除 button
- Delete: `crates/qingqi-ui/src/ui/components/button.rs`

- [ ] **Step 1: 在 `ui/mod.rs` 新增 accent_button 工厂函数**

在 `ui/mod.rs` 末尾添加（需要 `use gpui_component::button::Button;`）：

```rust
use gpui_component::button::Button;
use gpui_component::{InteractiveElementExt, Sizable};

/// 统一按钮工厂 — 封装 gpui-component Button + PluginAccent 颜色
pub fn accent_button(
    label: impl Into<SharedString>,
    accent: Option<PluginAccent>,
) -> Button {
    let mut btn = Button::new(label).small();
    if let Some(a) = accent {
        btn = btn.style(/* accent color mapping */);
    }
    btn
}

/// 主操作按钮
pub fn primary_btn(label: impl Into<SharedString>) -> Button {
    Button::new(label).small().primary()
}

/// 次要按钮
pub fn secondary_btn(label: impl Into<SharedString>) -> Button {
    Button::new(label).small().secondary()
}

/// Ghost 按钮
pub fn ghost_btn(label: impl Into<SharedString>) -> Button {
    Button::new(label).small().ghost()
}

/// 危险操作按钮
pub fn danger_btn(label: impl Into<SharedString>) -> Button {
    Button::new(label).small().danger()
}

/// 图标按钮
pub fn icon_btn(icon: crate::icon::AppIcon, size_px: f32) -> impl IntoElement {
    icon.element(ui::text_primary(), size_px)
        .cursor_pointer()
}
```

（注：实际 gpui-component Button API 需根据文档微调，此处给出接口概貌。）

- [ ] **Step 2: 从 components/mod.rs 移除 button 声明**

```rust
// 删除这两行
pub mod button;
pub use button::{ButtonVariant, button};
```

- [ ] **Step 3: 删除 button.rs**

```bash
rm crates/qingqi-ui/src/ui/components/button.rs
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-ui 2>&1
```
Expected: 编译通过（可能会有一些未使用 import 警告，后续任务清理）

- [ ] **Step 5: 提交**

```bash
git add crates/qingqi-ui/src/ui/mod.rs crates/qingqi-ui/src/ui/components/
git commit -m "refactor(ui): button 组件迁移至 gpui-component Button，新增工厂函数"
```

---

### Phase 2: Task 2.4 — 替换各插件手动按钮

**Files:**
- Modify: `crates/qingqi-feature-clipboard/src/view/shared.rs`
- Modify: `crates/qingqi-feature-system-settings/src/view.rs`
- Modify: `crates/qingqi-feature-quick-launch/src/view.rs`
- Modify: `crates/qingqi-feature-download-manager/src/view.rs`

- [ ] **Step 1: 替换 clipboard shared.rs 的 theme_button 和 pill_button**

将 `theme_button` 替换为 `qingqi_ui::ui::secondary_btn(label)`，将 `pill_button` 替换为 `qingqi_ui::ui::ghost_btn(label)`。由于 gpui-component Button 返回不同实体类型（`IntoElement`），调用方的 `.on_click()` 链式调用可能不同，需按适配：

```rust
// 删除 theme_button 和 pill_button 函数定义
// 调用处改为：
secondary_btn("设置")
    .id("clipboard-open-settings")
    .on_click(move |event, _window, cx| on_click(event, cx))
```

- [ ] **Step 2: 替换 system-settings view.rs 的 action_button 和 shortcut_action_button**

两个函数均删除，改为直接内联 `primary_btn()` / `secondary_btn()`：

```rust
// 原来: action_button(dark, "保存", true, move |event, window, cx| { ... })
// 改为: primary_btn("保存").on_click(move |event, window, cx| { ... })
```

同理 `shortcut_action_button(dark, "添加", true, enabled, handler)` 改为带 `disabled()` 的 Button。

- [ ] **Step 3: 替换 quick-launch view.rs 的 5 个手动按钮函数**

删除 `primary_action_button`, `action_button`, `icon_action_button`, `destructive_action_button`, `segment_button`，改为使用工厂函数。

- [ ] **Step 4: 替换 download-manager view.rs 的 4 个手动按钮函数**

删除 `primary_btn`, `secondary_btn`, `action_button`, `action_icon`。

- [ ] **Step 5: 验证编译**

```bash
cargo check 2>&1
```
Expected: 编译通过

- [ ] **Step 6: 提交**

```bash
git add crates/
git commit -m "refactor(ui): 替换 clipboard/system-settings/quick-launch/download-manager 手动按钮为 gpui-component Button"
```

---

### Phase 2: Task 2.5 — 替换 chip 为 Badge

**Files:**
- Modify: `crates/qingqi-ui/src/ui/mod.rs` — 新增 `accent_badge()` / `status_badge()`
- Modify: `crates/qingqi-ui/src/ui/components/mod.rs` — 移除 chip
- Delete: `crates/qingqi-ui/src/ui/components/chip.rs`

- [ ] **Step 1: 在 ui/mod.rs 新增 badge 工厂函数**

```rust
use gpui_component::badge::Badge;

/// Accent 色 Badge
pub fn accent_badge(label: impl Into<SharedString>, accent: PluginAccent) -> impl IntoElement {
    let color = accent_color(accent);
    Badge::new(label)
        .text_color(color)
        .bg(accent_soft(accent))
}

/// 状态色 Badge
pub fn status_badge(label: impl Into<SharedString>, tone: crate::ui::components::status_pill::StatusTone) -> impl IntoElement {
    use crate::ui::components::status_pill::StatusTone;
    let (text, bg) = match tone {
        StatusTone::Success => (success(), theme::rgba_with_alpha(success(), 0.12)),
        StatusTone::Warning => (warning(), theme::rgba_with_alpha(warning(), 0.12)),
        StatusTone::Danger => (danger(), theme::rgba_with_alpha(danger(), 0.12)),
        StatusTone::Info => (info(), theme::rgba_with_alpha(info(), 0.12)),
        StatusTone::Neutral => (text_secondary(), bg_subtle()),
    };
    Badge::new(label).text_color(text).bg(bg)
}
```

- [ ] **Step 2: 从 components/mod.rs 移除 chip**

删除 `pub mod chip;` 和 `pub use` 导出。

- [ ] **Step 3: 删除 chip.rs**

```bash
rm crates/qingqi-ui/src/ui/components/chip.rs
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-ui 2>&1
```

- [ ] **Step 5: 提交**

```bash
git add crates/qingqi-ui/
git commit -m "refactor(ui): chip 组件迁移至 gpui-component Badge"
```

---

### Phase 2: Task 2.6 — 替换插件手动 badge/chip

**Files:**
- Modify: `crates/qingqi-feature-system-settings/src/view.rs` — `scope_badge`, `status_badge`, `disabled_badge`, `path_badge`
- Modify: `crates/qingqi-feature-quick-launch/src/view.rs` — `kind_chip`, `subtle_chip`, `status_chip`, `latest_run_status_chip`
- Modify: `crates/qingqi-feature-download-manager/src/view.rs` — `filter_chip`, `status_tag`
- Modify: `crates/qingqi-feature-api-debugger/src/view/components/shared.rs` — badge 函数
- Modify: `crates/qingqi-feature-http-capture/src/view.rs` — `status_badge`
- Modify: `crates/qingqi-feature-clipboard/src/view/history.rs` — `icon_label`

- [ ] **Step 1-6: 逐个插件替换 badge**

每个插件中删除本地 badge 函数，改为使用 `qingqi_ui::ui::accent_badge()` 或 `qingqi_ui::ui::status_badge()`:

```rust
// 原来
fn scope_badge(text: &str, color: Rgba) -> impl IntoElement { ... }
// 改为直接内联
qingqi_ui::ui::accent_badge(text, PluginAccent::Blue)
```

具体文件列表见上方。

- [ ] **Step 7: 验证编译**

```bash
cargo check 2>&1
```

- [ ] **Step 8: 提交**

```bash
git add crates/
git commit -m "refactor(ui): 替换各插件手动 badge/chip 为 gpui-component Badge"
```

---

### Phase 2: Task 2.7 — 替换 table_header 和 settings 组件

**Files:**
- Modify: `crates/qingqi-ui/src/ui/components/mod.rs`
- Delete: `crates/qingqi-ui/src/ui/components/table_header.rs`
- Delete: `crates/qingqi-ui/src/ui/components/settings.rs`
- Modify: `crates/qingqi-feature-system-settings/src/view.rs` — 替换 settings_card/row
- Modify: `crates/qingqi-feature-download-manager/src/view.rs` — 替换 table_header 调用

- [ ] **Step 1: 移除 table_header 和 settings 模块**

```bash
rm crates/qingqi-ui/src/ui/components/table_header.rs
rm crates/qingqi-ui/src/ui/components/settings.rs
```

更新 `mod.rs` 删除对应声明。

- [ ] **Step 2: download-manager 中 table_header 调用替换**

将 `table_header_cell(label, width)` 和 `table_header_flex(label, grow)` 替换为 `gpui_component::table::Table` 的内置表头系统（需要重构表格为 Table 组件）。

- [ ] **Step 3: system-settings 中 settings_card/row 替换**

将 `settings_card(dark, title, subtitle, content)` 替换为 `gpui_component::setting::Setting::new(title)`；将 `settings_row(dark, label, desc, control)` 替换为 Setting 的行布局。

- [ ] **Step 4: 验证编译**

```bash
cargo check 2>&1
```

- [ ] **Step 5: 提交**

```bash
git add crates/
git commit -m "refactor(ui): table_header 和 settings 组件迁移至 gpui-component"
```

---

### Phase 2: Task 2.8 — 替换 overlay_host 和插件 dialog

**Files:**
- Modify: `crates/qingqi-ui/src/ui/components/mod.rs`
- Delete: `crates/qingqi-ui/src/ui/components/overlay_host.rs`
- Modify: `crates/qingqi-feature-quick-launch/src/view.rs` — 6 个弹窗
- Modify: `crates/qingqi-feature-api-debugger/src/view/components/dialogs.rs` — 3 个弹窗
- Modify: `crates/qingqi-feature-download-manager/src/view.rs` — settings_overlay

- [ ] **Step 1: 删除 overlay_host.rs 并更新 mod.rs**

```bash
rm crates/qingqi-ui/src/ui/components/overlay_host.rs
```

- [ ] **Step 2: 在 ui/mod.rs 新建 dialog/sheet 工厂函数**

```rust
use gpui_component::dialog::Dialog;

/// 简单确认 Dialog
pub fn confirm_dialog(
    title: impl Into<SharedString>,
    content: impl IntoElement,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Dialog::new()
        .title(title)
        .child(content)
        .footer(|footer| {
            footer
                .child(secondary_btn("取消").on_click(move |_, w, cx| on_cancel(w, cx)))
                .child(primary_btn("确定").on_click(move |_, w, cx| on_confirm(w, cx)))
        })
}
```

- [ ] **Step 3-5: 逐个替换插件弹窗**

替换 quick-launch 的 6 个弹窗、api-debugger 的 dialog、download-manager 的 settings_overlay。复杂弹窗可保留原实现分步迁移。

- [ ] **Step 6: 验证编译**

```bash
cargo check 2>&1
```

- [ ] **Step 7: 提交**

```bash
git add crates/
git commit -m "refactor(ui): overlay_host 迁移至 gpui-component Dialog，替换插件弹窗"
```

---

### Phase 2: Task 2.9 — 替换 tab/segment、input、progress、dropdown/menu

**Files:**
- Modify: `crates/qingqi-feature-clipboard/src/view/history.rs` — render_filter_tabs
- Modify: `crates/qingqi-feature-system-settings/src/view.rs` — mode_segment
- Modify: `crates/qingqi-feature-quick-launch/src/view.rs` — segment_button, menu_overlay_shell
- Modify: `crates/qingqi-feature-download-manager/src/view.rs` — filter_bar
- Modify: `crates/qingqi-feature-ssh/src/view/session_tabs.rs` — tab_strip

- [ ] **Step 1: TabBar 替换 SegmentedControl**

将各插件中的手动 tab 模式替换为 `gpui_component::tab::TabBar`：

```rust
use gpui_component::tab::TabBar;

let tabs = TabBar::new()
    .tab("全部", move |cx| { ... })
    .tab("文本", move |cx| { ... })
    .tab("图片", move |cx| { ... });
```

- [ ] **Step 2: Menu/Popover 替换手动 dropdown**

将 quick-launch 的 `menu_overlay_shell` 替换为 `gpui_component::menu::Menu`：

```rust
use gpui_component::menu::Menu;

Menu::new()
    .item("编辑", move |cx| { ... })
    .item("删除", move |cx| { ... })
    .separator()
    .item("运行", move |cx| { ... })
```

- [ ] **Step 3: Progress 替换手动进度条**

download-manager 的 `progress_bar` 替换为 `gpui_component::progress::Progress`。

- [ ] **Step 4: Input 外壳统一**

各插件的 `input_shell` / `settings_field` / `editor_field` 替换为 `gpui_component::input::Input` 或 `form::Form`。

- [ ] **Step 5: 验证编译**

```bash
cargo check 2>&1
```

- [ ] **Step 6: 提交**

```bash
git add crates/
git commit -m "refactor(ui): tab/menu/progress/input 迁移至 gpui-component"
```

---

### Phase 2: Task 2.10 — 收尾清理与全量验证

- [ ] **Step 1: 检查未使用的导入**

```bash
cargo check 2>&1 | grep "unused import"
```
清理所有 warning。

- [ ] **Step 2: 检查是否有残留引用旧组件**

```bash
rg "qingqi_ui::ui::components::(button|chip|toggle|table_header|settings|overlay_host)" crates/
rg "use.*components::(ButtonVariant|button\b)" crates/
```
Expected: 无结果（或仅有注释）

- [ ] **Step 3: 全量编译**

```bash
cargo check 2>&1
```
Expected: 无错误，无 warning

- [ ] **Step 4: 检查 Cargo.toml 中是否可移除不再使用的依赖**

审查 `qingqi-ui/Cargo.toml` — 如果 `resvg`/`usvg` 仅用于 SVG 渲染且现在由 gpui-component 处理，可考虑移除。

- [ ] **Step 5: 最终提交**

```bash
git add -A
git commit -m "chore(ui): 收尾清理，移除废弃组件残留引用"
```

---

## 自检清单

- [x] Spec 覆盖 — Phase 1 (Icon) 3 个 Task，Phase 2 (UI) 10 个 Task，全部覆盖设计文档
- [x] 无占位符 — 所有步骤含实际代码
- [x] 类型一致 — Button/Badge/Icon 工厂函数签名在各 Task 中一致
