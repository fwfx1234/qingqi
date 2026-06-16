# GPUI Component 重构设计

**日期**: 2026-06-16
**范围**: Icon 系统迁移 + UI 组件统一，数据库层保持不变
**策略**: 渐进式分阶段迁移

---

## 一、概述

将项目从自建 UI 组件体系迁移到 `gpui-component (0.5.1)` 生态，包括：

- **Icon 系统**: 用 `gpui_component::IconName` (86 个内置 Lucide 图标) 替换项目中可对应的自定义 SVG 图标
- **UI 组件**: 用 gpui-component 的 `Button`, `Switch`, `Badge`, `Dialog`, `Tab`, `Menu`, `Table`, `Progress` 等替换遍布各插件的 ~80 处手动 UI 模式

不纳入范围：插件 manifest 图标（`IconRef::asset`）、数据库/数据层、`qingqi-ui` 中的布局辅助函数（`section_card`, `page_title` 等）。

---

## 二、Phase 1: Icon 迁移（~0.5 天）

### 2.1 映射策略

当前项目 63 个 SVG icon 中约 50 个是通用 UI 操作图标，可映射到 `IconName`：

| 当前图标 | IconName |
|---------|----------|
| `check.svg` | `IconName::Check` |
| `close.svg` | `IconName::Close` |
| `search.svg` | `IconName::Search` |
| `chevron-down/left/right/up.svg` | `IconName::ChevronDown/Left/Right/Up` |
| `arrow-down/left/right/up.svg` | `IconName::ArrowDown/Left/Right/Up` |
| `plus.svg` | `IconName::Plus` |
| `minus.svg` | `IconName::Minus` |
| `settings.svg` / `settings-2.svg` | `IconName::Settings` / `Settings2` |
| `moon.svg` / `sun.svg` | `IconName::Moon` / `Sun` |
| `eye.svg` / `eye-off.svg` | `IconName::Eye` / `EyeOff` |
| `copy.svg` | `IconName::Copy` |
| `delete.svg` | `IconName::Delete` |
| `ellipsis.svg` / `ellipsis-vertical.svg` | `IconName::Ellipsis` / `EllipsisVertical` |
| `external-link.svg` | `IconName::ExternalLink` |
| `globe.svg` | `IconName::Globe` |
| `inbox.svg` | `IconName::Inbox` |
| `info.svg` | `IconName::Info` |
| `menu.svg` | `IconName::Menu` |
| `star.svg` / `star-off.svg` | `IconName::Star` / `StarOff` |
| `user.svg` | `IconName::User` |
| `triangle-alert.svg` | `IconName::TriangleAlert` |
| `circle-check/x/user.svg` | `IconName::CircleCheck/CircleX/CircleUser` |
| `book-open.svg` | `IconName::BookOpen` |
| `palette.svg`, `bell.svg`, `calendar.svg` | `IconName::Palette/Bell/Calendar` |
| `heart.svg` / `heart-off.svg` | `IconName::Heart` / `HeartOff` |
| `thumbs-up/down.svg` | `IconName::ThumbsUp/ThumbsDown` |
| `undo.svg` / `undo-2.svg` / `redo.svg` / `redo-2.svg` | `IconName::Undo/Undo2/Redo/Redo2` |
| `replace.svg` | `IconName::Replace` |
| `maximize.svg` / `minimize.svg` | `IconName::Maximize` / `Minimize` |
| `window-close/maximize/minimize/restore.svg` | `IconName::WindowClose/Maximize/Minimize/Restore` |
| `panel-left/right/bottom.svg` 及变体 | `IconName::Panel*` 系列 |
| `sort-ascending/descending.svg` | `IconName::SortAscending/SortDescending` |
| `gallery-vertical-end.svg` | `IconName::GalleryVerticalEnd` |
| `dash.svg`, `asterisk.svg` | `IconName::Dash/Asterisk` |
| `case-sensitive.svg` | `IconName::CaseSensitive` |
| `layout-dashboard.svg` | `IconName::LayoutDashboard` |
| `loader.svg` / `loader-circle.svg` | `IconName::Loader` / `LoaderCircle` |
| `bot.svg`, `github.svg`, `building-2.svg`, `map.svg` | `IconName::Bot/GitHub/Building2/Map` |
| `frame.svg`, `resize-corner.svg`, `square-terminal.svg` | `IconName::Frame/ResizeCorner/SquareTerminal` |
| `a-large-small.svg`, `chart-pie.svg` | `IconName::ALargeSmall/ChartPie` |
| `chevrons-up-down.svg` | `IconName::ChevronsUpDown` |
| `file.svg` | `IconName::File` |
| `folder.svg` / `folder-closed/open.svg` | `IconName::Folder` / `FolderClosed` / `FolderOpen` |
| `inspector.svg` | `IconName::Inspector` |

### 2.2 不迁移的图标

插件专属图标保持不变（属于插件 manifest 身份）：
`about`, `antenna`, `api`, `bolt`, `capture`, `clipboard`, `download`, `edit`, `folder-network`, `history`, `image`, `json`, `paste`, `qr`, `rocket`, `school`, `shield-eye`, `smartphone`

### 2.3 实现方案

新增 `crates/qingqi-ui/src/icon.rs`：

```rust
use gpui_component::{Icon, IconName, Sizable};

pub enum AppIcon {
    Named(IconName),
    Custom(&'static str),
}

impl AppIcon {
    pub fn element(self, size_px: f32) -> Icon {
        match self {
            AppIcon::Named(name) => Icon::new(name),
            AppIcon::Custom(path) => Icon::new(IconName::File).path(path.to_string()),
        }
    }
}
```

修改 `icon_element()` 函数 (`ui/mod.rs:215`) 内部调用 `AppIcon::element()`，保留 `text_color` 着色能力。

### 2.4 清理范围

- 从 `assets.rs` 删除已被覆盖的 ~50 个 `include_bytes!` 条目
- 从 `crates/qingqi/assets/icons/` 删除对应的 SVG 文件
- 更新所有 `"icons/xxx.svg"` 字符串引用为 `AppIcon::Named(IconName::Xxx)`

---

## 三、Phase 2: UI 组件迁移（~4 天）

### 3.1 qingqi-ui 共享组件迁移

| 当前组件 | 替换方案 | 操作 |
|---------|---------|------|
| `toggle` | `gpui_component::switch::Switch` | 直接替换，删除 `toggle.rs` |
| `separator` / `ui_divider` | `gpui_component::divider::Divider` | 直接替换 |
| `button` / `icon_button` | `gpui_component::button::Button` | 适配替换，删除 `button.rs` |
| `chip` | `gpui_component::badge::Badge` | 替换，删除 `chip.rs` |
| `table_header_cell/flex` | `gpui_component::table::Table` | 替换，删除 `table_header.rs` |
| `settings_card/row` | `gpui_component::setting::Setting` | 替换，删除 `settings.rs` |
| `overlay_host` | `gpui_component::dialog::Dialog` | 替换，删除 `overlay_host.rs` |
| `empty_state` | `v_flex` + Icon + Label 组合 | 保留（组合组件） |
| `status_pill` | `badge::Badge` + 自定义颜色 | 保留（组合组件） |

### 3.2 插件内手动 UI 替换

#### 3.2.1 Button 统一（27 处 → `button::Button`）

| 插件 | 手动按钮函数 | 数量 |
|------|------------|------|
| clipboard | `theme_button`, `pill_button` | 2 |
| system-settings | `retention_control` (4个), `plugin_dir_button`, `icon_cache_clear_button`, `shortcut_action_button`, `action_button` | 8 |
| quick-launch | `primary_action_button`, `action_button`, `icon_action_button`, `destructive_action_button`, `segment_button` | 5 |
| download-manager | `primary_btn`, `secondary_btn`, `action_button`, `action_icon` | 4 |
| api-debugger | dialog 内取消/确定按钮 | 4 |
| 其他 | http-capture, ssh, image-compress | 4 |

在 `qingqi-ui` 提供 `accent_button(accent, variant)` 工厂函数封装 Button，插件直接用。

#### 3.2.2 Chip/Badge 统一（17 处 → `badge::Badge` / `tag::Tag`）

| 插件 | 手动 form |
|------|---------|
| system-settings | `scope_badge`, `status_badge`, `disabled_badge`, `path_badge` |
| quick-launch | `kind_chip`, `subtle_chip`, `status_chip`, `latest_run_status_chip` |
| download-manager | `filter_chip`, `status_tag` |
| api-debugger | `section_micro_label`, `response_metric`, `circle_badge`, `status_badge` |
| http-capture | `status_badge`, `small_action` |
| clipboard | `icon_label` |

#### 3.2.3 Dialog/Overlay 统一（11 处 → `dialog::Dialog` + `sheet::Sheet`）

| 插件 | 弹窗 |
|------|------|
| quick-launch | `action_editor_sheet`, `pending_sheet`, `history_sheet`, `result_sheet`, `delete_confirm_sheet`, `menu_overlay_shell` (6个，~1200行) |
| api-debugger | `curl_import_dialog`, `rename_dialog`, `overlay_shell` (3个) |
| download-manager | `settings_overlay` |
| ssh | `context_menu` |

#### 3.2.4 Tab/Segment 统一（6 处 → `tab::TabBar`）

clipboard `render_filter_tabs`、download-manager `filter_bar`、system-settings `mode_segment`、quick-launch `segment_button`、ssh `tab_strip`

#### 3.2.5 其他

| 模式 | 数量 | 替代 |
|------|------|------|
| Input 外壳 | 6 处 | `input::Input` + `form::Form` |
| Dropdown/Menu | 4 处 | `menu::Menu` + `popover::Popover` |
| Progress Bar | 1 处 | `progress::Progress` |
| Table | 1 处 | `table::Table` |

### 3.3 Accent 适配

提供 `accent_style(accent) -> impl Fn(&mut StyleRefinement)` 辅助函数，将 `PluginAccent::Blue/Cyan/Green/Purple/Amber/Rose/Slate` 映射为对应的 `text_color` / `bg`。

---

## 四、实施顺序

```
Phase 1: Icon 迁移 (0.5天)
  ├─ 新增 AppIcon 枚举
  ├─ 修改 icon_element()
  ├─ 更新各调用处
  └─ 清理 assets.rs + SVG 文件

Phase 2: UI 组件迁移 (4天)
  ├─ Day 1: P0 组件 (toggle→Switch, divider→Divider)
  ├─ Day 1-2: P1 组件 (button 统一27处, badge/chip 统一17处, input外壳6处)
  ├─ Day 2-4: P2-P3 组件 (dialog/sheet 11处, tab 6处, table, progress, menu, tooltip)
  └─ Day 4: 收尾 (删除废弃组件文件, 清理依赖, 全量编译验证)
```

---

## 五、风险和缓解

| 风险 | 严重度 | 缓解 |
|------|--------|------|
| gpui-component API 不满足需求 | 低 | `empty_state`、`status_pill` 等复杂组合场景保留自定义 |
| 替换影响功能 | 中 | 逐个组件替换，每个替换后验证编译 + 功能 |
| dialog 迁移复杂度高 | 中 | 先替换简单 dialog，复杂弹窗保留原实现，逐步迁移 |
| Accent 色彩不完全匹配 | 低 | gpui-component 组件支持自定义 `text_color`/`bg`，可精确定制 |
| gpui-component 版本升级 | 低 | 锁定版本 `0.5.1`，与 GPUI `0.2.2` 兼容 |
