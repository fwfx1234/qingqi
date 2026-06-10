# SSH 插件 UI 重设计 v2 — 规格

> 日期：2026-06-10

## 设计决策

| 区域 | 风格 | 背景 |
|------|------|------|
| 侧边栏底色 | 毛玻璃 | `glass::panel` 不透度 0.04 |
| Profile 卡片 | 悬浮卡片（无状态指示） | `glass::bg` 不透度 0.08，独立阴影，圆角 8px |
| 文件树 | VS Code 风 | `ui::bg_surface()` 白底 |
| 终端 | 暗色终端 | `#fafbfc` 白底 |
| Session Tab | Chrome 风 | `#f8f9fa`，选中 `#ffffff`，底部无线 |
| 设置弹窗 | 模态弹窗 | 半透明遮罩 + 白底居中卡片 |

## 修改清单

### sidebar.rs
- 删除 `mac_traffic_lights()` 及相关引用
- 侧边栏底色 `glass::panel(true)`
- Profile 卡片：`rounded(px(8.0))`、阴影、`glass::bg(true)` 不透度增加
- 协议徽章：圆角小标签
- **无左侧色条、无连接状态指示**

### session_tabs.rs
- Chrome 风：选中 Tab `rounded_t_md` + 底部无边框（与内容融合）
- 状态点发光效果
- 关闭按钮 hover 变红

### file_tree.rs
- 白底 `bg(ui::bg_surface())`
- 图标：`Icon::new(IconName::Folder)` / `Icon::new(IconName::File)`
- 选中行：`hsla(0.55, 0.3, 0.4, 0.15)` 深色高亮
- 工具栏按钮圆角 `px(4.0)` + hover 背景

### settings_dialog.rs
- 遮罩层与弹窗内容区同级（不嵌套）
- 弹窗白底、圆角 `px(12.0)`、阴影
- TextInput 去外层边框包裹
- 保存/取消按钮右对齐

### terminal_pane.rs
- 白底 `bg(#fafbfc)`
- 状态栏 `user@host:path`
- 等宽字体 `Menlo`

## 不做
- 不添加文件夹/文件的连接状态
- 不添加 Profile 选中效果
- 不改变窗口级别按钮样式
