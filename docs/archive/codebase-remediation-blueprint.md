# Qingqi 代码级整改蓝图

> 基于当前工作树与 `docs/codebase-deep-audit-report.md` 的问题清单。  
> 目标不是“再总结一次问题”，而是把整改拆成能直接落 PR 的施工图。  
> 原则：先改契约，再改宿主，再改共享 UI，最后拆 feature view 和后台服务。

## 0. 先说结论

建议把整改拆成 7 个 PR 波次：

1. 文档同步
2. `qingqi-plugin` 契约收敛
3. `qingqi-core` / `qingqi-app` 生命周期与命令缓存
4. `qingqi-ui` 组件库补齐
5. 大型 feature `view` 拆分
6. 后台服务 / runtime / storage
7. 平台 unsafe 与收尾测试

每个波次都要做到：

- 先改 API，再迁移调用点。
- 先补测试，再删旧分支。
- 每次只收一个主题，不把 UI、服务、平台混成一个大改。

---

## 1. 文档同步 PR

### 1.1 `AGENT.md`

目标：让后续 agent 看到的是当前 workspace，而不是 pre-split 叙事。

要改的点：

- 删除 “current pre-split codebase” 之类的表述。
- 用当前 crate 边界替换 `src/app` / `src/core` / `src/platform` / `src/features`。
- 明确：
  - `qingqi-plugin` = SDK
  - `qingqi-core` = 宿主和注册表
  - `qingqi-app` = GUI 外壳
  - `qingqi-ui` = 共享 UI
  - `qingqi-platform` = OS API
  - `qingqi-feature-*` = 内置插件 crate
- 把测试命令改成当前 workspace 的命令。

### 1.2 `README.md`

目标：把项目结构改成当前真实形态。

要改的点：

- 目录结构改成 `crates/qingqi-*`。
- 删除旧 `crates/qingqi/src/app`、`src/core`、`src/platform` 的描述。
- 增加一句：内置插件本质上就是普通插件 crate。

---

## 2. `qingqi-plugin` 契约收敛

### 2.1 `crates/qingqi-plugin/src/plugin.rs`

这是最关键的文件。建议改成下面这个方向：

#### `Plugin` trait

建议新增 / 调整：

```rust
pub trait Plugin {
    fn manifest(&self) -> Manifest;

    fn commands_revision(&self) -> u64 {
        0
    }

    fn commands(&self, query: &str) -> Vec<Command> { ... }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView>;

    fn start_background(&mut self, _events: AppEventBus, _cx: &mut App) -> anyhow::Result<()> {
        Ok(())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn close_idle(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
```

原因：

- `commands_revision()` 让 launcher 不必每次全量失效。
- `start_background()` / `shutdown()` / `close_idle()` 返回 `Result`，避免失败完全无感。
- 现在 `start_background` 只有 `()`，宿主只能靠 panic 捕获，不够可观测。

#### `Manifest`

建议把当前重复字段收敛成：

- `accent: PluginAccent`
- `open_prefixes: Vec<Arc<str>>`
- `action_prefixes: Vec<Arc<str>>`

建议删除：

- `visual: Option<PluginVisualSpec>`
- `command_prefixes`

推荐最终语义：

- `open_prefixes`：打开插件用的前缀
- `action_prefixes`：插件动作 / 命令用的前缀

如果某个插件不需要区分，两个字段可以相同。

#### `ManifestBuilder`

建议新增 builder，直接替代 feature 里一堆 struct literal：

```rust
pub struct ManifestBuilder { ... }

impl ManifestBuilder {
    pub fn new(id: impl Into<Arc<str>>) -> Self;
    pub fn name(self, name: impl Into<Arc<str>>) -> Self;
    pub fn description(self, description: impl Into<Arc<str>>) -> Self;
    pub fn icon(self, icon: IconRef) -> Self;
    pub fn accent(self, accent: PluginAccent) -> Self;
    pub fn category(self, category: PluginCategory) -> Self;
    pub fn status(self, status: PluginStatus) -> Self;
    pub fn mode(self, mode: ViewMode) -> Self;
    pub fn window(self, window: WindowSpec) -> Self;
    pub fn open_prefixes(self, prefixes: impl IntoIterator<Item = impl Into<Arc<str>>>) -> Self;
    pub fn action_prefixes(self, prefixes: impl IntoIterator<Item = impl Into<Arc<str>>>) -> Self;
    pub fn keywords(self, keywords: impl IntoIterator<Item = impl Into<Arc<str>>>) -> Self;
    pub fn stats(self, stats: PluginStats) -> Self;
    pub fn command_hint(self, hint: impl Into<Arc<str>>) -> Self;
    pub fn background(self, yes: bool) -> Self;
    pub fn build(self) -> anyhow::Result<Manifest>;
}
```

建议在 `build()` 里做校验：

- id 不能为空
- name 不能为空
- open/action prefixes 去重
- `background == true` 时 `status` 自动为 `Background`
- `WindowSize::Ratio` 做范围检查

### 2.2 `crates/qingqi-plugin/src/plugin_spec.rs`

建议把这个文件缩成纯枚举/窗口规格：

- `PluginCategory`
- `PluginStatus`
- `PluginAccent`
- `ViewMode`
- `WindowSize`
- `WindowSpec`

建议删除：

- `PluginVisualSpec`
- `PluginWindowMode` 这个别名

因为这些字段都已经在 `Manifest` 里了，继续双写只会漂移。

### 2.3 `crates/qingqi-plugin/src/events.rs`

当前 `AppEventBus` 已经有 `revision/source/kind`，下一步只需要补方便入口：

```rust
pub fn publish_commands_changed(&self, source: impl Into<Arc<str>>) -> u64;
pub fn publish_feature_changed(&self, source: impl Into<Arc<str>>) -> u64;
pub fn publish_jobs_changed(&self, source: impl Into<Arc<str>>) -> u64;
```

作用：

- 让 feature 层不用每次都自己拼 `AppEventKind::CommandsChanged`
- 统一日志字段

### 2.4 `crates/qingqi-plugin/src/database.rs`

建议补两个能力：

1. `unregister_database(key: &str)` / `unregister_databases(keys)`
2. `MigrationRunner`

建议接口：

```rust
pub struct MigrationStep {
    pub version: u32,
    pub name: &'static str,
    pub run: fn(&rusqlite::Transaction<'_>) -> anyhow::Result<()>,
}

pub struct MigrationRunner;
```

用途：

- 统一 `schema_version`
- 统一 `ALTER TABLE` 幂等逻辑
- 统一 migration 测试 fixture

---

## 3. `qingqi-core` / `qingqi-app` 改造

### 3.1 `crates/qingqi-core/src/plugin.rs`

建议的代码级改动点：

#### `PluginManager` 字段

把现有的：

- `dynamic_plugin_ids: HashSet<Arc<str>>`

改成更明确的：

- `dynamic_command_cache: HashMap<Arc<str>, DynamicCommandSnapshot>`

建议新增：

```rust
struct DynamicCommandSnapshot {
    revision: u64,
    commands: Vec<Command>,
}
```

#### `register`

建议改成：

```rust
pub fn register_checked(&mut self, plugin: Box<dyn Plugin>) -> anyhow::Result<()>;
```

逻辑：

- 重复 plugin id 直接报错
- 不再静默替换旧 plugin
- 注册时顺便写入 `plugin_order`

#### `open`

当前只有 `debug_assert_eq!`，建议改成运行时错误：

```rust
if expected_mode != view.mode() {
    anyhow::bail!(
        "plugin {plugin_id} returned {:?}, expected {:?}",
        view.mode(),
        expected_mode
    );
}
```

#### `build_commands`

建议拆成两段：

1. 静态命令：`plugin.commands("")`
2. 动态命令：`plugin.commands(query)`

两段都要 `catch_unwind`。

动态命令建议改成：

- 先读 `commands_revision()`
- 如果 revision 没变，直接用缓存
- 如果变了，再调用 `commands(query)` 重新生成

#### `start_background` / `shutdown` / `close_idle`

建议给 `PluginManager` 加：

- `started_plugins: HashSet<Arc<str>>`
- `starting_plugins` 保护重复启动

逻辑：

- `start_background()` 只对未启动 plugin 生效
- `shutdown()` 只关已经启动过的 plugin
- `close_idle()` 只针对 window plugin

### 3.2 `crates/qingqi-core/src/registry.rs`

建议改成两阶段：

#### 第一阶段：validate

检查：

- plugin id 唯一
- database key 唯一
- capability 合法
- source 合法

#### 第二阶段：build + commit

流程：

1. `register_databases`
2. `build runtime`
3. `register_checked`
4. 任一步失败就 rollback 已注册数据库

建议新增 rollback helper：

```rust
pub fn unregister_databases<I>(&self, keys: I) -> Result<()>;
```

### 3.3 `crates/qingqi-app/src/app/launcher.rs`

建议改的点：

- `start_event_watch()` 里按 `event.source` + `event.revision` 做局部刷新
- `CommandsChanged` 不要让整个 command cache 全量失效
- `Launcher::new()` 不再依赖 `manifest.visual`
- `PluginVisual` 改成只承载 launcher 真正需要的字段：
  - `accent`
  - `category`
  - `status`
  - `mode`
  - `window`

建议新增方法：

```rust
fn refresh_dynamic_commands_for(&mut self, plugin_id: &str, query: &str);
fn refresh_plugin_visuals(&mut self);
```

### 3.4 `crates/qingqi-app/src/app/window_controller.rs`

建议改的点：

- `PluginWindow::drop()` 里 `view.on_close()` 要 `catch_unwind`
- `open_plugin_with_trace()` 打开失败时要把 plugin id、trace id、耗时打印完整
- `cleanup_before_close()` / `on_close()` 的职责拆清：
  - window controller 负责窗口生命周期
  - plugin view 负责自己的内部状态清理

### 3.5 `crates/qingqi-app/src/app/runtime.rs`

建议把启动顺序固定成：

1. init tracing
2. bootstrap paths/database
3. build `PluginManager`
4. register builtin plugins
5. 初始化 `WindowController`
6. `app_catalog.start_background()`
7. `plugin_manager.start_background()`
8. `BackgroundSupervisor` 启动 theme / tray / hotkey

建议关闭顺序：

1. 停 background supervisor
2. `plugin_manager.shutdown()`
3. `database.shutdown()`

---

## 4. `qingqi-ui` 组件库改造

### 4.1 `crates/qingqi-ui/src/ui/mod.rs`

当前这个文件同时放了：

- token
- 旧原语
- 新 `components/*`

建议拆成：

- `tokens.rs`
- `legacy.rs`
- `components/*`

`mod.rs` 只保留 re-export。

### 4.2 `crates/qingqi-ui/src/ui/components/button.rs`

建议扩展成：

```rust
pub enum ButtonSize { XSmall, Small, Medium }
pub enum ButtonVariant { Primary, Secondary, Ghost, Danger }
pub enum ButtonState { Normal, Active, Disabled, Loading }

pub struct ButtonProps {
    pub variant: ButtonVariant,
    pub size: ButtonSize,
    pub accent: Option<PluginAccent>,
    pub state: ButtonState,
    pub icon: Option<&'static str>,
}
```

`button()` 仍返回 `gpui::Div`，这样调用点还能继续链 `.on_click()` / `.id()`。

### 4.3 `crates/qingqi-ui/src/ui/components/chip.rs`

建议把 `dark: bool` 去掉，改为由 token 自己决定：

```rust
pub enum ChipSize { Small, Medium }
pub enum ChipTone { Neutral, Accent, Success, Warning, Danger }
pub enum ChipState { Normal, Selected, Disabled }
```

新增：

- `segmented_control()`
- `chip_group()`

### 4.4 `crates/qingqi-ui/src/ui/components/status_pill.rs`

建议支持：

- icon
- compact
- domain 状态映射

例如：

```rust
pub enum StatusTone {
    Neutral,
    Success,
    Warning,
    Danger,
    Info,
}

pub fn status_pill(label, tone) -> impl IntoElement;
pub fn status_pill_for_task(status: TaskStatus) -> impl IntoElement;
pub fn status_pill_for_http(code: u16) -> impl IntoElement;
```

### 4.5 `crates/qingqi-ui/src/ui/components/overlay_host.rs`

建议扩成单一 host，支持：

- `Dialog`
- `Sheet`
- `Drawer`
- `ContextMenu`
- `Popover`

建议新增：

```rust
pub enum OverlayKind { Dialog, Sheet, Drawer, ContextMenu, Popover }
pub struct OverlayHostProps { ... }
```

必须统一：

- Esc 关闭
- 点击遮罩关闭
- 内容区阻止冒泡
- 底部 action bar

### 4.6 `crates/qingqi-ui/src/ui/components/table_header.rs`

建议把 `table_header_flex()` 换成明确列规格：

```rust
pub enum ColumnWidth {
    Fixed(f32),
    Flex { grow: f32, min: f32, max: Option<f32> },
}
```

然后再补：

- `DataTableShell`
- `TableRowActions`
- `TableEmptyState`

### 4.7 `crates/qingqi-feature-gpui-demo`

建议加一个 UI 预览页：

- Button 状态
- Chip 状态
- StatusPill 状态
- OverlayHost 状态
- TableHeader / DataTableShell

这个 crate 很适合做共享 UI 的可视化回归。

---

## 5. 大型 feature view 拆分

### 5.1 `crates/qingqi-feature-api-debugger/src/view.rs`

建议拆成：

```text
view/
  mod.rs
  state.rs
  tabs.rs
  request_editor.rs
  response_panel.rs
  collection_tree.rs
  environment.rs
  overlays.rs
  style.rs
  input.rs
```

具体搬运：

- `OpenTab` / `KvRow` / `KvEditor` / `AuthFormInputs` / `AuthFormValues` -> `state.rs` / `request_editor.rs`
- `collection_tree()` / `group_section()` / `request_tree_block()` -> `collection_tree.rs`
- `open_tabs_bar()` / `action_bar()` -> `tabs.rs`
- `editor_panel()` / `auth_form_panel()` -> `request_editor.rs`
- `response_panel()` / `response_history_view()` / `response_code_view()` -> `response_panel.rs`
- `env_popup()` / `env_manager_dialog()` -> `environment.rs`
- `overlay_shell()` / `context_menu_overlay()` / `curl_import_dialog()` / `rename_dialog()` -> `overlays.rs`
- `status_badge()` / `method_badge()` / `scenario_status_pill()` -> `style.rs`
- `kv_input()` / `single_input()` / `multiline_input()` / `kv_editor_table()` -> `input.rs`

这里最重要的状态收口：

```rust
enum ActiveOverlay {
    EnvPopup,
    EnvManager,
    CollectionMenu(CollectionMenuState),
    CurlImport,
    Rename,
}
```

### 5.2 `crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs`

建议拆成：

```text
view/
  mod.rs
  state.rs
  sidebar.rs
  remote_browser.rs
  terminal_panel.rs
  transfer_panel.rs
  overlays.rs
  shared.rs
  terminal/
    input.rs
    render.rs
    layout.rs
```

具体搬运：

- `ProfileEditorState` / `RemoteActionState` / `RemoteMenuState` / `ProfileMenuState` -> `state.rs`
- `left_sidebar()` -> `sidebar.rs`
- `file_pane()` / `remote_entry_list()` / `remote_entry_row()` -> `remote_browser.rs`
- `terminal_pane()` / `render_terminal_rows()` / `render_terminal_row()` -> `terminal/render.rs`
- `terminal_input_for_event()` / `terminal_mouse_*()` / `map_mouse_button()` -> `terminal/input.rs`
- `transfer_strip()` -> `transfer_panel.rs`
- `remote_menu_overlay()` / `profile_menu_overlay()` / `remote_action_overlay()` / `profile_editor_overlay()` -> `overlays.rs`
- `glass_panel()` / `panel_header()` / `toolbar_button()` / `menu_item()` / `danger_menu_item()` -> `shared.rs` 或共享 UI

### 5.3 `crates/qingqi-feature-quick-launch/src/view.rs`

建议拆成：

```text
view/
  mod.rs
  state.rs
  action_list.rs
  editor.rs
  parameters.rs
  history.rs
  result.rs
  overlays.rs
  shared.rs
```

具体搬运：

- `PendingExecution` / `HistorySheetState` / `ResultSheetState` / `ActionMenuState` / `DeleteConfirmState` / `ActionEditorState` -> `state.rs`
- `open_action_menu()` / `close_action_menu()` / `open_delete_confirm()` -> `overlays.rs`
- `open_selected_editor()` / `save_editor()` / `set_editor_*()` -> `editor.rs`
- `open_selected_history()` / `refresh_history_panel()` -> `history.rs`
- `open_selected_result()` / `set_result()` / `copy_result_stdout()` -> `result.rs`
- `pending sheet` 相关 -> `parameters.rs`

核心状态建议改成：

```rust
enum ActiveOverlay {
    ActionMenu(ActionMenuState),
    DeleteConfirm(DeleteConfirmState),
    Pending(PendingExecution),
    Editor(ActionEditorState),
    Result(ResultSheetState),
    History(HistorySheetState),
}
```

### 5.4 `crates/qingqi-feature-download-manager/src/view.rs`

建议拆成：

```text
view/
  mod.rs
  state.rs
  toolbar.rs
  filters.rs
  task_table.rs
  settings_overlay.rs
  format.rs
```

具体搬运：

- `FilterTab` -> `state.rs`
- `header_bar()` / `url_input_bar()` -> `toolbar.rs`
- `filter_bar()` / `filter_chip()` -> `filters.rs`
- `task_list()` / `task_row()` / `progress_bar()` / `status_tag()` -> `task_table.rs`
- `settings_overlay()` / `settings_field()` -> `settings_overlay.rs`
- `format_bytes()` / `format_speed()` / `format_eta()` / `format_progress()` / `truncate_*()` -> `format.rs`

### 5.5 `crates/qingqi-feature-http-capture/src/view.rs`

建议拆成：

```text
view/
  mod.rs
  state.rs
  filter_bar.rs
  capture_table.rs
  detail_panel.rs
  mock_panel.rs
  certificate_panel.rs
  overlays.rs
```

具体搬运：

- `show_mock_panel` / mock 编辑字段 -> `state.rs` / `mock_panel.rs`
- `filter` / `selected_id` / `detail_tab` -> `state.rs`
- `filter_bar()` -> `filter_bar.rs`
- `capture_table()` -> `capture_table.rs`
- `detail tabs` / `response body` / `headers` -> `detail_panel.rs`
- mock 规则 UI -> `mock_panel.rs`
- 证书引导 / 安装提示 -> `certificate_panel.rs`

### 5.6 其他 feature

建议也顺手收口，但优先级低于上面 5 个大块：

- `crates/qingqi-feature-image-compress/src/view.rs`
  - `batch.rs`
  - `queue_table.rs`
  - `settings_panel.rs`
  - `toolbar.rs`
- `crates/qingqi-feature-qr-code/src/view.rs`
  - `state.rs`
  - `input_panel.rs`
  - `preview_panel.rs`
  - `scan_panel.rs`
- `crates/qingqi-feature-clipboard/src/view/*`
  - 继续保持 `history.rs` / `settings.rs` / `shared.rs` 分层
- `crates/qingqi-feature-system-settings/src/view.rs`
  - 拆 `sections/`

---

## 6. 后台服务 / runtime / storage

### 6.1 `crates/qingqi-feature-download-manager/src/service.rs`

建议新增：

- `DownloadWorkerPool`
- `DownloadJobHandle`
- `ProgressThrottle`

改造目标：

- 不要再每个下载一个裸 `thread::spawn`
- 不要在 worker 里反复 `store.lock().unwrap()`
- 进度持久化节流到固定间隔

建议拆出的文件：

```text
service/
  mod.rs
  executor.rs
  job.rs
  progress.rs
```

### 6.2 `crates/qingqi-feature-api-debugger/src/service.rs`

建议拆成：

```text
service/
  mod.rs
  executor.rs
  request.rs
  environment.rs
  import.rs
  script.rs
```

核心改造：

- 复用一个 blocking client
- 所有 import / send / env 操作都进 job executor
- 文件 IO 不在 UI handler 里同步跑
- `thread::spawn` 改 bounded worker / `spawn_blocking`

### 6.3 `crates/qingqi-feature-ftp-sftp-ssh-client/src/runtime.rs`

建议新增：

- `RuntimeSupervisor`
- `ShutdownToken`
- `TaskHandle`

把：

- transfer
- terminal
- session

分开管理，避免 session 关了任务还在跑。

### 6.4 `crates/qingqi-feature-http-capture/src/engine.rs`

建议新增：

- `CaptureEngineHandle`
- `ProxyStopToken`

把 proxy 启动/停止变成可 join 的句柄，而不是只靠后台线程自然退出。

### 6.5 `crates/qingqi-feature-http-capture/src/proxy_handler.rs`

建议把所有 mock response 构建的 `unwrap()` 改成显式错误返回：

- 不能构建时返回 500 fallback
- 同时记录 `tracing::error!`

### 6.6 `crates/qingqi-platform`

#### `tray.rs`

- `static mut CURRENT_TRAY` 改成 `OnceLock<Mutex<Option<TrayIcon>>>`
- 或者把 tray 完全交给 `BackgroundSupervisor`

#### `clipboard.rs`

- 拆成 `clipboard/windows.rs`
- 拆成 `clipboard/macos.rs`
- 拆成 `clipboard/unsupported.rs`

#### `power.rs` / `theme.rs`

- 所有 `panic!` 改 `Result`
- `unsafe impl Send` 补 `SAFETY:` 注释

### 6.7 `crates/qingqi-plugin/src/database.rs` + 各 store

建议：

- `MigrationRunner` 统一 schema 版本
- 每个 store 都只写自己的 migration 列表
- `download-manager` / `quick-launch` / `http-capture` / `api-debugger` / `clipboard` 都切换到统一 helper

---

## 7. 具体测试清单

### 7.1 插件 SDK / core

要补的测试：

- `PluginManager::open` 模式不一致返回 error
- dynamic command panic 不会打断其他 plugin
- `register_checked` 会拒绝重复 plugin id
- `build_all` 失败时能回滚已注册 database
- `ManifestBuilder` 去重和校验

### 7.2 UI

建议用 `qingqi-feature-gpui-demo` 做 preview：

- Button variant / size / disabled / loading
- Chip selected / disabled
- StatusPill for task/http/job
- OverlayHost dialog/menu/sheet
- TableHeader/DataTableShell

### 7.3 服务

建议补：

- download manager 并发上限
- download manager pause/resume/cancel
- api debugger 连续导入 / 发送不会炸线程
- ftp session 关闭后任务退出
- http capture proxy stop 可 join
- qr/image background result drain 不丢消息

### 7.4 运行命令

每个阶段至少跑：

```bash
cargo fmt --all
cargo check --workspace
```

涉及 core / service / UI 拆分后再跑：

```bash
cargo test --workspace -j 1 --quiet
cargo clippy --workspace --all-targets
```

---

## 8. 推荐落地顺序

1. 文档：`AGENT.md`、`README.md`
2. 插件 SDK：`plugin.rs`、`plugin_spec.rs`、`events.rs`、`database.rs`
3. 宿主：`core/plugin.rs`、`core/registry.rs`、`app/launcher.rs`、`app/window_controller.rs`
4. UI：`ui/components/*` + `ui/mod.rs`
5. 大 view 拆分：API Debugger、FTP、Quick Launch、HTTP Capture
6. 服务重构：Download、API、FTP runtime、HTTP Capture
7. 平台 / migration 收尾

这条顺序的好处是：

- 先把契约收紧，后面的 feature 才有统一入口
- 先补共享 UI，后面的 view 才不会继续复制按钮和 overlay
- 先改宿主生命周期，后面的后台任务才好落地

