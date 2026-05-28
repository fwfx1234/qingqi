# Qingqi Architecture Adjustment Plan

本文从 Rust、GPUI 和桌面应用架构三个角度审视当前 Qingqi。结论是：项目已经有了正确的骨架，尤其是 `app`、`core`、`platform`、`features` 的大方向，以及 `PluginRuntime` / `PluginSession` 的生命周期区分。下一阶段不应该推倒重来，而应该把已经出现的模式固化成更清晰的 contract，让复杂插件在相同的边界内增长。

## Executive Summary

当前最值得保留的设计：

- `src/app/runtime.rs` 只负责启动编排，窗口生命周期已经迁到 `src/app/window_controller.rs`。
- `PluginManager` 聚焦插件 runtime、manifest、command cache 和 panic isolation，方向正确。
- `AppEventBus` 已经把插件后台变化和 GPUI 全局刷新解耦。
- `core::job` 已经为下载、压缩、传输等长任务提供了统一抽象入口。
- 每个功能以 `features/<plugin>/` 组织，适合继续演进成稳定插件边界。

当前主要问题：

- 后台任务分散在 `thread::spawn`、`cx.spawn(...).detach()` 和轮询 watcher 中，缺少统一任务句柄、取消和错误归集。
- 多个插件仍在 view 渲染路径中同步读取 store 或复制大列表，随着数据量增大会影响 GPUI 响应。
- `AppEventBus` 只有全局 revision，事件合并、按 feature 刷新、按 kind 刷新还比较粗。
- 部分 service 仍引用 GPUI 类型或平台剪贴板上下文，业务层可测试性边界不够纯。
- 大插件 view 文件过大，`quick_launch/view.rs`、`api_debugger/view.rs`、`ftp_sftp_ssh_client/view.rs` 后续维护风险高。
- 动态 command、job、后台事件之间还没有形成统一的 feature capability 描述。

## Target Architecture

目标架构不是增加抽象层数量，而是让每个边界有明确职责。

```text
app/
  runtime.rs             app bootstrap only
  window_controller.rs   launcher/plugin window lifecycle
  background.rs          app-owned recurring tasks and event refresh
  events.rs              lightweight notification bus
  ui.rs/theme.rs         shared visual primitives and tokens

core/
  plugin.rs              runtime/session/capability contracts
  command.rs             typed launcher commands
  job.rs                 long-running job snapshots and controls
  storage.rs             app and feature data paths
  page.rs                pagination contracts

platform/
  clipboard.rs           GPUI/OS clipboard adapter
  hotkey.rs/tray.rs      OS integrations
  shell.rs/apps.rs       app/file/process adapters

features/<plugin>/
  manifest.rs            static metadata and visual/window spec
  model.rs               persisted/domain DTOs
  store.rs               persistence and migrations
  service.rs             domain operations, background orchestration
  view/                  GPUI state, VM mapping, render helpers
  plugin.rs              runtime/session wiring only
```

推荐数据流：

```text
GPUI event
  -> ViewAction
  -> Plugin session or Entity mutates local UI state
  -> Service command/query
  -> Store/platform/background worker
  -> Service snapshot revision changes
  -> AppEventBus publishes kind/source revision
  -> BackgroundSupervisor coalesces refresh
  -> View syncs from cheap snapshot/page
```

## Adjustment 1: Make Runtime, Session, Service Contracts Stricter

现状：

- `PluginRuntime` 和 `PluginSession` 已经分离，但 `plugin.rs` 里仍有部分 runtime 负责 watcher、service ownership、session 构造等多件事。
- 不同插件 service ownership 不统一：有 `Arc<QuickLaunchService>`、`Arc<ApiService>`、`Rc<RefCell<DownloadService>>`、`Arc<Mutex<ClipboardService>>`。
- `close_idle` 语义目前偏隐式，background plugin、window plugin 和 non-background plugin 的资源释放策略需要更明确。

建议：

1. 保持 `PluginRuntime` 长生命周期，只允许它拥有：
   - service handle
   - command cache revision source
   - background watcher handle
   - plugin-wide lightweight cache
2. 保持 `PluginSession` 窗口生命周期，只允许它拥有：
   - GPUI `Entity<T>`
   - window-local selection/input/filter state
   - view model cache
   - subscriptions
3. 给插件能力补一层轻量 trait，而不是继续扩展单个大 trait：
   - `CommandProvider`
   - `BackgroundProvider`
   - `JobProvider` 已存在，可以继续放在 service 层
   - 后续如需要再引入 `SettingsProvider`
4. 收敛 service ownership：
   - 业务 service 默认用 `Arc<Service>`。
   - service 内部状态需要并发访问时再用 `Mutex/RwLock` 包住具体字段。
   - GPUI 单线程窗口状态才使用 `Rc<RefCell<_>>`。
   - `Rc<RefCell<DownloadService>>` 这类混合形态应逐步改为 `Arc<DownloadService>`，因为下载工作本身已经跨线程。

落地顺序：

1. 新增文档约束并在新插件中执行。
2. 先改下载管理器 service handle 为 `Arc<DownloadService>`，因为它是 JobProvider reference。
3. 再清理 clipboard service 对 GPUI clipboard 的直接依赖，抽出 platform adapter。
4. 最后再考虑拆分 `PluginRuntime` trait capability，避免一次性影响所有插件。

## Adjustment 2: Replace Ad-Hoc Background Work With Supervised Tasks

现状：

- `BackgroundSupervisor` 管 app 级轮询，已经很好地避免了 `runtime.rs` 膨胀。
- 插件内部还有多处 `cx.spawn(...).detach()` watcher。
- 业务 service 中还有 `thread::spawn`，如下载、API 请求、Quick Launch 执行。
- 任务完成后的错误状态、取消、join、panic 处理没有统一归口。

建议建立 `core::task` 或扩展 `app::background`：

```text
TaskSupervisor
  - start_named(name, owner, interval, callback)
  - spawn_blocking_job(job_id, work, completion)
  - cancel(job_id)
  - shutdown_owner(owner)
```

短期不要过度设计 async runtime。GPUI 已经提供 `cx.spawn` 和 background executor；当前可以先做最小 contract：

- 每个后台 loop 必须有 stable name 和 owner。
- 每个 loop 必须防重复启动。
- 每个 loop 必须通过 `AppEventBus` 发布状态变化。
- 每个长任务必须有 service revision，并尽量实现 `JobProvider`。
- 禁止在 view click handler 中直接 `thread::spawn`。

优先迁移对象：

1. `DownloadService`：保留 blocking HTTP 可以接受，但 worker 应集中登记 active task，错误、取消、pause/resume 全部通过 `JobProvider` 露出。
2. `ApiService`：从 shell `curl` + `thread::spawn` 迁为 `reqwest` service worker，先保留功能再替换实现；至少要增加请求取消和超时状态。
3. `QuickLaunchService`：进程执行保留 `std::process` 合理，但 active pid、timeout、stop 需要成为标准 job snapshot。
4. `ImageCompress` 和 `Ftp/Sftp/Ssh`：批处理、传输队列必须直接接 `JobProvider`，不要再各自发明进度 UI。

## Adjustment 3: Make App Events More Useful Without Becoming a Store

现状：

- `AppEventBus` 只有全局 revision 和 last event。
- Core no longer converts event revisions into app-wide repaint.
- Consumers should subscribe/filter and refresh only the owning UI surface.

建议分两步做：

1. 保持 `AppEventBus` 不存数据，只增强索引：
   - global revision
   - per source revision
   - per kind revision
   - last event ring buffer 可选，默认只留 last event
2. `BackgroundSupervisor` 继续做 coalescing：
   - 80ms 合并窗口刷新可以保留。
   - `CommandsChanged` 应触发 launcher command revision，而不是要求每个 open window 都全量 refresh。
   - `JobsChanged` 后续可只刷新 job indicator 或打开的 job source window。

建议事件规则：

- `FeatureChanged`：feature service snapshot 变了，插件窗口需要同步。
- `CommandsChanged`：launcher command cache 需要失效，通常由 quick launch、app launcher 触发。
- `JobsChanged`：job progress/status 变了，job-aware UI 需要同步。

落地注意：

- 不要把事件 payload 做成数据总线。数据仍从 service/store snapshot 读取。
- watcher 发布事件时只读 cheap revision，不做慢查询。
- view 中的 refresh 只应拉 cheap snapshot 或分页数据。

## Adjustment 4: Move Slow Queries Out of Render-Time Sync

现状：

- 文档规则已经要求 render 不做 IO。
- 仍能看到一些 render-triggered sync：例如下载视图 `refresh_if_stale()` 会同步读取 store list，再 merge active progress。
- clipboard 已经开始把历史分页改成 async，是正确方向。

建议：

1. 为每个 service 提供 cheap snapshot：
   - revision
   - counts
   - active item ids
   - visible page token
   - pending response/error
2. 大列表查询必须使用明确 action：
   - `load_page(query, filter, offset, limit)`
   - `refresh_async(generation)`
   - `load_more_async(generation)`
3. render 中只允许：
   - 检查 revision
   - 发起非阻塞 refresh request
   - 消费已缓存的 view model
4. `Store` 不应该暴露给 `View`。当前下载视图通过 `service.store()` 读任务，应该改为 `DownloadService::list_tasks(filter) -> Page<DownloadTaskVm>` 或 `DownloadSnapshot`。

优先处理：

- `DownloadManagerPanel::load_tasks` 从直接读 store 改为 service API。
- `ApiDebuggerPanel` 把 pending response/error sync 做成明确 action 或 subscription。
- `QuickLaunchView` 超大文件先按 section 拆分，再逐步把列表和历史加载变成 page/snapshot。

## Adjustment 5: Split Large GPUI Views by State, VM, and Render Section

现状文件规模显示：

- `src/features/quick_launch/view.rs` 约 3280 行。
- `src/features/api_debugger/view.rs` 约 1950 行。
- `src/features/ftp_sftp_ssh_client/view.rs` 约 1868 行。
- `src/features/qr_code/view.rs`、`download_manager/view.rs`、`image_compress/view.rs` 也在继续增长。

建议大插件统一采用：

```text
view/
  mod.rs          panel/entity/session-facing type
  state.rs        UI state structs and enums
  action.rs       ViewAction and reducer-like methods
  vm.rs           render-ready view models
  layout.rs       top-level layout composition
  sections/*.rs   header/sidebar/list/detail/footer render helpers
  shared.rs       plugin-local UI helpers
```

拆分原则：

- 先移动纯 render helper，不改变行为。
- 再移动 state enum / VM 构造。
- 最后再改 service API。
- 每次拆一个插件，不做横跨多个大插件的机械重排。

GPUI 具体规则：

- `Entity<T>` 用于参与渲染和通知的状态。
- `Rc<RefCell<_>>` 只作为过渡，用在 window-local panel。
- 输入框、编辑器、列表选择等 entity 创建一次，不能在 render 中创建。
- 大闭包不要 clone 大列表，传 id/index/service handle。
- UI action 完成后优先使用 `cx.notify()` 或当前窗口的 `window.refresh()`；
  后台 loop 不应触发 app-wide repaint。

## Adjustment 6: Establish Feature Capability and Status Matrix

现状：

- manifest 已经包含 visual、window、stats、command hint。
- 迁移矩阵存在，但个别状态和当前实现不一致。
- feature 是否有 command、job、settings、background watcher，需要读代码才能知道。

建议：

1. 扩展文档矩阵，跟踪每个插件：
   - `runtime`
   - `commands`
   - `background`
   - `jobs`
   - `store`
   - `large view split`
   - `test coverage`
2. 代码层面先不用急着扩 manifest。等 2-3 个插件完成同类迁移后，再决定是否引入 `PluginCapabilitySpec`。
3. 下载、快速启动、FTP/SFTP、图片压缩这类可观察任务，统一标记为 job-capable。

## Adjustment 7: Platform Boundary and IO Adapters

现状：

- `platform/` 已有 clipboard、hotkey、tray、shell、apps，是正确方向。
- 一些 service 直接依赖 `gpui::App` 或 `std::process::Command`。
- API debugger 通过 shell `curl` 执行请求，便于快速落地，但长期会限制取消、流式响应、header/body 精确控制和跨平台行为。

建议：

- 剪贴板：拆出 `ClipboardReader` / `ClipboardWriter` trait，GPUI adapter 放 `platform`，service 只处理记录和配置。
- shell/open：继续放 `platform::shell`，插件不要散落 `Command::new("open")`。
- API 请求：用 `reqwest` 封装 `HttpClient` trait，保留 curl preview 只是展示，不作为执行引擎。
- 进程执行：Quick Launch 可以保留系统进程，但应经 `platform::process` 包装 signal、process group、open url/path。
- 文件选择、保存目录、权限检测后续都放 platform，不进入 view。

## Adjustment 8: Testing and Quality Gates

当前测试方向是对的：store/service 单测多于 UI 测试。下一步要把测试和架构风险绑定：

- Store migration：必须有 legacy schema 或空库测试。
- Service action：必须测试 validation、revision bump、错误状态。
- JobProvider：必须测试 snapshot、cancel、pause/resume 状态转移。
- CommandProvider：必须测试 query、prefix、revision cache。
- View 拆分：至少跑 `cargo check`，如拆出 VM 构造则补纯函数测试。

建议命令：

```bash
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo fmt
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo check
```

warning 策略：

- 已有 warning 可以分批清。
- 新代码不应引入新的 warning。
- 架构迁移不要为了消 warning 删除明确规划中的 contract，除非同步更新文档。

## Phased Roadmap

The phases below are the implementation roadmap after the core specification is
accepted. They are not intended to be completed as one broad plugin rewrite.
For the current core-only milestone, record the rules in
[core-architecture-spec.md](core-architecture-spec.md), update the agent
handoff notes, and defer plugin implementation batches to focused follow-up
work.

### Phase 0: Documentation and Guardrails

目标：统一后续接力工程师的判断标准。

- 增加本文档。
- 更新 `AGENT.md`，把后台任务、事件、view 拆分、service ownership 写成规则。
- 修正迁移矩阵中过时的状态。

验收：

- `cargo check` 不受文档改动影响。
- 文档之间没有互相矛盾的规则。

### Phase 1: Service Ownership and Job Reference

目标：把下载管理器作为标准长任务样板。

- `DownloadService` 改为 `Arc<DownloadService>` ownership。
- `DownloadManagerPanel` 不再直接读 `service.store()`。
- `DownloadService` 提供 `snapshot/list_tasks/filter` API。
- 下载 watcher 只发布 `JobsChanged`，不做 UI 数据读取。

验收：

- 下载新增、开始、暂停、恢复、取消、删除仍可用。
- `JobProvider` 单测覆盖关键状态。
- `cargo test download_manager` 和 `cargo check` 通过。

### Phase 2: Event Bus Refinement

目标：让事件仍轻量，但支持更精细刷新。

- `AppEventBus` 增加 per source / per kind revision。
- `BackgroundSupervisor` 按 kind 做 coalescing。
- `PluginManager` command cache invalidation 只受 command revision 和 `CommandsChanged` 影响。

验收：

- quick launch action 改动后 launcher command 更新。
- 普通 feature 变化不会无意义重建 command cache。
- 事件 API 有单测。

### Phase 3: Large View Splitting

目标：降低大插件维护成本。

- 先拆 `quick_launch/view.rs`，因为它最大且包含动作、编辑、历史、结果多种状态。
- 再拆 `api_debugger/view.rs`。
- 最后拆 `ftp_sftp_ssh_client/view.rs`，拆分前先确定真实 backend trait。

验收：

- 纯移动 commit 行为不变。
- 新增 section 文件不直接访问 store。
- 每个插件拆分后 `cargo check` 通过。

### Phase 4: Platform and IO Adapters

目标：让 service 更可测试，平台行为更集中。

- clipboard service 移除 GPUI `App` 依赖。
- API debugger 执行层迁到 reqwest adapter。
- Quick Launch 的 `open`、signal、process group 进入 platform adapter。

验收：

- service 单测不需要 GPUI。
- macOS 行为仍通过 platform adapter 保持。
- 错误状态可以在 UI 明确展示。

### Phase 5: GPUI Component and Root Strategy

目标：谨慎使用 `gpui-component`，避免 Root 迁移破坏窗口 lifecycle。

- 普通控件可逐步替换为 component。
- Root 迁移按窗口单独做，不作为插件功能迁移的顺手改动。
- Root 后必须重做 window handle/reopen/close path，因为 `downcast::<PluginWindow>()` 语义会变化。

验收：

- launcher/plugin window open、activate、reopen、close 都手动验证。
- `PluginSession::on_close` 仍被调用。
- 没有悬挂窗口 handle。

## Architecture Decision Records To Add Later

后续建议用短 ADR 记录这些决策：

- ADR-001: Qingqi is not a suishou compatibility runtime.
- ADR-002: Runtime/session/service ownership model.
- ADR-003: Background task supervision and job snapshots.
- ADR-004: Event bus is revision notification, not data store.
- ADR-005: GPUI Root migration policy.

## Immediate Checklist

- 新插件必须有 `manifest.rs`、`service.rs` 或明确说明为什么不需要 service。
- 任何可能超过 500 条的数据列表必须分页或虚拟化。
- 任何超过 1000 行的 view 文件，新增功能前先考虑拆 section。
- 任何可取消/可观察长任务必须接 `JobProvider` 或在本文档记录例外。
- 任何后台循环必须有 owner、重复启动保护和事件发布策略。
- 任何新持久化文件必须通过 `AppPaths`。
- 任何平台 IO 必须优先放入 `platform/` 或 service adapter。
