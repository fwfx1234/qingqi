# Qingqi 当前代码优化审计报告

> 日期：2026-06-06  
> 范围：当前 `F:\develop\qingqi` workspace，只读审计源码与文档；仅新增本报告。  
> 方法：本地扫描 + 4 个子 agent 并行分析（UI、插件设计、代码质量、AGENT/docs）。  
> 验证：`cargo check --workspace` 通过；依赖边界体检通过；当前仍有既有未提交改动，未回滚。

## 0. 当前状态摘要

仓库已经完成多 crate workspace 拆分，核心结构是：

- `qingqi-plugin`：插件 SDK 契约。
- `qingqi-core`：插件宿主、注册器、命令缓存与排序。
- `qingqi-app`：应用运行时、窗口控制、启动器、后台集成。
- `qingqi-ui`：主题、共享 UI、TextInput、资源。
- `qingqi-platform`：系统能力封装。
- `qingqi-feature-*`：每个内置插件一个 crate。
- `qingqi`：bin crate，组合内置插件注册。

边界体检结果：

- `cargo tree -p qingqi-app | rg "qingqi-feature"`：无 feature 依赖。
- `cargo tree -p qingqi-core | rg "qingqi-feature"`：无 feature 依赖。
- `cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)"`：无上层依赖。
- `cargo check --workspace`：通过，但存在 warning 和 `russh v0.54.5` future-incompat 提示。

当前工作区已有未提交改动：

- `crates/qingqi-app/src/app/runtime.rs`
- `crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs`
- `crates/qingqi-feature-qr-code/src/view.rs`

本报告没有回滚或修改这些文件。

## 1. 优先级总览

| 优先级 | 主题 | 结论 | 建议动作 |
|---|---|---|---|
| P0 | `AGENT.md` / `README.md` 过期 | 文档仍写 pre-split 和旧 `src/*` 路径，会误导后续 agent | 先更新 AGENT/README，再推进重构 |
| P1 | 插件契约显式化 | 视图模式、能力权限、动态命令刷新多数靠隐式约定 | 加运行时校验、capability 声明、命令变更模型 |
| P1 | 共享 UI 组件不足 | `qingqi-ui` 有雏形，但按钮/chip/overlay/status 覆盖不够，feature 继续复制 | 先补共享组件族，再迁移插件 |
| P1 | 大型 view 拆分 | 多个 `view.rs` 超过 1000-4500 行，职责混杂 | 按 toolbar/list/detail/overlay/editor 拆模块 |
| P2 | 后台任务与锁模型 | 下载、API、FTP runtime 混用裸线程、blocking client、`Arc<Mutex<_>>` | 收敛 executor/job 生命周期与 lock 策略 |
| P2 | Manifest/注册重复 | Manifest/VisualSpec/前缀/注册模板重复 | builder 或宏化，减少漂移 |
| P3 | 存储迁移与依赖治理 | schema 迁移风格分散，少量依赖没收进 workspace | 轻量 migration helper；统一 workspace dependencies |
| P3 | warning 清理 | check 通过但 warning 可作为低风险清理 | 分 crate 消除 unused/dead code |

## 2. UI 可拆分与共享组件优化

### 2.1 先补齐 `qingqi-ui` 的组件能力

`qingqi-ui` 已有共享组件：

- `crates/qingqi-ui/src/ui/components/button.rs`
- `crates/qingqi-ui/src/ui/components/chip.rs`
- `crates/qingqi-ui/src/ui/components/status_pill.rs`
- `crates/qingqi-ui/src/ui/components/empty_state.rs`
- `crates/qingqi-ui/src/ui/components/settings.rs`
- `crates/qingqi-ui/src/ui/components/overlay_host.rs`
- `crates/qingqi-ui/src/ui/components/table_header.rs`

但它们目前更像“基础 helper”，覆盖不了插件真实场景，导致 feature 层继续重写：

- 图片压缩：`mode_chip`、`primary_button`、`secondary_button`、`ghost_button`、`action_button`。
- 快速启动：`primary_action_button`、`action_button`、`icon_action_button`、`destructive_action_button`、多种 chip。
- JSON 解析：`secondary_button`、`mode_pill`、`query_execute_button`。
- HTTP 抓包：一整段本地 chip / tab helper。
- 下载管理器：`filter_chip`、`action_button`、`settings_field`。

建议新增或升级这些共享组件：

- `Button`：`Primary / Secondary / Ghost / Danger`，`Small / Medium`，支持 icon、loading、disabled、destructive。
- `IconButton`：统一图标按钮尺寸、hover、active、disabled。
- `Chip / SegmentedControl`：用于过滤器、模式切换、tab-like 小按钮。
- `StatusPill`：按 `Success / Warning / Danger / Info / Neutral` tone 映射样式。
- `LabeledField / InputShell`：统一 label + input + description + error。
- `OverlayHost / MenuOverlay / Sheet / Dialog`：统一 backdrop、Esc、点击外部关闭、底部操作区。
- `DataTableShell`：统一表头、空态、滚动区域、列宽策略。

落地顺序建议：

1. Button + IconButton + StatusPill。
2. Chip / SegmentedControl。
3. OverlayHost 扩展。
4. DataTableShell。
5. SettingsSection / SettingsRow 归一化。

### 2.2 大 view 文件应拆分

当前最大文件：

| 文件 | 行数 | 建议拆分 |
|---|---:|---|
| `crates/qingqi-feature-api-debugger/src/view.rs` | 4569 | `collection_tree`、`request_editor`、`response_panel`、`environment_dialog`、`overlays`、`kv_editor` |
| `crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs` | 3688 | `sidebar`、`remote_browser`、`profile_editor`、`terminal_panel`、`transfer_panel`、`overlays` |
| `crates/qingqi-feature-quick-launch/src/view.rs` | 3285 | `action_list`、`editor`、`history`、`parameters`、`result_overlay`、`overlays` |
| `crates/qingqi-feature-image-compress/src/view.rs` | 2426 | `toolbar`、`drop_zone`、`image_table`、`settings`、`batch_status` |
| `crates/qingqi-feature-download-manager/src/view.rs` | 1708 | `task_table`、`filters`、`settings_overlay`、`task_actions` |
| `crates/qingqi-feature-system-settings/src/view.rs` | 1625 | `theme_section`、`shortcut_section`、`permission_section`、`cache_section`、`diagnostics_section` |
| `crates/qingqi-feature-http-capture/src/view.rs` | 1270 | `capture_table`、`filter_bar`、`detail_tabs`、`mock_panel` |

拆分原则：

- 一个主 `View` 保留状态与总体 render。
- 子模块负责一类区域：toolbar、list/table、detail、overlay、editor、settings。
- 纯转换逻辑移到可测试模块，例如状态文案、表单解析、格式化。
- 不跨 feature 复用模块；要共享就下沉到 `qingqi-ui` 或 `qingqi-plugin`。

### 2.3 插件级 UI 改造建议

API Debugger：

- 保留已有响应式 breakpoint 思路。
- 拆出 request editor、response panel、environment manager、collection tree。
- `overlay_shell`、`status_badge`、`method_badge`、KV table 等迁向共享组件。
- Auth、Scripts、Variables 默认收进高级 tab，主路径保留 method + URL + headers/body + send。

FTP/SFTP/SSH：

- 左侧连接列表、中间文件浏览、右侧终端/日志/详情可折叠。
- 传输队列默认底部 compact bar，点击展开。
- profile editor 拆出独立模块，常用字段和高级字段分组。
- 收敛 `glass_panel`、本地 `toolbar_button`、`empty_state`、各种 overlay。

Quick Launch：

- 引入统一 `ActiveOverlay` + `OverlayHost`，替代多个互斥 overlay 分支。
- action 列表、编辑器、参数输入、历史、运行结果分模块。
- 主界面只保留搜索、运行、编辑、更多；历史和高级参数收纳。

Image Compress：

- 用共享 Button/Chip/DataTable 替换本地按钮和表格。
- 质量控制改为 slider 或 preset，输出策略进入设置。
- 批量状态固定展示总数、成功、失败、节省空间、当前处理。

Download Manager：

- 任务表格与图片压缩共用 `DataTableShell`。
- 设置抽屉复用 `SettingsSection/SettingsRow`。
- 批量操作进入 toolbar “更多”，主动作保留新建下载。

Clipboard / System Settings：

- Clipboard 已有 `view/history.rs`、`view/settings.rs` 拆分，方向正确。
- settings 组件应从系统设置抽到 `qingqi-ui`，供剪贴板、下载、FTP profile 复用。

About：

- 本地 `section_card`、`tech_row`、`desc_row` 可替换为共享 info/section 原语。

## 3. 插件设计优化

### 3.1 视图模式要从 debug 约束变成运行时约束

当前 `PluginManager::open()` 会对 `manifest.mode` 和 `PluginView::mode()` 做 `debug_assert_eq!`，但 release 下失配不会失败。

建议：

- 最小改法：把 debug assert 升级为运行时错误。
- 更长期：拆成强类型 open API，例如 `open_window_view` 只允许窗口插件，注册时就固化 view mode。
- 插件构建测试中增加“manifest mode 与实际 view 一致”的通用断言。

收益：

- 避免 release 下窗口/列表/inline 插件失配导致后续 downcast 或 UI 生命周期异常。

### 3.2 权限/能力边界显式化

现在插件能否使用主题、快捷键、AppIndex、数据库、路径、事件，主要取决于注册时给它传了什么 handle。这个模型对内置插件可用，但未来外部插件或更多内置插件加入后，可读性和审计性会下降。

建议在 `Manifest` 或 `PluginDescriptor` 增加 capability 声明：

- `Database(Vec<DatabaseSpec>)`
- `StoragePath`
- `Clipboard`
- `Shortcut`
- `Theme`
- `AppIndex`
- `Network`
- `Shell`
- `Background`
- `GlobalHotkey`

注册器根据 capability 注入 host handle，并在启动日志中记录插件能力。

### 3.3 注册流程改为两阶段或具备回滚策略

当前 `FeatureRegistry::build_all()` 对每个 entry 依次注册数据库再 build 插件；中途失败会留下部分已注册数据库和部分已注册插件状态。`PluginSource` 也还没有驱动 builtin/external 差异。

建议：

- 两阶段：先校验所有 descriptor/database/capability，再统一 build/register。
- 或增加失败回滚/错误报告：明确已注册哪些、失败在哪个插件。
- `PluginSource` 要么删除，要么用于 external 插件路径、权限策略、日志标记。

### 3.4 Manifest / VisualSpec / 前缀重复收敛

当前 `Manifest` 和 `PluginVisualSpec` 重复持有 `icon/category/status/mode/window` 等信息，`background` 与 `status=Background` 也存在双写风险。`prefixes` 与 `command_prefixes` 两套前缀语义也容易让插件作者困惑。

建议：

- 引入 `ManifestBuilder` 或声明宏，统一生成 manifest、visual、默认 open command。
- `PluginVisualSpec` 尽量从 `Manifest` 派生，只保留额外 UI 展示字段。
- 明确或合并 `prefixes` / `command_prefixes`：
  - 若需要区分，命名为 `open_prefixes` 与 `action_prefixes`。
  - 若不需要区分，保留单一 prefixes 源。

### 3.5 动态命令刷新模型更细化

当前 `AppEventKind` 只有 `FeatureChanged / CommandsChanged / JobsChanged`，`CommandsChanged` 没有差分信息；`PluginManager` 的命令缓存主要靠插件自己发布事件来触发失效。

建议：

- `CommandsChanged { plugin_id, revision }` 显式化。
- dynamic command provider 增加 `commands_revision()`，launcher 可按 revision 刷新。
- `PluginCx` 提供统一 `invalidate_commands()` 或 `notify_commands_changed()` 已有接口的强约束版本。

## 4. 通用代码优化

### 4.1 后台任务模型收敛

下载管理器：

- `DownloadService` 持有多个 `Arc<Mutex<_>>` 与 `reqwest::blocking::Client`。
- 每个下载直接 `thread::spawn`，后台线程里多处 `store.lock().unwrap()` 更新 DB。

建议：

- 改为有限并发 worker 或 Tokio blocking pool。
- 暂停/取消/失败恢复统一成 job 状态机。
- 锁获取改用 `lock_or_recover` 或返回错误，避免 poisoned lock 直接 panic。

API Debugger：

- 大量 CRUD/import/send 直接 `thread::spawn`。
- 每次请求重建 blocking client；文件 body 使用同步 `std::fs::read`。

建议：

- 抽 feature 级 executor/job runner。
- 复用 HTTP client。
- 将请求、导入、文件读取放到受控阻塞池。
- 快速连续点击/导入/发送增加并发回归测试。

FTP/SFTP/SSH：

- runtime 混合 `thread::spawn`、Tokio runtime、同步锁与 mpsc 事件。

建议：

- transfer、terminal、session state 拆边界。
- runtime 统一持有 JoinHandle / shutdown token。
- 事件总线考虑 broadcast/channel，关闭 session 时验证线程退出。

### 4.2 平台层全局状态加强约束

平台层存在全局状态：

- tray 使用 `static mut CURRENT_TRAY`。
- hotkey 使用 `OnceLock<Mutex<Vec<HotKey>>>`。

建议：

- tray 状态封装为主线程 owner，或改 `OnceLock<Mutex<Option<TrayIcon>>>`。
- 明确 API 必须主线程调用。
- 增加重复 install/uninstall、退出、托盘重建测试。

### 4.3 存储 migration helper

当前多个 store 自己处理 schema：

- API Debugger 有 `SCHEMA_VERSION`。
- Download Manager 有 store version。
- Quick Launch 直接 `ALTER TABLE`。
- Clipboard 有 FTS rebuild 逻辑。

建议沉淀轻量 migration helper：

- 统一版本表。
- 幂等 `ALTER TABLE`。
- migration 失败回滚。
- 旧 schema fixture 测试。

### 4.4 Cargo 依赖统一

少量依赖仍是 crate-local 版本：

- `qingqi-feature-http-capture`：`http-body-util`、`hyper`、`hyper-util`、`rustls`、`tokio-rustls`。
- `qingqi-feature-download-manager`：`urlencoding`。

建议：

- 对跨 crate 或核心网络/TLS 依赖上收到 workspace dependencies。
- 调整后跑 `cargo tree -d`、`cargo check --workspace`。

### 4.5 Warning 清理

`cargo check --workspace` 通过，但 warning 包括：

- `qingqi-platform`：unused mut、dead code、未读字段、未用函数。
- `qingqi-feature-api-debugger`：unused variable、未用方法。
- `qingqi-feature-http-capture`：未读字段。
- `russh v0.54.5` future-incompat 提示。

建议：

- 先清理 warning，不改变行为。
- 跑 `cargo report future-incompatibilities --id 1` 看 russh 风险。
- 后续引入 `cargo clippy --workspace --all-targets` 作为高风险改动验收。

## 5. `AGENT.md` 优化建议

`AGENT.md` 是当前最需要先修的文档，因为它会直接影响后续 agent 的判断。

### 5.1 更新架构现状

问题：

- 仍写 `current pre-split codebase`。
- 仍引用旧路径 `src/app`、`src/core`、`src/platform`、`src/features`。
- 但 `docs/workspace-split-guide.md` 已标明 P0-P8 完成，实际仓库也已是 workspace。

建议改为当前 crate 边界：

- `crates/qingqi-app/src/app/runtime.rs`：bootstrap、tracing、paths、stores、plugin manager、菜单/动作、后台启动。
- `crates/qingqi-app/src/app/window_controller.rs`：launcher/plugin window 生命周期。
- `crates/qingqi-app/src/app/background.rs`：app-level platform/background loops。
- `crates/qingqi-core`：`PluginManager`、`FeatureRegistry`、`CommandUsageStore`、命令排序。
- `crates/qingqi-plugin`：SDK trait、Manifest、Command、events、storage、host handles。
- `crates/qingqi-ui`：theme/token/components/text input/assets。
- `crates/qingqi-platform`：OS APIs，不依赖 feature UI。
- `crates/qingqi-feature-*`：插件实现，依赖 `qingqi-plugin` + `qingqi-ui`，不依赖 app/core/其它 feature。
- `crates/qingqi`：bin 组合根和 builtin registry。

### 5.2 加入依赖边界体检命令

建议 AGENT 写入：

```bash
cargo tree -p qingqi-app | rg "qingqi-feature"
cargo tree -p qingqi-core | rg "qingqi-feature"
cargo tree -p qingqi-plugin | rg "qingqi-(core|app|platform|feature)"
rg -n "\bqingqi_feature_" crates/qingqi-core crates/qingqi-app
rg -n "\bqingqi_app::|\bqingqi_platform::|\bqingqi_feature_" crates/qingqi-plugin
```

期望均无不合理输出。

### 5.3 更新测试与交付命令

当前 AGENT 只写 `cargo fmt` 和 `cargo check`。建议升级为：

- 常规交付：
  - `cargo fmt --all`
  - `cargo check --workspace`
  - 相关 crate 的 `cargo test -p <crate>`
- 高风险/跨 crate 改动：
  - `cargo test --workspace -j 1 --quiet`
  - `cargo clippy --workspace --all-targets`
- Windows 上全量测试推荐 `-j 1`，避免 pagefile/编译资源波动误判。

### 5.4 补齐代码风格硬约定

建议加入：

- 命名：`XxxPlugin`、`XxxView`、`XxxViewModel`、`XxxService`、`XxxStore`、`XxxSnapshot`。
- 新代码避免随意新增 `Panel/Element/Session` 这类模糊命名。
- 非测试代码避免 `unwrap/expect`，错误带上下文。
- tracing 用结构化字段。
- 新增 warning 不应交付，workspace lints 里 `unwrap_used = warn`、`todo = warn`、`dbg_macro = deny`。

### 5.5 补齐 UI / GPUI 规则

建议加入：

- UI 控件优先级：`gpui-component` 优先，项目 adapter 次之，原生 `div()` 只做布局。
- 当前窗口未 Root 化时，不使用需要 `gpui_component::Root` 的 dialog/sheet/notification/focused input API。
- feature/view 禁止新增裸 `rgb(0x...)`、直接 palette、launcher 专用 token。
- 字体统一 `ui::font_ui()` / `ui::font_mono()`。
- 图标走 SVG 和 `ui::icon_element` / `ui::icon_tile`，不要新增 emoji 图标。
- 新代码不要到处传 `dark: bool`，语义 token 内部处理主题。

### 5.6 加入 UI 审查清单

建议从 `docs/plugin-ui-optimization-plan.md` 摘要进 AGENT：

- 主流程无遮挡。
- 高级功能收纳到设置/更多/高级。
- empty/loading/error/permission 状态完整。
- disabled 真不可点击，不只是 opacity。
- 复杂窗口支持宽/中/窄布局。
- 新增本地 button/chip/badge/card 前，先判断是否应进入 `qingqi-ui`。

### 5.7 加入多 agent / 脏工作区规则

建议加入：

- 开工先 `git status --short`。
- 多 agent 并行时按模块分配只读/写入范围。
- 不回滚别人未提交改动。
- 修改前先读相关文件和 docs。
- 不重排无关文件，不 stage unrelated dirty files。
- 每个子任务记录验证命令和未验证风险。

## 6. README / docs 整合建议

README 目前也过期：

- 仍写平台能力在 `crates/qingqi/src/platform`。
- 仍写 `crates/qingqi` 承载 app/core/features/platform 主体。
- 仍列 `crates/qingqi/src/app`、`crates/qingqi/src/core`、`crates/qingqi/src/platform`。

建议：

- README 更新为当前多 crate 结构。
- `docs/gpt-5.4-workspace-split-execution-plan.md` 标注为“历史执行手册 / 复盘 / 低优先级清理参考”，不要与当前日常规范并列为主入口。
- `docs/conventions.md` 保留硬规则。
- `docs/plugin-ui-optimization-plan.md` 保留 UI migration backlog 和插件级任务，不重复承载硬规范。
- `docs/gpui-component-guide.md` 作为 UI 改动专用验证与 Root 限制说明。

## 7. 推荐实施路线

### 阶段 A：文档先校准

1. 更新 `AGENT.md` 为当前 workspace 架构。
2. 更新 `README.md` 当前项目结构。
3. 在 AGENT 加入依赖边界、测试命令、UI 规则、多 agent 规则。

验收：

- 新 agent 不会再按旧 `src/*` 路径规划任务。
- README 与 `docs/workspace-split-guide.md` 一致。

### 阶段 B：共享 UI 基建

1. 补齐 `Button / IconButton / StatusPill / Chip`。
2. 扩展 `OverlayHost`。
3. 新增 `DataTableShell` 和 `SettingsSection/SettingsRow`。

样板迁移：

- 先迁移图片压缩和下载管理器，验证 Button/Table。
- 再迁二维码和系统设置，验证 Settings/Overlay。

### 阶段 C：插件契约显式化

1. `PluginManager::open()` 运行时校验 view mode。
2. 增加 ManifestBuilder，减少 visual/prefix/background 双写。
3. 明确 capability/permission。
4. 改良 CommandsChanged / dynamic command revision。

### 阶段 D：大型 view 拆分

1. Quick Launch：先拆 overlay 与 editor。
2. API Debugger：拆 request/response/environment/collection。
3. FTP/SFTP/SSH：拆 sidebar/browser/terminal/transfer/profile editor。

### 阶段 E：后台任务与存储

1. 下载管理器 worker/job 化。
2. API Debugger executor/client 复用。
3. FTP runtime shutdown/JoinHandle 可观测。
4. migration helper 与旧 schema 测试。

## 8. 建议的近期任务清单

1. `docs: refresh AGENT and README for split workspace`
2. `ui: add shared Button/IconButton/StatusPill variants`
3. `ui: migrate image-compress buttons and table shell`
4. `ui: migrate download-manager table and settings rows`
5. `plugin: enforce manifest view mode at runtime`
6. `plugin: introduce ManifestBuilder and clarify prefixes`
7. `quick-launch: centralize overlays with ActiveOverlay`
8. `api-debugger: split request editor and response panel modules`
9. `download-manager: replace per-download thread spawning with bounded worker`
10. `storage: add migration helper and old-schema tests`

## 9. 结论

Qingqi 的核心架构已经从单体迁到了合理的多 crate workspace，依赖方向目前是健康的。下一阶段最值得做的不是继续大规模拆 crate，而是把“隐式约定”变成“显式模型”：

- 文档先对齐当前架构，避免 agent 被旧路径误导。
- UI 先补共享组件，再迁移插件，避免每个 view 重写按钮和 overlay。
- 插件契约强化 view mode、capability、manifest builder、动态命令刷新。
- 后台任务收敛 executor/job 生命周期，降低线程和锁风险。

优先做 P0/P1 项，能最快降低后续维护成本，也能让后续多 agent 改造更容易分工和验证。
