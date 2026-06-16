# 多主题配置功能设计

## 概述

废弃 `qingqi-ui` 自定义主题系统，统一使用 `gpui_component::theme`，通过 `ThemeRegistry` 管理多主题（21 个内置主题 + 用户自定义），在系统设置中提供主题选择器。

## 架构

```
启动流程:
  gpui_component::init(cx)
    -> ThemeRegistry 加载内置 Default Light/Dark
    -> ThemeRegistry::watch_dir("config/themes/") 发现内置主题 JSON
    -> ThemeStore 读取 theme.json 恢复上次选中的主题和模式
    -> ThemeService.apply(theme_name, mode) -> 更新 Theme 全局

运行时:
  Settings UI 选择主题 -> ThemeService.set_theme("macOS Classic")
                      -> 持久化到 theme.json
                      -> Theme::change(mode) 刷新全局颜色
                      -> cx.refresh_windows()

颜色读取 (所有 UI 代码):
  let t = Theme::global(cx);
  t.background           // 替代 theme::semantic().bg_page
  t.foreground           // 替代 theme::semantic().text_primary
  t.muted_foreground     // 替代 theme::semantic().text_secondary
  t.border               // 替代 theme::semantic().border_default
  t.primary              // 替代 theme::semantic().primary
```

## 数据与持久化

### theme.json 格式

```json
{
  "theme": "macOS Classic",
  "mode": "system"
}
```

向后兼容：旧文件只有 `mode` 则 `theme` 默认 `"Default"`。

### ThemeStore 接口

| 方法 | 说明 |
|------|------|
| `theme_name() -> &str` | 当前选中主题名 |
| `set_theme(name) -> Result` | 切换主题，持久化 |
| `mode() -> ThemeMode` | Light / Dark / System（不变） |
| `set_mode(mode) -> Result` | 切换模式（不变） |
| `themes() -> Vec<ThemeMeta>` | 从 ThemeRegistry 读取可用主题列表 |

`ThemeMode` 保持三档：Light / Dark / System。System 模式时调用 OS API 获取 `effective_dark()`。

## 内置主题

从 [gpui-component themes](https://github.com/longbridge/gpui-component/tree/main/themes) 下载全部 21 个主题 JSON 文件，存放在 `crates/qingqi-app/src/app/themes/`，通过 `include_str!` 嵌入二进制。

首批启动时将内置主题复制到 `config/themes/` 目录，`ThemeRegistry::watch_dir()` 自动加载。

## Settings UI

```
主题与外观
  主题风格   [ Default ] [ macOS Classic ] [ Catppuccin ] [ Ayu ] ...
             可滚动主题列表，选中高亮
  主题模式   [ 浅色 ] [ 深色 ] [ 跟随系统 ]
             保持现有三档 segment 控件
  系统检测   当前系统外观: 深色
```

主题列表来源于 `ThemeRegistry::global(cx).sorted_themes()`，按 `ThemeSet.name` 去重显示。

## 迁移映射

旧 `qingqi-ui::theme` token 替换：

| 旧 token | 新写法 |
|----------|-------|
| `semantic().bg_page` | `t.background` |
| `semantic().bg_surface` | `t.list` |
| `semantic().bg_elevated` | `t.popover` |
| `semantic().bg_subtle` | `t.muted` |
| `semantic().bg_hover` | `t.list_hover` |
| `semantic().border_default` | `t.border` |
| `semantic().text_primary` | `t.foreground` |
| `semantic().text_secondary` | `t.muted_foreground` |
| `semantic().text_body` | `t.muted_foreground` |
| `semantic().primary` | `t.primary` |
| `semantic().success` | `t.success` |
| `semantic().danger` | `t.danger` |
| `semantic().overlay_backdrop` | `t.overlay` |
| `accent_color(accent)` | `t.primary` 或 `t.base_blue` 等 |
| `font_ui()` | `t.font_family` |
| `font_size_body()` | `t.font_size` |
| `radius_md()` | `t.radius` |

## 清理清单

- 删除 `qingqi-ui/src/theme.rs` 中的 `SemanticColors`、`build_light/build_dark`、`semantic()`、颜色调色板常量
- 删除 `qingqi-ui/src/theme_mode.rs` 的 `AtomicBool`
- 删除 `theme.rs` 中 `accent_color/accent_soft/accent_hover` 函数
- `ui/glass.rs`、`ui/mod.rs` 中的辅助函数改为从 `Theme::global(cx)` 取值
- 删除 `qingqi-plugin/src/theme.rs` 中的 `ThemeMode` 枚举（或保留仅用于持久化）

## 新增文件

- `qingqi-ui/src/theme_service.rs` — 主题选择/应用/持久化服务
- `qingqi-app/src/app/themes/` — 21 个内置主题 JSON（`include_str!` 嵌入）

## 不做什么

- 不保留 qingqi-ui 自己的颜色层
- 不支持主题编辑/创建 UI（用户手动放 JSON 到 config/themes/ 即可）
- 不做运行时热加载 UI 预览
