# 多主题配置功能实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 废弃 qingqi-ui 自定义主题系统，统一使用 gpui_component::theme，支持 21 个内置主题的多主题选择器。

**Architecture:** ThemeStore 扩展为记录主题名+模式，启动时 ThemeRegistry 加载内置主题 JSON 并监听自定义目录，Settings UI 显示主题选择器。所有 UI 代码从 Theme::global(cx) 读取颜色。

**Tech Stack:** Rust, GPUI, gpui_component (theme system), serde_json

---

### Task 1: 扩展 ThemeStore 支持主题名称

**Files:**
- Modify: `crates/qingqi-app/src/app/theme_store.rs`
- Modify: `crates/qingqi-app/src/app/runtime.rs:56-84` (ThemeHandleAdapter)

- [ ] **Step 1: 扩展 ThemeConfig 结构体，添加 theme_name 字段**

```rust
// theme_store.rs — 替换 ThemeConfig
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ThemeConfig {
    #[serde(default = "default_theme_name")]
    theme: String,
    mode: ThemeMode,
}

fn default_theme_name() -> String {
    "Default".into()
}
```

- [ ] **Step 2: ThemeStore 添加 theme 字段和方法**

```rust
// theme_store.rs — 在 ThemeStore 结构体中添加
pub struct ThemeStore {
    mode: ThemeMode,
    theme: String,          // 新增
    config_path: PathBuf,
    system_dark: bool,
}

// ThemeStore 添加方法
impl ThemeStore {
    pub fn theme(&self) -> &str {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: String) -> Result<()> {
        if self.theme == theme {
            return Ok(());
        }
        let previous = self.theme.clone();
        self.theme = theme;
        if let Err(e) = self.save() {
            self.theme = previous;
            return Err(e);
        }
        Ok(())
    }
}
```

- [ ] **Step 3: 修改 new() 和 load_mode()，支持读取新格式并向后兼容**

```rust
// theme_store.rs — new() 中同时加载 theme name
pub fn new(config_path: PathBuf) -> Self {
    let (mode, theme) = Self::load_config(&config_path).unwrap_or_else(|error| {
        tracing::warn!(path = %config_path.display(), error = %error,
            "failed to load theme config, falling back to default");
        (ThemeMode::default(), "Default".to_string())
    });
    let system_dark = Self::read_system_dark();
    let store = Self { mode, theme, config_path, system_dark };
    store.apply_current();
    store
}

fn load_config(path: &Path) -> Result<(ThemeMode, String)> {
    if !path.exists() {
        return Ok((ThemeMode::default(), "Default".to_string()));
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("cannot read theme config {}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok((ThemeMode::default(), "Default".to_string()));
    }
    // 尝试新格式
    if let Ok(config) = serde_json::from_str::<ThemeConfig>(trimmed) {
        return Ok((config.mode, config.theme));
    }
    // 兼容旧格式：只有 mode
    if let Ok(mode) = serde_json::from_str::<ThemeMode>(trimmed) {
        return Ok((mode, "Default".to_string()));
    }
    bail!("invalid theme config format")
}

fn save(&self) -> Result<()> {
    if let Some(parent) = self.config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create theme config directory {}", parent.display()))?;
    }
    let config = ThemeConfig { theme: self.theme.clone(), mode: self.mode };
    let json = serde_json::to_string_pretty(&config).context("cannot encode theme config")?;
    fs::write(&self.config_path, json)
        .with_context(|| format!("cannot write theme config {}", self.config_path.display()))
}
```

- [ ] **Step 4: 扩展 ThemeHandle trait 添加 theme_name 方法**

```rust
// qingqi-plugin/src/host.rs — ThemeHandle trait 添加
fn theme_name(&self) -> String;
```

```rust
// runtime.rs — ThemeHandleAdapter 实现
fn theme_name(&self) -> String {
    self.store.read().map(|s| s.theme().to_string()).unwrap_or_default()
}
```

- [ ] **Step 5: 更新测试以覆盖新字段**

在 theme_store.rs 测试中添加主题名持久化测试。

- [ ] **Step 6: Build check**

Run: `cargo check -p qingqi-app -p qingqi-plugin`
Expected: No errors

---

### Task 2: 创建 ThemeService + 运行时初始化

**Files:**
- Create: `crates/qingqi-app/src/app/theme_service.rs`
- Modify: `crates/qingqi-app/src/app/runtime.rs:217-244`
- Modify: `crates/qingqi-app/src/app/background.rs` (theme listener)

- [ ] **Step 1: 创建 ThemeService 模块**

```rust
// theme_service.rs
use std::{fs, path::PathBuf, sync::Arc};
use anyhow::Result;
use gpui::{App, SharedString};
use gpui_component::theme::{Theme, ThemeRegistry};

pub struct ThemeService {
    themes_dir: PathBuf,
}

impl ThemeService {
    pub fn new(themes_dir: PathBuf) -> Self {
        Self { themes_dir }
    }

    /// 将内置主题 JSON 写入 themes 目录（如果不存在）
    fn seed_builtin_themes(&self) -> Result<()> {
        fs::create_dir_all(&self.themes_dir)?;
        let builtins: &[(&str, &str)] = &[
            ("adventure", include_str!("themes/adventure.json")),
            ("alduin", include_str!("themes/alduin.json")),
            ("asciinema", include_str!("themes/asciinema.json")),
            ("ayu", include_str!("themes/ayu.json")),
            ("catppuccin", include_str!("themes/catppuccin.json")),
            ("everforest", include_str!("themes/everforest.json")),
            ("fahrenheit", include_str!("themes/fahrenheit.json")),
            ("flexoki", include_str!("themes/flexoki.json")),
            ("gruvbox", include_str!("themes/gruvbox.json")),
            ("harper", include_str!("themes/harper.json")),
            ("hybrid", include_str!("themes/hybrid.json")),
            ("jellybeans", include_str!("themes/jellybeans.json")),
            ("kibble", include_str!("themes/kibble.json")),
            ("macos-classic", include_str!("themes/macos-classic.json")),
            ("matrix", include_str!("themes/matrix.json")),
            ("mellifluous", include_str!("themes/mellifluous.json")),
            ("molokai", include_str!("themes/molokai.json")),
            ("solarized", include_str!("themes/solarized.json")),
            ("spaceduck", include_str!("themes/spaceduck.json")),
            ("tokyonight", include_str!("themes/tokyonight.json")),
            ("twilight", include_str!("themes/twilight.json")),
        ];
        for (name, content) in builtins {
            let path = self.themes_dir.join(format!("{name}.json"));
            if !path.exists() {
                fs::write(&path, *content)?;
            }
        }
        Ok(())
    }

    /// App 启动时初始化：写入内置主题、注册 watch_dir、应用已保存的主题
    pub fn init(
        &self,
        theme_name: &str,
        mode: qingqi_plugin::theme::ThemeMode,
        cx: &mut App,
    ) -> Result<()> {
        self.seed_builtin_themes()?;

        // 注册监听 themes 目录
        ThemeRegistry::watch_dir(
            self.themes_dir.clone(),
            cx,
            |_cx| {}, // 目录变化时自动重载，无需额外操作
        )?;

        // 应用已保存的主题
        self.apply_theme(theme_name, mode, cx);
        Ok(())
    }

    /// 选择并应用主题
    pub fn apply_theme(
        &self,
        theme_name: &str,
        mode: qingqi_plugin::theme::ThemeMode,
        cx: &mut App,
    ) {
        // 找到主题的 light 或 dark 变体
        let registry = ThemeRegistry::global(cx);
        let themes = registry.themes();
        
        // 查找匹配的主题集（按 name 分组）
        let effective_dark = match mode {
            qingqi_plugin::theme::ThemeMode::Light => false,
            qingqi_plugin::theme::ThemeMode::Dark => true,
            qingqi_plugin::theme::ThemeMode::System => cx.window_appearance().is_dark(),
        };

        // 根据主题名找到对应的 ThemeConfig
        let target_mode = if effective_dark {
            gpui_component::theme::ThemeMode::Dark
        } else {
            gpui_component::theme::ThemeMode::Light
        };

        // 查找匹配的 theme config
        if let Some(config) = themes.values().find(|c| {
            c.name.as_ref() == theme_name && c.mode == target_mode
        }).or_else(|| {
            // fallback: 用默认主题
            if effective_dark {
                Some(ThemeRegistry::global(cx).default_dark_theme())
            } else {
                Some(ThemeRegistry::global(cx).default_light_theme())
            }
        }) {
            let theme = Theme::global_mut(cx);
            theme.apply_config(config);
        }
        
        Theme::change(target_mode, None, cx);
        cx.refresh_windows();
    }
}
```

- [ ] **Step 2: 修改 runtime.rs 初始化 ThemeService**

```rust
// runtime.rs — 在 run() 中，gpui_component::init(cx) 之后插入
let themes_dir = paths.data_dir().join("config").join("themes");
let theme_service = ThemeService::new(themes_dir);

let initial_theme = theme_store.read().map(|s| s.theme().to_string()).unwrap_or_default();
let initial_mode = theme_store.read().map(|s| s.mode()).unwrap_or_default();
if let Err(e) = theme_service.init(&initial_theme, initial_mode, cx) {
    tracing::error!(error = %e, "failed to init theme service");
}
```

- [ ] **Step 3: 修改 background.theme_listener 使用 Theme::change**

在 `background.rs` 中，当系统主题变化时调用 `Theme::sync_system_appearance(None, cx)` 替代 `qingqi_ui::theme_mode::set_dark()`。

- [ ] **Step 4: Build check**

Run: `cargo check -p qingqi-app`

---

### Task 3: 迁移 qingqi-ui — 核心主题文件

**Files:**
- Modify: `crates/qingqi-ui/src/theme.rs` (大幅精简)
- Delete: `crates/qingqi-ui/src/theme_mode.rs`
- Modify: `crates/qingqi-ui/src/lib.rs`

- [ ] **Step 1: 精简 theme.rs — 只保留工具函数和颜色映射**

保留：`rgba_with_alpha`, `http_method_color`, `accent_color`, `accent_soft`, `accent_soft_dark`, spacer/radius/font-size 函数（作为常量值，不以主题为准）

删除：`SemanticColors`, `build_light`, `build_dark`, `semantic()`, 所有 slate/blue/green/red/amber/violet/cyan/white 调色板函数, 所有 launcher_*, keycap_bg, terminal_* 函数，`accent_hover`

```rust
// theme.rs — 精简后
use gpui::{Hsla, Pixels, Rgba, hsla, px, rgb};
use qingqi_plugin::plugin_spec::PluginAccent;

// 保留：工具函数
pub fn rgba_with_alpha(color: Rgba, alpha: f32) -> Hsla { /* 不变 */ }

// 保留：http_method_color（改为接受 &Theme 或 is_dark: bool）
pub fn http_method_color(method: &str, dark: bool) -> Rgba { /* 不变 */ }

// 保留：accent 映射（这些是 PluginAccent 的颜色，不是主题色）
pub fn accent_color(accent: PluginAccent) -> Rgba { /* 不变 */ }
pub fn accent_soft(accent: PluginAccent) -> Rgba { /* 不变 */ }
pub fn accent_soft_dark(accent: PluginAccent) -> Rgba { /* 不变 */ }

// 保留：spacing/radius/font-size（这些是布局常量，沿用）
pub fn space_0p5() -> Pixels { px(2.0) }
pub fn space_1() -> Pixels { px(4.0) }
// ... 等等
pub fn radius_xs() -> Pixels { px(4.0) }
// ... 等等
pub fn font_size_title() -> Pixels { px(20.0) }
// ... 等等
```

- [ ] **Step 2: 删除 theme_mode.rs 并更新 lib.rs**

```rust
// lib.rs — 删除 pub mod theme_mode;
// 保留 pub mod theme;
```

- [ ] **Step 3: http_method_color 改为接受 dark: bool 参数**

```rust
pub fn http_method_color(method: &str, dark: bool) -> Rgba {
    match method {
        "GET" => if dark { rgb(0x34d399) } else { rgb(0x10b981) },
        "POST" => if dark { rgb(0xfbbf24) } else { rgb(0xf59e0b) },
        // ... 其余不变
        _ => if dark { rgb(0x94a3b8) } else { rgb(0x64748b) },
    }
}
```

- [ ] **Step 4: Build check（会有一堆编译错误，预期之内）**

---

### Task 4: 迁移 qingqi-ui — ui/mod.rs 和 ui/glass.rs

**Files:**
- Modify: `crates/qingqi-ui/src/ui/mod.rs`
- Modify: `crates/qingqi-ui/src/ui/glass.rs`

- [ ] **Step 1: 重写 ui/mod.rs 中的辅助函数，接受 &App 参数**

将所有 `fn xxx() -> Color` 改为 `fn xxx(cx: &App) -> Color`，从 `Theme::global(cx)` 取值：

```rust
use gpui_component::theme::Theme;

pub fn font_ui() -> &'static str { "Inter, PingFang SC" }  // 不变
pub fn font_mono() -> &'static str {
    if cfg!(target_os = "macos") { "Menlo" } else { "Consolas" }
}

pub fn bg_canvas(cx: &App) -> Hsla {
    Theme::global(cx).background
}
pub fn bg_surface(cx: &App) -> Hsla {
    Theme::global(cx).list
}
pub fn bg_subtle(cx: &App) -> Hsla {
    Theme::global(cx).muted
}
pub fn bg_hover(cx: &App) -> Hsla {
    Theme::global(cx).list_hover
}
pub fn text_primary(cx: &App) -> Hsla {
    Theme::global(cx).foreground
}
pub fn text_secondary(cx: &App) -> Hsla {
    Theme::global(cx).muted_foreground
}
pub fn text_tertiary(cx: &App) -> Hsla {
    Theme::global(cx).muted_foreground  // muted_foreground 对应 tertiary
}
pub fn border_light(cx: &App) -> Hsla {
    Theme::global(cx).border
}
pub fn success(cx: &App) -> Hsla {
    Theme::global(cx).success
}
pub fn warning(cx: &App) -> Hsla {
    Theme::global(cx).warning
}
pub fn danger(cx: &App) -> Hsla {
    Theme::global(cx).danger
}
pub fn info(cx: &App) -> Hsla {
    Theme::global(cx).info
}
pub fn overlay_backdrop(cx: &App) -> Hsla {
    Theme::global(cx).overlay
}
pub fn row_hover(cx: &App) -> Hsla {
    Theme::global(cx).list_hover
}
pub fn white() -> Hsla {
    gpui::hsla(0.0, 0.0, 1.0, 1.0)
}
pub fn accent_color(accent: PluginAccent) -> Rgba {
    theme::accent_color(accent)  // 从 theme.rs 的映射
}
pub fn accent_soft(accent: PluginAccent) -> Rgba {
    theme::accent_soft(accent)
}
// bg_keycap 移除，调用方直接用 Theme::global(cx).muted
```

- [ ] **Step 2: 更新 glass.rs 接受 &App 参数**

```rust
use gpui_component::theme::Theme;

pub fn bg(cx: &App) -> Hsla {
    let t = Theme::global(cx);
    rgba_with_alpha(t.list, if t.is_dark() { 0.22 } else { 0.82 })
}
pub fn border(cx: &App) -> Hsla {
    let t = Theme::global(cx);
    rgba_with_alpha(t.border, if t.is_dark() { 0.28 } else { 0.24 })
}
pub fn divider(cx: &App) -> Hsla {
    let t = Theme::global(cx);
    rgba_with_alpha(t.border, if t.is_dark() { 0.20 } else { 0.16 })
}
pub fn hover_bg(cx: &App) -> Hsla {
    if Theme::global(cx).is_dark() {
        hsla(0.0, 0.0, 1.0, 0.055)
    } else {
        hsla(0.0, 0.0, 0.88, 0.34)
    }
}
pub fn panel(cx: &App) -> Hsla {
    let t = Theme::global(cx);
    rgba_with_alpha(t.popover, if t.is_dark() { 0.55 } else { 0.78 })
}
pub fn inset(cx: &App) -> Hsla {
    if Theme::global(cx).is_dark() {
        hsla(225.0/360.0, 0.18, 0.10, 0.18)
    } else {
        rgba_with_alpha(Theme::global(cx).list, 0.50)
    }
}
pub fn sidebar(cx: &App) -> Hsla {
    let t = Theme::global(cx);
    rgba_with_alpha(t.sidebar, if t.is_dark() { 0.40 } else { 0.88 })
}
pub fn bar(cx: &App) -> Hsla {
    if Theme::global(cx).is_dark() {
        hsla(225.0/360.0, 0.16, 0.14, 0.26)
    } else {
        rgba_with_alpha(Theme::global(cx).list, 0.68)
    }
}
pub fn shadow() -> Vec<BoxShadow> { /* 不变 */ }
```

- [ ] **Step 3: 更新 ui/mod.rs 中其他使用 theme::semantic() 的函数**

如 `icon_tile`, `ui_button`, `ui_card`, `ui_chip`, `section_card`, `status_bar` 等 — 全部改为接受 `cx: &App` 或 `cx: &mut Window`。

---

### Task 5: 迁移 qingqi-ui — 组件文件

**Files:**
- Modify: `crates/qingqi-ui/src/ui/components/button.rs`
- Modify: `crates/qingqi-ui/src/ui/components/chip.rs`
- Modify: `crates/qingqi-ui/src/ui/components/toggle.rs`
- Modify: `crates/qingqi-ui/src/ui/components/settings.rs`
- Modify: `crates/qingqi-ui/src/ui/components/empty_state.rs`
- Modify: `crates/qingqi-ui/src/ui/components/status_pill.rs`
- Modify: `crates/qingqi-ui/src/ui/components/table_header.rs`
- Modify: `crates/qingqi-ui/src/ui/components/overlay_host.rs`
- Modify: `crates/qingqi-ui/src/ui/window_chrome.rs`
- Modify: `crates/qingqi-ui/src/text_input.rs`

- [ ] **Step 1: button.rs — 替换 theme 引用**

`ui::white()` → `gpui::hsla(0., 0., 1., 1.)`, `ui::danger()` → 调用方传入或直接用 `Theme::global(cx).danger`， `theme::radius_md()` → 保留（常量），`theme::font_size_body()` → 保留（常量），`crate::theme_mode::is_dark()` → `Theme::global(cx).is_dark()`（需要 cx 参数）。

- [ ] **Step 2: chip.rs — 替换 theme 引用**

`theme::accent_soft_dark/accent_soft` → 从 theme.rs 保留的函数（这些是 PluginAccent 映射，不需要转变）。`theme::radius_sm/font_size_caption` → 保留常量。

- [ ] **Step 3: toggle.rs — 替换 theme 引用**

`theme::blue_500()` → 用 `Theme::global(cx).blue` 替代（需要接受 cx 参数）。

- [ ] **Step 4: settings.rs — 替换 theme 引用**

`theme::radius_lg/space_4/space_3/font_size_*` → 保留常量。这些不依赖 theme_mode。

- [ ] **Step 5: empty_state.rs — 替换 font 引用**

保留 `theme::font_size_heading/font_size_body` 常量。

- [ ] **Step 6: status_pill.rs — 替换 color 引用**

`ui::success/warning/danger/info()` → 改为需要 cx 参数版本。

- [ ] **Step 7: table_header.rs — 保留常量**

`theme::font_size_caption` 保留。

- [ ] **Step 8: overlay_host.rs — 替换**

`ui::overlay_backdrop()` → `Theme::global(cx).overlay`。

- [ ] **Step 9: window_chrome.rs — 替换**

`ui::danger()`, `ui::white()`, `ui::bg_keycap()` → 需要 cx 参数。

- [ ] **Step 10: text_input.rs — 替换**

`ui::bg_surface()`, `ui::border_light()` → 需要 cx 参数。

---

### Task 6: 迁移 qingqi-app 启动器

**Files:**
- Modify: `crates/qingqi-app/src/app/launcher.rs`
- Modify: `crates/qingqi-app/src/app/window_controller.rs`

- [ ] **Step 1: launcher.rs — 大量 launcher_* 函数替换**

所有 `theme::launcher_*` 函数已被删除。迁移到 gpui-component token + `rgba_with_alpha` 组合：

| 旧 launcher 函数 | 新替代 |
|---|---|
| `launcher_title_text()` | `Theme::global(cx).foreground` |
| `launcher_faint_text()` | `Theme::global(cx).muted_foreground` |
| `launcher_muted_text()` | `Theme::global(cx).muted_foreground` |
| `launcher_accent()` | `Theme::global(cx).blue` |
| `launcher_glass()` | `rgba_with_alpha(Theme::global(cx).background, 0.98)` (light) / `0.30` (dark) |
| `launcher_soft_line()` | `rgba_with_alpha(Theme::global(cx).border, 0.9)` |
| `keycap_bg()` | `Theme::global(cx).muted` |
| `launcher_icon_surface()` | `rgba_with_alpha(Theme::global(cx).list, 0.78)` |
| `launcher_icon_border()` | `rgba_with_alpha(Theme::global(cx).border, 0.72)` |
| `launcher_row_hover()` | `Theme::global(cx).list_hover` |
| `launcher_row_selected()` | `Theme::global(cx).list_active` |
| `launcher_badge_bg()` | `rgba_with_alpha(Theme::global(cx).muted, 0.82)` |

注意：这些函数当前接受 `dark: bool` 参数，需要改为从 `&App` 获取。在所有调用方 `Render` 中可获得 `cx`。

- [ ] **Step 2: window_controller.rs — ui 函数替换**

`ui::window_close_button()` 需要 cx 参数。

---

### Task 7: 迁移 feature crates（分批）

**Files (batch 1):**
- `crates/qingqi-feature-system-settings/src/view.rs`
- `crates/qingqi-feature-gpui-demo/src/plugin.rs`
- `crates/qingqi-feature-about/src/view.rs`

- [ ] **Step 1: system-settings — 核心 UI 重写**

这是主题配置 UI 的核心文件。需要：
1. 在 `SettingsView` 中添加 `theme_handle: ThemeHandleRef` 获取可用主题列表
2. 重写 `settings_card("主题与外观")` 区域，添加主题选择器
3. 所有 `qingqi_ui::theme_mode::is_dark()` → `Theme::global(cx).is_dark()`
4. 所有 `theme::semantic().xxx` → `Theme::global(cx).xxx` 或对应的 `ui::xxx(cx)`
5. 所有 `theme::space_*`, `theme::radius_*`, `theme::font_size_*` → 保留常量
6. `theme::white()` → `gpui::hsla(0., 0., 1., 1.)`
7. `theme::rgba_with_alpha(color, alpha)` → 保留

**主题选择器 UI 实现：**

```rust
// 在 settings_card("主题与外观") 中添加
fn theme_selector(
    entity: Entity<SettingsView>,
    current_theme: &str,
    dark: bool,
) -> impl IntoElement {
    let themes = /* 从 ThemeRegistry 获取主题列表 */;
    // 水平滚动的主题卡片列表
    div()
        .flex()
        .gap_2()
        .overflow_x_scroll()
        .children(themes.into_iter().map(|t| theme_card(entity.clone(), t, current_theme, dark)))
}

fn theme_card(
    entity: Entity<SettingsView>,
    theme: ThemeMeta,
    current: &str,
    dark: bool,
) -> impl IntoElement {
    let active = theme.name == current;
    div()
        .id(format!("theme-{}", theme.name))
        .px_3().py_2()
        .rounded(theme::radius_md())
        .border_1()
        .border_color(if active { Theme::global(cx).primary } else { Theme::global(cx).border })
        .bg(if active { Theme::global(cx).list_active } else { Theme::global(cx).list })
        .hover(|s| s.bg(Theme::global(cx).list_hover).cursor_pointer())
        .flex().flex_col().gap_1()
        .child(div().text_size(theme::font_size_caption()).child(&theme.name))
        .on_click({ /* set_theme */ })
}
```

- [ ] **Step 2: gpui-demo — 迁移**

替换 `theme::semantic().xxx` → `Theme::global(cx).xxx` 或 `ui::xxx(cx)`。

- [ ] **Step 3: about — 迁移**

同上。

---

**Files (batch 2):**
- `crates/qingqi-feature-api-debugger/src/view/mod.rs`
- `crates/qingqi-feature-api-debugger/src/view/components/*.rs`
- `crates/qingqi-feature-ssh/src/view/*.rs`

- [ ] **Step 4: api-debugger — 迁移**

主要替换：
- `qingqi_ui::theme_mode::is_dark()` → `Theme::global(cx).is_dark()`
- `glass::*` 函数接受 cx 参数
- `theme::http_method_color(..., dark)` → 现在接受 `dark: bool`
- `ui::text_secondary()`, `ui::text_tertiary()` 等

- [ ] **Step 5: ssh — 迁移**

主要替换：
- `glass::sidebar(cx)`, `glass::border(cx)`, `glass::divider(cx)` 等
- `theme::blue_500()`, `theme::blue_600()` → `Theme::global(cx).blue`
- 所有 `ui::*` 辅助函数需要 cx

---

**Files (batch 3):**
- `crates/qingqi-feature-clipboard/src/view/*.rs`
- `crates/qingqi-feature-quick-launch/src/view.rs`
- `crates/qingqi-feature-http-capture/src/view.rs`
- `crates/qingqi-feature-image-compress/src/view.rs`
- `crates/qingqi-feature-download-manager/src/view.rs`
- `crates/qingqi-feature-qr-code/src/view.rs`
- `crates/qingqi-feature-json-parser/src/view.rs`
- `crates/qingqi-feature-anti-peeping/src/plugin.rs`

- [ ] **Step 6: 迁移**

每批替换模式相同：
1. `qingqi_ui::theme_mode::is_dark()` → `Theme::global(cx).is_dark()`
2. `theme::semantic().xxx` → `ui::xxx(cx)` 或 `Theme::global(cx).xxx`
3. `theme::rgba_with_alpha(theme::semantic().xxx, a)` → `theme::rgba_with_alpha(Theme::global(cx).xxx, a)`
4. `ui::accent_color(accent)` → 保留（从 theme.rs 的 accent 映射获取）
5. `ui::border_light()` → `ui::border_light(cx)` 或 `Theme::global(cx).border`
6. 所有 `ui::*` 辅助函数调用添加 cx 参数
7. `theme::white()` → `gpui::hsla(0., 0., 1., 1.)`

---

### Task 8: 最终验证

- [ ] **Step 1: cargo check 全 workspace**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 2: cargo test**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 3: 手动检查**

启动应用，验证：
- 主题选择器显示所有 21 个内置主题
- 切换主题后 UI 颜色即时变化
- Light/Dark/System 模式正确切换
- 主题选择持久化到 theme.json
