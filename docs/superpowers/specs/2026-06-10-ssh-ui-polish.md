# SSH 插件 UI 修复 — 设计规格

> 日期：2026-06-10

## 问题诊断

| # | 问题 | 根因 |
|---|------|------|
| 1 | 左侧边栏出现两套交通灯 | `PluginWindowMode::Window` 原生标题栏 + `sidebar.rs` 手绘 macOS 交通灯 |
| 2 | 弹窗点击任意位置即关闭，输入框无法获取焦点 | 遮罩层 `on_click` 关闭事件冒泡到弹窗内容区，GPUI 0.2.2 无法阻止冒泡 |
| 3 | Profile 卡片无毛玻璃，文件列表反而是毛玻璃 | 样式写反 |
| 4 | TextInput 样式异常 | 外层包裹 `border_1()` + `rounded_md()` 与 TextInput 自带 chrome 冲突 |

## 修复方案

### 1. 去掉手绘交通灯

**文件**: `sidebar.rs`
- 删除 `mac_traffic_lights()` 函数
- 标题栏只保留 "远程管理" 标题 + "+" 按钮
- 交通灯由 `PluginWindowMode::Window` 原生标题栏提供

### 2. 弹窗遮罩/内容区分离

**文件**: `settings_dialog.rs`
- 遮罩层和弹窗内容区渲染为**同级元素**（不嵌套）
- 只对遮罩层添加 `on_click` 关闭
- 弹窗内容区不再有 `on_click(|_,_,_|{})` 阻止冒泡
- 结构：
  ```
  div.overlay (absolute, full-size, on_click=close)
  div.dialog  (absolute, centered, NO on_click)
  ```

### 3. TextInput 样式修复

**文件**: `settings_dialog.rs`
- `render_text_input` 中去掉外层 `border_1()` + `rounded_md()`
- TextInput 自带 chrome 渲染边框和圆角

### 4. Profile 毛玻璃 + 文件白底

**文件**: `sidebar.rs`, `file_tree.rs`
- Profile 卡片：使用 `qingqi_ui::ui::glass::panel(dark)` 或 `glass::bg(dark)` 实现毛玻璃
- 文件列表：使用 `ui::bg_surface()` 白色背景

### 5. 连接链路验证

**文件**: 无新增改动
- "+" 按钮 → `toggle_settings` → 弹窗 → 填写表单 → "保存" → `create_profile_from_form` → Profile 列表更新 → 点击卡片 → `connect_profile` → `open_session`
- 整条链路已在上轮实现中完成，本次修复后即可端到端测试

## 修改文件

| 文件 | 修改 |
|------|------|
| `view/sidebar.rs` | 去交通灯、加毛玻璃 |
| `view/settings_dialog.rs` | 遮罩分离、TextInput 去边框 |
| `view/file_tree.rs` | 文件列表白底 |

## 验证

- [ ] 交通灯只出现一套
- [ ] 点击 "+" 打开设置弹窗
- [ ] 弹窗内 TextInput 可输入文字
- [ ] 点击遮罩区域（非弹窗内容区）关闭弹窗
- [ ] 点击弹窗内容区不关闭
- [ ] "保存" 按钮创建 Profile，弹窗关闭
- [ ] Profile 卡片显示毛玻璃效果
- [ ] 文件列表显示白色背景
- [ ] 点击 Profile 卡片触发 `connect_profile`
