# 插件界面问题清单与优化方案

> 状态：待处理 / 设计与重构路线文档。
>
> 本文记录 2026-05-30 对当前插件界面的审查结论，用于后续逐步重构。它不是现状说明，而是后续 UI 统一化的执行依据。
>
> 相关约定：
> - [architecture.md](architecture.md)：插件三种形态与整体架构。
> - [conventions.md](conventions.md)：UI 组件、主题 token、图标、性能铁律。
> - [gpui-component-guide.md](gpui-component-guide.md)：gpui-component 的使用边界与迁移手册。

---

## 1. 产品目标

插件界面的最终目标不是“组件堆满”，而是：

1. **界面无遮挡**
   - 任意弹层、菜单、确认框、详情面板不得遮住当前任务的关键操作。
   - 弹层必须有明确关闭路径：`Esc`、点击遮罩、取消按钮至少一种；复杂弹层建议三者都有。
   - 长内容必须可滚动，底部主操作不能被窗口高度变化遮挡。

2. **正常使用优先**
   - 插件打开后默认展示最常用路径，不要求用户先配置才能完成基本任务。
   - 没有数据、加载中、失败、权限不足、后台任务运行中都必须有明确状态。
   - 常用动作必须可见、可点击、可用键盘触发。

3. **常用功能简洁**
   - 顶部只放 1–3 个主动作；避免把所有高级选项直接堆在主界面。
   - 列表行只展示识别与决策所需信息；详细信息进入详情区、抽屉或配置页。
   - 文案短、动作动词化，例如“新建连接”“开始压缩”“保存二维码”。

4. **高级功能有入口配置**
   - 高级参数进入“更多 / 设置 / 高级”入口，不直接占据主流程空间。
   - 高级配置应当可折叠、可恢复默认值，并说明影响范围。
   - 常用配置可以记忆上次选择，但不能让旧配置悄悄改变用户预期。

5. **复杂独立窗口也要紧凑、现代化**
   - FTP/SFTP/SSH、API 调试器、快速启动管理这类复杂插件允许多面板，但必须有清晰信息层级。
   - 主区域优先展示“当前正在做什么”；辅助面板可折叠、可拖拽宽度或在窄屏堆叠。
   - 现代化不等于装饰多：统一间距、圆角、阴影、空态、hover、disabled，比毛玻璃/emoji 更重要。

---

## 2. 当前总体问题

### 2.1 组件没有真正统一

项目已有 `src/app/ui.rs` 作为共享 UI 原语层，提供：

- 语义 token：`bg_surface`、`bg_subtle`、`text_primary`、`text_secondary`、`border_light`、`success`、`warning`、`danger`、`accent_color`、`accent_soft` 等。
- 组件原语：`section_card`、`page_title`、`separator`、`status_bar`、`badge`、`mono_block`、`icon_element`、`icon_tile`、`toolbar_button`、`primary_button`、`text_input_shell`、`metric_pill`、`stat_card`、`status_pill`、`category_pill`、`row_card`、`plugin_surface`、`plugin_content`、`plugin_scroll_content`、`ui_button`、`ui_icon_button`、`ui_card`、`ui_badge`、`ui_empty_state`、`ui_chip`、`ui_divider`。

但现实是，大多数插件仍在自己的 `view.rs` 里重新定义按钮、chip、badge、输入框壳、表头、空状态。例如：

| 重复类型 | 现状 | 应收敛到 |
|---|---|---|
| 主按钮 | `api_debugger::primary_button`、`image_compress::primary_button`、`qr_code::primary_action_button`、`quick_launch::primary_action_button` 等 | 统一 `ui::Button` / `ui::ui_button` |
| 次按钮 / 普通按钮 | `action_button` 在多个插件里同名不同高度、字号、hover | 统一 button variant |
| chip / badge / pill | `kind_chip`、`status_chip`、`filter_chip`、`segmented_chip`、`status_pill` 重复 | `ui_chip` / `ui_badge` / `status_pill` |
| 表头 | `download_manager` 与 `image_compress` 各自定义 `table_header_cell` / `table_header_flex` | 共享 table/header 原语 |
| 输入框壳 | `labeled_field`、`profile_field`、`settings_input_group`、`sheet_input` 等 | `LabeledField` / `text_input_shell` |
| 空状态 | `empty_state_card`、各插件本地 `empty_state` | `ui_empty_state` + 插件 icon |

### 2.2 视觉 token 被绕过

当前已有 `conventions.md` §8 对主题与样式 token 做了硬约定，但现状仍有这些问题：

- 根字体不一致：有的用 `"Inter, PingFang SC"`，有的只用 `"PingFang SC"`，导致英文/数字字体风格不一致。
- 多处硬编码 `"SF Mono"`，应使用 `ui::font_mono()`。
- FTP/SFTP/SSH 视图存在大量裸 `rgb(0x...)` 颜色，应改为语义 token。
- 插件 view 直接调用 launcher 专用颜色，如 `theme::launcher_soft_line`、`launcher_muted_text`，属于跨层样式泄漏。
- 圆角取值过多，当前至少出现 `3/4/5/6/7/8/9/10/11/12/14/16/18/20/999` 等，应收敛到 `theme::radius_sm/md/lg` 和 pill 圆角。

### 2.3 图标体系不一致

项目已经迁移到 SVG 图标体系，但当前 view 中仍有多处 emoji 作为视觉图标，例如标题或按钮里的“📋 / 📂 / 📱 / 📡”。这会带来：

- 不同系统字体下显示不一致。
- 无法跟随主题色 tint。
- 与 `assets/icons/*.svg` 的统一方向冲突。

后续新增或替换图标应遵循 `conventions.md` §9：

- 新图标放 `assets/icons/<kebab-name>.svg`。
- 取用走 `ui::icon_element` / `ui::icon_tile`。
- 禁止新增 `qta/*.png`。

### 2.4 交互状态不完整

主要问题：

- 很多本地按钮 hover 只设置 `cursor_pointer()`，没有背景/边框变化，缺少反馈。
- disabled 常用 `.opacity(0.4)` 伪装，没有统一 disabled 语义，也可能仍可点击。
- 弹层数量多但没有统一 `ModalHost` / `OverlayHost`，例如快速启动管理界面用多个互斥 `else if` 分支拼弹层。
- 空状态、加载态、错误态有的插件有，有的没有；文案和位置不统一。
- 高级配置经常直接占据主界面空间，导致常用路径被挤压。

### 2.5 复杂插件布局不够自适应

复杂插件（FTP/SFTP/SSH、API 调试器、快速启动管理）有多栏、多面板、弹层、日志、表格、终端/编辑器等复杂区域。现状问题：

- 只有 API 调试器明显做了窄屏堆叠逻辑，其他复杂窗口自适应不足。
- 固定像素侧栏/表格列较多，小窗口下容易拥挤。
- 高级功能、日志、传输队列、历史记录等辅助信息常驻主界面，压缩主要任务空间。

---

## 3. 目标 UI 架构

### 3.1 UI 组件选型：gpui-component 优先

`gpui-component` 已经作为高层控件库引入，后续插件 UI 应优先使用它，尤其是有状态、有交互语义、需要一致行为的控件。沿用 `conventions.md` §7 的三层结构，但优先级调整为：

```text
1. gpui-component           首选：button、tab、form、badge/tag、switch、checkbox、slider、progress、table、编辑器、overlay 等交互控件
2. 项目 ui:: adapter/origin  当 gpui-component 默认效果不满足项目视觉时，包一层 adapter，统一 token、圆角、间距、hover/disabled/loading
3. 原生 GPUI div()          仅用于布局、容器、一次性简单元素，禁止用 div 手写库已提供的复杂控件
```

执行原则：

- **优先查 gpui-component**：按钮、tab/segmented、switch/checkbox、slider/progress、badge/tag、form 行、table、编辑器、overlay，默认先看组件库是否满足。
- **效果不满足就改造，不绕开**：通过主题覆盖、本地 adapter、项目级 wrapper 让组件服从 `theme::token` 和 `app::ui` 语义，而不是在插件 view 里继续手写新按钮/新 chip。
- **adapter 归属清晰**：单个插件试验可先放本地；两个及以上插件需要同类改造时，必须抽到 `app::ui` 或 `app::ui::components`。
- **布局仍可用 `div()`**：容器、flex/grid、一次性装饰可以用原生 GPUI；但可点击、可聚焦、可禁用、可加载、可选中的交互元素不应重复手写。
- **Root 限制要遵守**：需要 `gpui_component::Root` 的 overlay/dialog/input 能力，必须按窗口单独 Root 化；未 Root 化窗口只能使用安全组件或本地 adapter。

### 3.2 `app::ui` 需要补齐的原语

建议逐步补齐以下共享组件或 builder：

| 组件 / adapter | 优先底座 | 目的 | 替代现有重复 |
|---|---|---|---|
| `Button` | `gpui_component::button::Button` | 支持 `Primary / Secondary / Ghost / Danger`、`Small / Medium`、icon、disabled、loading、hover | 各插件本地 `primary_button/action_button/ghost_button/destructive_action_button` |
| `IconButton` | gpui-component button + icon adapter | 统一圆形/方形图标按钮，支持 badge count、active、disabled | `icon_button/top_bar_icon_button/row_icon_button` |
| `Chip` / `SegmentedControl` | gpui-component tag/tab/switch 类组件 | 统一过滤器、模式切换、tab-like 小按钮 | `mode_chip/filter_chip/segment_button/segmented_chip` |
| `StatusPill` | gpui-component badge/tag adapter | 统一状态标签，输入状态枚举或语义 tone | 各插件 `status_tag/status_chip/status_pill` |
| `LabeledField` | gpui-component form/input adapter | label + description + input + trailing action | `labeled_field/profile_field/editor_field/settings_field` |
| `InputShell` | gpui-component input 或项目 TextInput adapter | 单行输入框外壳、禁用态、错误态、focus ring | 各插件 `input_shell/search_input_shell/sheet_input` |
| `SectionCard` | 项目 `ui::` 容器 adapter | 标题、描述、右侧动作、内容区 | `settings_card/group_section/draft_section/profile_form_section` |
| `TableHeader` / `DataTableShell` | gpui-component table / 虚拟 list adapter | 表头、列宽、空态、滚动容器统一 | `table_header_cell/table_header_flex` 重复 |
| `EmptyState` | 项目 `ui::` 组合组件 | icon + title + description + action | 各插件本地 empty state |
| `OverlayHost` / `Sheet` / `Dialog` | gpui-component overlay/dialog（Root 化后）或本地兼容 adapter | 弹层尺寸、遮罩、关闭行为、底部按钮区统一 | 快速启动/API/FTP 各自 overlay |
| `Toolbar` | 项目 `ui::` 容器 + gpui-component actions | 标题、主动作、次动作、状态提示布局统一 | 各插件顶部栏 |

### 3.3 推荐页面结构

独立窗口插件统一为：

```text
PluginSurface
└─ PluginWindowLayout
   ├─ Header / Toolbar       标题、状态、1–3 个主动作
   ├─ Body                   主任务区域
   │  ├─ PrimaryPane         常用功能 / 当前任务
   │  ├─ SecondaryPane?      详情 / 预览 / 日志，可折叠
   │  └─ UtilityPane?        高级功能入口，不默认抢占空间
   └─ StatusBar?             轻量状态；不要承载关键操作
```

原则：

- 主操作靠近主内容，不放在窗口角落让用户找。
- 高级设置不默认展开；用“高级”“更多”“设置”入口。
- 详情、历史、日志、传输队列默认折叠或在右侧辅助栏，不遮挡主任务。
- 操作完成后的反馈放在固定位置：顶部 notice、底部 status、或对应行状态，不到处散落。

---

## 4. 插件级问题与优化建议

### 4.1 FTP/SFTP/SSH

当前特征：复杂独立窗口，包含连接列表、文件浏览、协议面板、终端/日志、传输队列、profile 编辑弹层、文件菜单、文件夹弹层。

主要问题：

- 视觉自成一套，存在较多 `glass/frost/shadow` 样式和硬编码颜色。
- 组件复用度最低，很多按钮、状态 pill、空态、输入框壳本地定义。
- 面板多，辅助信息（协议日志、传输队列、编辑器）容易挤压文件浏览主区域。
- 高级连接参数与常用连接流程需要更清晰分层。

优化方向：

1. **主路径重排**：左侧只保留连接搜索 + 连接列表 + 新建按钮；中间为文件浏览；右侧为“终端 / 日志 / 详情”可折叠辅助栏。
2. **传输队列收纳**：默认以底部 compact bar 展示正在传输数量和总体进度；点击展开完整队列。
3. **高级连接配置折叠**：新建/编辑 profile 中只默认展示协议、主机、端口、用户名、认证方式；高级项（代理、编码、被动模式、跳板机、路径策略）进入高级折叠区。
4. **统一样式**：替换本地 `frost_button/status_pill/meta_badge/empty_state_card/profile_field` 为 `app::ui` 原语。
5. **消除硬编码色**：状态色统一 `success/warning/danger/text_secondary`。

评分：合理性 3/5；组件复用度 1.5/5。

### 4.2 快速启动管理（quick_launch）

当前特征：列表 + 搜索 + 管理行 + 多个 sheet/overlay（动作菜单、删除确认、参数输入、编辑器、运行结果、历史）。

主要问题：

- 弹层使用多个互斥 `else if` 分支，状态管理脆弱。
- 键盘逻辑需要枚举所有弹层状态，后续新增弹层容易漏。
- 主界面同时承担搜索、运行、编辑、历史入口，信息密度偏高。
- 本地按钮/chip/status 组件多，和其他插件不一致。

优化方向：

1. **引入 `OverlayHost`**：把所有弹层抽象成 `ActiveOverlay` 枚举，例如 `ActionMenu/DeleteConfirm/Parameters/Editor/Result/History`，统一关闭、焦点、Esc 行为。
2. **主界面只保留常用路径**：搜索、运行、停止、编辑、更多。历史和高级参数进入右侧详情或弹层。
3. **编辑器分区**：基本信息、执行目标、参数、反馈策略、高级设置分组；高级设置默认折叠。
4. **运行结果轻量化**：成功/失败状态在列表行直接可见；详细 stdout/stderr 进入结果弹层或详情面板。
5. **统一组件**：替换本地 `primary_action_button/action_button/icon_action_button/destructive_action_button/segment_button/kind_chip/subtle_chip/status_chip`。

评分：合理性 3/5；组件复用度 2/5。

### 4.3 API 调试器

当前特征：集合树 + 多标签 + 请求编辑器 + 响应面板 + 环境弹层/环境管理器。是目前少数具备响应式堆叠逻辑的插件。

主要问题：

- 有独立“毛玻璃”视觉语言，与其他插件的卡片式界面差异大。
- 本地定义了 `primary_button/soft_button/icon_button/status_badge/method_badge/scenario_status_pill` 等。
- 环境管理与环境选择入口较多，需要明确“当前环境”和“管理环境”的关系。
- 大文本输入/响应区应与 `gpui-component` 编辑器/大文本策略对齐。

优化方向：

1. **保留好的响应式结构**：`STACK_BREAKPOINT_PX` 思路可推广到其他复杂窗口。
2. **收敛视觉语言**：决定是否保留毛玻璃。如果保留，应作为全局主题变体；否则改回 `SectionCard` + `Toolbar`。
3. **请求编辑器简化**：默认展示方法、URL、Headers、Body；Auth、Pre/Post Script、Variables 进入高级 tab。
4. **响应面板固定信息层级**：状态码、耗时、大小始终置顶；Body/Headers/Cookies/Timing 作为二级 tab。
5. **环境入口统一**：顶部只显示当前环境 selector + 管理按钮，环境管理使用统一 Dialog。

评分：合理性 3.5/5；组件复用度 2/5。

### 4.4 图片压缩

当前特征：顶部模式/质量、drop zone、图片表格、底部输出与批处理状态。

主要问题：

- 使用 emoji 作为标题/按钮图标。
- 本地定义多套按钮：`primary_button/secondary_button/ghost_button/action_button/quality_button`。
- 与下载管理器重复定义表头 helper。
- 质量调节用 `+/-` 按钮，缺少更直观的 slider 或 preset。
- 高级项（覆盖原图、输出目录、格式策略）应更明确归入设置区。

优化方向：

1. **常用路径**：拖入/选择/粘贴图片 → 选择模式 → 开始压缩。
2. **高级设置入口**：输出目录、覆盖原图、质量、格式保留/转换进入“设置”面板；主界面只显示当前摘要。
3. **统一表格**：与下载管理共用 `DataTableShell` / `TableHeader`。
4. **批量状态清晰化**：底部展示总数、成功、失败、节省空间、正在处理；失败行提供重试。
5. **替换 emoji 图标**：用 SVG 图标。

评分：合理性 3/5；组件复用度 2/5。

### 4.5 下载管理器

当前特征：URL 输入、任务列表、过滤、设置面板、下载状态持久化。

主要问题：

- 组件复用相对较好，但仍有本地 `action_button/filter_chip/settings_field/table_header_*`。
- 表格列宽与按钮尺寸需要统一响应式策略。
- 设置面板应与系统设置/其他插件的设置分组共用样式。

优化方向：

1. **主流程简化**：URL 输入 + 新建下载作为唯一主动作；过滤和设置进入二级区域。
2. **任务表格统一**：与图片压缩共用 table 原语；状态 tag 使用统一 `StatusPill`。
3. **设置收纳**：并发数、重试、默认目录、自定义 header 等进入设置抽屉。
4. **批量操作入口**：暂停全部/继续全部/清理完成放入 toolbar 的“更多”。

评分：合理性 4/5；组件复用度 3/5。

### 4.6 二维码

当前特征：输入区 + 预览区 + 扫描面板 + 历史面板。

主要问题：

- 本地按钮过多：`primary_action_button/action_button/utility_button/icon_button/ghost_button`。
- 扫描、历史、生成、保存、复制、粘贴同时出现在主界面，按钮密度较高。
- 历史面板开合可能挤压主预览，应固定为右侧抽屉或下方折叠区。

优化方向：

1. **主路径**：输入内容 → 自动/手动生成 → 复制/另存为。
2. **扫描和历史二级入口**：放到 toolbar 的次级按钮，打开右侧 drawer 或 sheet。
3. **空态明确**：无输入时预览区展示 icon + “输入文本后生成二维码”。
4. **按钮统一**：所有按钮替换为共享 Button / IconButton。

评分：合理性 3.5/5；组件复用度 2/5。

### 4.7 HTTP 抓包

当前特征：顶部状态与操作、左侧过滤、列表、详情 tab。

主要问题：

- 使用 emoji 作为抓包图标。
- 左侧过滤栏固定宽度，窄窗口可能挤压详情区。
- disabled 通过 `.opacity(0.4)` 模拟，缺少真正 disabled 语义。
- 存在重复/无意义分支，应顺手清理。

优化方向：

1. **过滤器可折叠**：窄窗口下过滤栏变成顶部 filter bar 或 drawer。
2. **详情优先级**：请求概览、Headers、Body、Timing tab 使用统一 tab/section 样式。
3. **禁用态统一**：按钮使用统一 disabled 状态，不允许点击。
4. **图标替换**：emoji 替换为 SVG。

评分：合理性 3.5/5；组件复用度 3/5。

### 4.8 JSON 解析器

当前特征：输入/输出、格式化/压缩/校验等工具栏按钮。

主要问题：

- 体量较小，但仍有本地 `mode_button/toolbar_button/module_header`。
- 编辑器类输入应与大文本/代码输入策略统一。

优化方向：

1. **工具栏统一**：模式切换用 `SegmentedControl`，动作按钮用共享 Button。
2. **错误定位**：校验失败时在固定 notice 区展示错误摘要，后续可支持定位。
3. **输入输出布局**：窄窗口上下堆叠，宽窗口左右分栏。

评分：合理性 4/5；组件复用度 3/5。

### 4.9 剪贴板历史

当前特征：历史列表、详情预览、设置 tab。

主要问题：

- 文件拆分合理，但 `history/settings/shared` 内还有本地 `header_action_button/theme_button/search_field/input_shell/settings_row`。
- 历史列表、详情预览和设置页的按钮/输入框风格与其他插件不完全统一。

优化方向：

1. **历史主路径**：搜索/筛选/复制/固定/删除保持可见。
2. **详情面板紧凑**：大预览内容可滚动，操作固定在面板顶部或底部。
3. **设置页复用 `SettingsSection/SettingsRow`**：与系统设置共用样式。
4. **按钮替换**：header action 和 theme button 改共享 Button / IconButton。

评分：合理性 4/5；组件复用度 2.5/5。

### 4.10 系统设置

当前特征：多个设置卡片，结构清晰，是目前最接近目标的插件。

主要问题：

- `settings_card/settings_row/action_button/seg_button/badge` 都是本地好实现，但没有抽到共享层。
- 平台相关设置需要条件展示；例如 Windows 环境下不应突出显示 macOS 权限。

优化方向：

1. **抽公共设置组件**：把 `settings_card/settings_row` 迁移到 `app::ui`，供剪贴板、下载、FTP profile 编辑等复用。
2. **平台条件渲染**：macOS 权限只在 macOS 下显示；其他平台显示对应能力或隐藏。
3. **设置项状态统一**：启用/禁用/未实现/错误/成功使用统一 `StatusPill`。

评分：合理性 4.5/5；组件复用度 3.5/5。

### 4.11 关于页

当前特征：简单静态信息页。

主要问题：

- 本地重新定义了 `section_card`，与 `app::ui::section_card` 重名，容易混淆。

优化方向：

1. **直接复用 `ui::section_card` / `ui::page_title`**。
2. **技术栈和说明列表使用统一 `KeyValueRow` / `InfoRow` 原语**。

评分：合理性 4/5；组件复用度 3/5。

---

## 5. 统一交互规范

### 5.1 按钮

按钮必须具备：

- `variant`：Primary、Secondary、Ghost、Danger。
- `size`：Small、Medium，必要时 Large。
- `state`：enabled、hover、active、disabled、loading。
- 可选 icon，icon 来自 SVG。
- disabled 状态必须不可点击，不只是不透明度降低。

建议用法：

- 每个页面最多一个 Primary 主按钮。
- 危险动作使用 Danger，且必要时二次确认。
- 行内低频动作使用 Ghost/IconButton。
- toolbar 中常用动作不超过 3 个，其余进入“更多”。

### 5.2 弹层 / 抽屉 / 菜单

统一分为：

| 类型 | 用途 | 规则 |
|---|---|---|
| Popover/Menu | 轻量选择、更多操作 | 不承载复杂表单；点击外部关闭 |
| Sheet/Drawer | 历史、详情、高级设置、较长表单 | 不遮住整个主界面；可滚动 |
| Dialog | 删除确认、危险操作、必须中断确认 | 文案短，主/次按钮明确 |
| Full overlay | 参数输入、编辑复杂动作等必须聚焦任务 | 必须有标题、关闭、底部操作区 |

快速启动、API、FTP 应优先引入统一 `ActiveOverlay` / `OverlayHost`，避免多个 `if/else` 分支互相遮挡。

### 5.3 空态、加载态、错误态

所有列表/详情/预览区域必须有：

- Empty：无数据时说明下一步。
- Loading：后台任务进行中。
- Error：失败原因 + 可恢复动作。
- Permission：权限不足时给出打开设置/重试入口。

统一结构：

```text
Icon
Title
Description
Primary action? / Secondary action?
```

### 5.4 高级功能入口

高级功能不能散落在主界面，应统一：

- 入口名：优先“高级”“设置”“更多”。
- 位置：toolbar 右侧、section header 右侧、或详情页 tab。
- 行为：默认折叠；用户展开后可记忆，但首次打开保持简洁。
- 高级项必须说明影响范围，尤其覆盖文件、代理、脚本、批量删除等高风险操作。

### 5.5 复杂窗口紧凑策略

复杂独立窗口至少支持三种密度策略：

1. **宽窗口**：侧栏 + 主区域 + 辅助栏。
2. **中等窗口**：侧栏可折叠，辅助栏变 tab。
3. **窄窗口**：主区域优先，侧栏/辅助栏变 drawer 或上下堆叠。

不要只靠固定像素宽度。可参考 API 调试器的 breakpoint 思路，并提炼为共享 helper。

---

## 6. 分阶段实施路线

### 阶段 0：文档与基线

- [x] 记录当前问题和优化目标。
- [ ] 建立 UI 审查清单，后续 PR 按清单检查。
- [ ] 明确 `app::ui` 中保留哪一代原语，标记 legacy。

### 阶段 1：低风险 token 收敛

目标：不改变布局，只统一基础视觉。

- [ ] 根字体统一为 `ui::font_ui()`。
- [ ] mono 字体统一为 `ui::font_mono()`。
- [ ] feature/view 中移除裸 `rgb(0x...)` / palette 直接调用。
- [ ] launcher 专用颜色从插件 view 中移除，换通用语义 token。
- [ ] 圆角/字号/间距收敛到 token。
- [ ] 清理明显死分支，如 `if dark { X } else { X }` 两边相同。

### 阶段 2：组件原语升级

目标：先把基础组件做对，再迁移插件。这里的“原语”优先是 **gpui-component adapter**，不是继续手写 `div()` 控件。

- [ ] 梳理 gpui-component 当前可用控件清单：button、tab、badge/tag、checkbox/switch、slider/progress、table、input/editor、overlay/dialog。
- [ ] 基于 `gpui_component::button::Button` 新增/改造统一 Button adapter。
- [ ] 基于 gpui-component 或其 adapter 新增 IconButton、Chip/SegmentedControl、StatusPill、LabeledField、EmptyState。
- [ ] 新增 SectionCard/Toolbar 这类项目布局原语，内部 action 优先使用 gpui-component adapter。
- [ ] 新增 TableHeader/DataTableShell，优先基于 gpui-component table / 虚拟 list，先服务下载管理与图片压缩。
- [ ] 新增 OverlayHost/Sheet/Dialog：Root 化窗口后优先用 gpui-component overlay/dialog；Root 化前提供兼容 adapter。
- [ ] 补齐 hover/active/disabled/loading 状态，并保证 disabled 不可点击。

### 阶段 3：样板插件迁移

建议顺序：

1. **图片压缩**：范围适中，按钮和表格重复明显，适合作为 button/table 样板。
2. **二维码**：按钮/历史/扫描入口适合作为“常用简洁 + 高级入口”的样板。
3. **下载管理器**：与图片压缩共用 table，验证共享组件可复用。
4. **系统设置 / 剪贴板设置**：抽出 settings section/row。

完成标准：

- 本地 `primary_button/action_button/ghost_button/status_tag/table_header_*` 删除。
- emoji 图标替换为 SVG。
- 主界面常用路径更清晰，高级功能入口明确。

### 阶段 4：复杂窗口重构

建议顺序：

1. **快速启动管理**：先引入 `OverlayHost`，解决弹层互斥和键盘逻辑脆弱问题。
2. **FTP/SFTP/SSH**：重排主路径、折叠辅助面板、统一颜色与组件。
3. **API 调试器**：决定毛玻璃是否保留；若保留，提为全局主题变体；否则收敛到卡片式视觉。

完成标准：

- 主任务无遮挡。
- 高级功能默认收纳。
- 窄窗口可正常使用。
- 日志/历史/传输/环境管理不抢占主流程。

### 阶段 5：验收与回归

- [ ] `cargo fmt`
- [ ] `cargo check`
- [ ] 重点插件手动验证：打开窗口、常用动作、弹层关闭、窄窗口、暗/亮主题。
- [ ] 逐插件截图或手动记录，确保无遮挡、按钮状态正确、空态清晰。

---

## 7. UI 审查清单

每次修改插件 view 前后检查：

### 视觉一致性

- [ ] 是否使用 `ui::plugin_surface` / `ui::font_ui` / `ui::font_mono`？
- [ ] 是否没有裸 `rgb(0x...)`、palette 直接调用、launcher 专用颜色？
- [ ] 是否使用统一 radius/spacing/font-size？
- [ ] 是否没有 emoji 作为图标？

### 组件复用

- [ ] 新增交互控件前是否先查过 gpui-component？
- [ ] gpui-component 默认效果不满足时，是否通过 adapter/wrapper 改造，而不是手写第三套？
- [ ] 是否没有新建本地 `*_button` / `*_chip` / `*_badge` / `*_card`？
- [ ] 如果本地 adapter/helper 出现第二次，是否抽到 `app::ui` / `app::ui::components`？
- [ ] 表格/大列表是否使用 gpui-component table、虚拟 list 或明确分页？
- [ ] 输入框/设置行/状态标签是否复用 gpui-component 或共享 adapter？

### 交互可用性

- [ ] 主流程是否一眼可见？
- [ ] 高级功能是否被收纳到明确入口？
- [ ] 弹层是否可关闭、不会遮挡关键操作？
- [ ] disabled/loading/error/empty 状态是否清晰？
- [ ] 键盘路径是否仍可用，尤其 `Esc`、`Enter`、上下选择？

### 复杂窗口

- [ ] 主区域是否优先展示当前任务？
- [ ] 辅助栏/日志/历史/传输队列是否可折叠？
- [ ] 窄窗口是否不会出现关键操作被挤出或遮挡？
- [ ] 常用动作是否不超过 1 个主按钮 + 少量次按钮？

---

## 8. 近期推荐任务拆分

1. **基于 gpui-component 新增统一 Button adapter，并迁移图片压缩与二维码按钮**
   - 低风险，可立刻改善 hover/disabled/尺寸不一致。
   - 不满足项目视觉时改造 adapter，不在插件里继续手写按钮。

2. **基于 gpui-component table / 虚拟 list 新增 DataTableShell，合并图片压缩和下载管理表头**
   - 解决复制粘贴，统一表格密度。

3. **新增 SettingsSection/SettingsRow，从系统设置抽公共组件**
   - 系统设置当前结构最好，适合作为共享样板。

4. **新增 OverlayHost，先迁移 quick_launch**
   - 解决遮挡和互斥弹层问题，为复杂插件铺路。

5. **FTP/SFTP/SSH 视觉 token 收敛**
   - 移除硬编码色和本地 frost/button/pill，降低后续重排难度。

---

## 9. 不建议做的事

- 不建议一次性重写所有插件 UI；风险高，难验证。
- 不建议为了“现代化”继续增加新的毛玻璃/阴影体系；应先统一基础交互和组件。
- 不建议把所有 `div()` 都替换成 `gpui-component`；布局容器仍可用 `div()`。
- 不建议因为 gpui-component 默认样式不完全匹配就绕开不用；应优先通过主题覆盖、adapter 或 wrapper 改造。
- 不建议在插件内继续新增本地按钮/chip/card/helper；需要新视觉先补 gpui-component adapter 或 `app::ui`。
- 不建议把高级设置直接铺满主界面；应收纳到明确入口。

---

## 10. 总结

当前界面的核心问题不是某个插件“丑”，而是：

1. 项目已经有设计系统雏形，但插件没有稳定执行。
2. 常用/高级功能边界不清，复杂插件容易拥挤或被弹层遮挡。
3. 按钮、chip、表格、输入框、空态、图标、字体、圆角都存在多套实现。

后续应以“**无遮挡、正常使用、常用简洁、高级可配置、复杂窗口紧凑现代化**”为验收目标，先补共享组件，再逐插件迁移。每次只迁一个插件或一类组件，保证可验证、可回退。
