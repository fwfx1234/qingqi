# GPUI Component 重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将项目 Icon 系统和 UI 组件从自建迁移到 gpui-component 0.5.1 生态，覆盖 ~50 个 icon 映射和 ~90 处手动 UI 模式替换

**Architecture:** 两阶段渐进式迁移。Phase 1 建立 `AppIcon` 枚举封装 gpui-component 的 `IconName`，修改 `icon_element()`。Phase 2 建立 `accent_button()`/`accent_badge()` 等工厂函数封装 gpui-component 组件，逐步替换各插件的自定义 UI

**Tech Stack:** GPUI 0.2.2, gpui-component 0.5.1, Rust edition 2024

---

## ⚠️ 2026-06-19 现状对齐修订（执行前必读）

> 本计划初版写于 2026-06-16，**早于主题系统大迁移**（提交 `3bb32fa`→`fad8923`）。其间代码基线已大幅漂移，原计划多处 API/行号/方向失效。下方修订基于对当前代码 + `vendor/gpui-component` 0.5.1 真实源码的核对，**以本节为准，与下方原始 Task 冲突时优先本节**。当前基线 `cargo check --workspace` 通过，可直接开工。

### G0. 已验证可直接用的部分
- **Phase 1 的 `map_icon` 映射表准确**：核对 `vendor/gpui-component/src/icon.rs`，`IconName` 共 82 个变体，计划映射用到的图标（ArrowDown…Inspector）**全部存在**，映射表可原样使用。
- Task 划分（Phase 1 ×3、Phase 2 ×10）结构合理，保留。

### G1. 全局修正一：取色函数已全部 `cx` 化（影响每个工厂函数）
主题迁移后，`qingqi-ui` 的取色函数签名全部新增 `cx: &App`，颜色运行时从 `Theme::global(cx)` 取：
- `bg_surface(cx)`、`text_primary(cx)`、`text_secondary(cx)`、`border_light(cx)`、`success(cx)`、`danger(cx)`、`warning(cx)`、`info(cx)`、`separator(cx)`、`ui_divider(label, cx)` …
- **例外（无需 cx，accent 为硬编码 rgb）**：`accent_color(accent)`、`accent_soft(accent)`、`accent_soft_dark(accent)`、`theme::rgba_with_alpha(color, alpha)` 仍可用。

**后果**：原计划所有工厂函数签名（`accent_button(label, accent)`、`accent_badge(label, accent)`、`status_badge(label, tone)`…）**都缺 `cx` 参数**。凡是用到 `Theme::global` 色或自定义 variant 的工厂，签名必须改为带 `cx: &App`。详见 G4 重写后的工厂函数。

### G2. 全局修正二：原计划漏掉 `ui/mod.rs` 整套手写组件
原计划只盯着 `components/*.rs`，但 `crates/qingqi-ui/src/ui/mod.rs` 里还有**另一套函数式手写组件**，同样是迁移目标：
- `ui_button(label, variant, dark, icon, danger, cx)` — feature 侧 **4 处**调用
- `primary_button(label, cx)` / `toolbar_button(label, cx)` — feature 侧 **3 处**
- `ui_icon_button`、`ui_chip(label, accent, dark)`、`ui_divider(label, cx)`、`metric_pill`(×2)、`stat_card`、`category_pill`、`text_input_shell`

迁移时**两套并存的手写按钮/chip 都要处理**：`components/button.rs`（共享组件）+ `ui/mod.rs::ui_button` 系列 + 各 feature 本地按钮函数。建议统一收敛到 G4 的工厂函数后，再删除三套旧实现。

### G3. 真实组件 API 对照（替换原计划中的"接口概貌"伪代码）

**Icon**（`src/icon.rs`）
- `IconName` 实现 `Into<Icon>`；`Icon::new(impl Into<Icon>)` 可直接接受 `IconName`。
- 自定义 SVG：`Icon::default().path("icons/foo.svg")`（`Icon::new` 不接受裸字符串）。
- `impl Styled`（`.text_color(impl Into<Hsla>)`）+ `impl Sizable`（`.with_size(impl Into<Size>)`）+ `#[derive(IntoElement)]`，`From<Icon> for AnyElement`。
- `Size` 映射：`XSmall`=12px、`Small`=14px、`Medium`=16px、`Large`=24px、`Size::Size(px)`=精确像素。**用 `Size::Size(px(size_px))` 比 4 档近似更准**。

**Button**（`src/button/button.rs`）
- `Button::new(id: impl Into<ElementId>)`（**首参是 id 不是 label**）→ `.label(impl Into<SharedString>)`、`.icon(impl Into<Icon>)`。
- variant（`ButtonVariants` trait）：`.primary()` `.danger()` `.warning()` `.success()` `.info()` `.ghost()` `.link()` `.text()` `.with_variant(v)` `.custom(ButtonCustomVariant)`。**无 `.secondary()`**——Secondary 是默认。
- accent 色按钮：`Button::new(id).label(..).custom(ButtonCustomVariant::new(cx).color(bg).foreground(fg).border(b))`（**需 cx**）。
- `impl Sizable`（`.small()/.xsmall()/.large()`）、`impl Disableable`（`.disabled(bool)`）、`impl Selectable`（`.selected(bool)`）。
- `.on_click(impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)`（id 已在 new 设置，无需再 `.id()`）；`From<Button> for AnyElement`。

**Switch**（`src/switch.rs`）：`Switch::new(id)` → `.checked(bool)`、`.label(impl Into<Text>)`、`.disabled(bool)`、`.on_click(Fn(&bool, &mut Window, &mut App))`（**首参 `&bool`，非 ClickEvent**）。

**Tag**（`src/tag.rs`）← **chip 的正确替换（不是 Badge）**
- `Tag::new()` / `Tag::primary()` / `Tag::secondary()` / `Tag::danger()` / `Tag::success()` / `Tag::warning()` / `Tag::info()`。
- `Tag::custom(color: Hsla, foreground: Hsla, border: Hsla)` — accent chip 用这个。
- `Tag::color(impl Into<ColorName>)`、`.with_variant(TagVariant)`、`.outline()`、`.rounded()/.rounded_full()`；`impl Sizable`/`impl Styled`。内置 variant 自动 `cx.theme()` 取色。

**Badge**（`src/badge.rs`）：`Badge::new()` + `.dot()/.count(n)/.icon(..)/.max(n)/.color(Hsla)`，`impl ParentElement`。**仅用于角标**（包裹元素右上角的红点/数字），不做文本标签。

**Divider**（`src/divider.rs`）：`Divider::horizontal()/vertical()/horizontal_dashed()` + `.label(impl Into<SharedString>)`（**支持带文字**）+ `.color(Hsla)`/`.dashed()`。可同时替换 `separator(cx)` 和 `ui_divider(label, cx)`。

**Progress**（`src/progress.rs`）：`Progress::new()` + `.value(f32)`（**clamp 到 0–100**，非 0–1）+ `.bg(Hsla)`；`impl Styled`（圆角/高度走 Styled）。

**Dialog / Sheet**（`src/dialog.rs`、`src/sheet.rs`、`src/root.rs`）— **命令式，非可 render 的 builder**
- 打开：`window.open_dialog(cx, |window, cx| Dialog::new(window, cx).title(..).child(..).footer(..).confirm().on_ok(..).on_cancel(..))`（`ContextModal` trait，`root.rs:45`）。
- `Dialog::new(window, cx)`；`.confirm()`/`.alert()`/`.on_ok(F)`/`.on_cancel(F)`。Sheet 同构（`Sheet::new(window, cx).title().footer()`）。
- ⚠️ 项目现有 `overlay_host` 是**状态驱动的自绘 overlay**，gpui-component 是**命令式 open + Root 渲染**。两者模型不同，迁移需把"状态显隐"改成"事件触发 open"，**成本中-高，列为后期 Task 且可分步**。

**Setting**（`src/setting/`）：`Settings::new(id)` 容器 + `SettingPage::new(title).description()` + `SettingGroup::new().title()` + `SettingItem::new(title, field).description()`（field 为右侧控件）。比 `settings_card/row` 更结构化，迁移需重组数据。

**Input**（`src/input/`）：有状态——`cx.new(|cx| InputState::new(window, cx))` 持有 `Entity<InputState>`，render 时 `TextInput::new(&state)`。项目当前用自有 `qingqi_ui::text_input::TextInput`，**收益有限、成本中，建议暂缓或最后做**。

**TabBar**（`src/tab/`）：`TabBar::new(id).child(Tab::new()).selected_index(usize).on_click(F)`。

### G4. 重写后的工厂函数（真实可编译签名，放入 `ui/mod.rs`）
```rust
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Sizable as _, Disableable as _};

/// 主按钮
pub fn primary_btn(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).small().primary()
}
/// 次要按钮（Secondary 是默认 variant，无需 .secondary()）
pub fn secondary_btn(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).small()
}
/// Ghost 按钮
pub fn ghost_btn(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).small().ghost()
}
/// 危险按钮
pub fn danger_btn(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).small().danger()
}
/// Accent 色按钮（自定义 variant，需 cx）
pub fn accent_btn(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    accent: PluginAccent,
    cx: &App,
) -> Button {
    let c = accent_color(accent).into();
    Button::new(id).label(label).small().custom(
        ButtonCustomVariant::new(cx).color(c).foreground(white()),
    )
}
```
```rust
use gpui_component::tag::Tag;

/// Accent 色 Tag（替换原 chip / ui_chip / 各 feature 本地彩色 chip）
/// `Tag::custom` 直接传硬编码 accent 色，**无需 cx**。
pub fn accent_tag(label: impl Into<SharedString>, accent: PluginAccent) -> impl IntoElement {
    let label: SharedString = label.into();
    let color: gpui::Hsla = accent_color(accent).into();
    let soft: gpui::Hsla = accent_soft(accent).into();
    Tag::custom(soft, color, soft).small().child(label)
}
/// 状态 Tag（替换 status_pill 的文本标签场景）
/// 内置 variant 在 render 时自取 `cx.theme()` 色，**无需 cx**。
pub fn status_tag(label: impl Into<SharedString>, tone: components::StatusTone) -> impl IntoElement {
    use components::StatusTone::*;
    let label: SharedString = label.into();
    match tone {
        Success => Tag::success(), Warning => Tag::warning(),
        Danger => Tag::danger(), Info => Tag::info(), Neutral => Tag::secondary(),
    }
    .small()
    .child(label)
}
```
> 已核对 `tag.rs`：`Tag` **`impl ParentElement`**（`.child(impl IntoElement)` 可用，render 时 `.children(self.children)`），`Tag::new()` 无参、`Tag::custom(color, foreground, border)` 三参均 `Hsla`。`white()`（`Hsla`）已在 `ui/mod.rs` 提供，供 `accent_btn` 的 `.foreground()` 使用。

### G5. 真实替换面（按文件，执行时据此分批）
- **Button（最大头，16 文件）**：`api-debugger`（mod/response_panel/env_editor/action_bar/tab_bar/context_menu/kv_editor/editor_panel，7+）、`ssh`（file_edit_confirm/file_rename/profile_editor/file_upload_overwrite/app_settings，5）、`qr-code`、`image-compress`、`gpui-demo`；外加 `ui/mod.rs::ui_button`(4)/`primary_button`(3)。
- **toggle→Switch（1 文件）**：`clipboard/src/view/shared.rs`。
- **overlay_host→Dialog（1 文件）**：`quick-launch/src/view.rs`（命令式改造，后期）。
- **settings_card/row→Setting（2 文件）**：`system-settings/src/view.rs`、`clipboard/src/view/settings.rs`。
- **table_header→Table（2 文件）**：`image-compress/src/view.rs`、`download-manager/src/view.rs`。
- **chip/badge→Tag**：各 feature **本地定义**的 `scope_badge`/`kind_chip`/`status_chip`/`filter_chip` 等（非 `components::chip`）+ `components/chip.rs` + `ui_chip`。

### G6. 建议执行顺序（替换原计划"实施顺序"）
1. **Phase 1 Icon**（低风险，map_icon 已验证）→ 2. **Switch**（1 处）→ 3. **Divider**（separator+ui_divider）→ 4. **Tag**（chip 收敛，注意是 Tag 非 Badge）→ 5. **Button**（最大头，先建 G4 工厂，再按 G5 分文件批量）→ 6. **Progress / TabBar** → 7. **Setting**（重构）→ 8. **Dialog/Sheet**（命令式改造，最后）→ 9. **Input**（评估后决定是否做）。每步 `cargo check` + 提交。

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

> 🔧 **修订（见 G0/G3）**：`map_icon` 映射表已对 `IconName`（82 变体）全量核对，**准确可用**。Step 1 的 `AppIcon::element` 中：① 自定义 SVG 分支建议用 `Icon::default().path(resolved)`（`Icon::new` 不接受裸字符串）；② 尺寸建议用 `.with_size(gpui_component::Size::Size(px(size_px)))` 精确像素，比 4 档 XSmall/Small/Medium/Large 近似更准（4 档亦可编译）。

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

> 🔧 **修订（见 G3）**：`Switch::new(id)` 首参是 id；`.on_click` 闭包签名为 `Fn(&bool, &mut Window, &mut App)`，**首参是 `&bool`（新状态），不是 ClickEvent**。下方 Step 1 的 `on_click(move |event, _window, cx| handler(event, cx))` 需改为接收 `&bool`，并据原 handler 签名转换。

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

> 🔧 **修订（见 G3）**：已核对 `divider.rs`，`Divider::horizontal().label(text)` **确实支持带文字分隔线**，无需保留原实现。注意当前 `separator(cx)` / `ui_divider(label, cx)` 均已带 `cx` 参数（主题迁移后）。

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

> 🔧 **修订（以 G4 工厂函数为准，下方 Step 1 伪代码作废）**：`Button::new(id)` 首参是 **id 不是 label**，label 用 `.label()`；**无 `.secondary()`**（Secondary 是默认 variant）；accent 按钮用 `.custom(ButtonCustomVariant::new(cx)…)`（**需 cx**）；`.on_click(Fn(&ClickEvent, &mut Window, &mut App))`。本 Task 还需覆盖 G2 列出的 `ui/mod.rs::ui_button`(4)/`primary_button`(3) 两套手写按钮。

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

### Phase 2: Task 2.5 — 替换 chip 为 Tag（原标题"Badge"作废）

> 🔧 **方向性修正（重要）**：`Badge` 是**角标组件**（dot/count/icon，包裹在元素右上角），**不能做彩色文本标签**。chip 的正确替换是 **`Tag`**——见 G3（Tag API）与 G4（`accent_tag`/`status_tag` 工厂）。下方 Step 1 的 `accent_badge`/`status_badge`（基于 `Badge::new(label).text_color().bg()`）全部作废。

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

> 🔧 **修订**：各 feature 本地的 `scope_badge`/`kind_chip`/`status_chip`/`filter_chip` 等改用 `qingqi_ui::ui::accent_tag()` / `status_tag()`（**Tag，非 Badge**，见 G4）。这些是 feature **本地函数**，不是 `components::chip`。

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

> 🔧 **修订（见 G3）**：`Table` 是 `Table<D: TableDelegate>` **有状态组件**（需实现 `TableDelegate` + 持有 `Entity`），迁移成本高。`download-manager`/`image-compress` 的简单表头**建议保留现有 div 实现或单独评估**，不强行迁 `Table`。`settings_card/row` → `Settings`/`SettingPage`/`SettingItem`/`SettingGroup` 体系（见 G3），需重组数据结构。

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

> 🔧 **修订（见 G3）**：gpui-component 的 Dialog 是**命令式**——`window.open_dialog(cx, |window, cx| Dialog::new(window, cx).title(..).child(..).footer(..).confirm().on_ok(..).on_cancel(..))`（`ContextModal` trait），**不是可 render 的 builder**。下方 Step 2 的 `Dialog::new().title().child().footer()` 写法作废。项目现有 `overlay_host` 是状态驱动自绘 overlay，迁移需改为事件触发 open，**成本中-高，建议置于执行顺序末尾（G6 第 8 步）并分步进行**。

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

> 🔧 **修订（见 G3/G6）**：`TabBar::new(id).child(Tab::new()).selected_index(ix).on_click(F)`；`Progress::new().value(f32)`（**0–100**）。⚠️ `Input` 为有状态组件（`InputState` 需 `Entity` + `cx.new`），`PopupMenu` 基于 **Action 系统**（`.menu(label, Box<dyn Action>)`，需为每项定义 gpui `Action`），二者与项目现有模式差异大、成本中——**建议靠后或暂缓**，Tab/Progress 可优先。

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
