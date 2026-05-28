# Qingqi Core Architecture Specification

本文是 Qingqi 的核心架构规范，目标是约束后续 Rust + GPUI 开发的稳定边界。插件功能迁移可以分批做，但核心结构必须先保持一致：启动、窗口、事件、后台任务、插件生命周期、平台 IO 和测试边界各自有唯一职责。

## Scope

本规范覆盖核心架构：

- `src/app/`
- `src/core/`
- `src/platform/`
- `src/features/registry.rs`
- 每个插件必须遵守的 runtime/session/service/view 边界

本规范不要求一次性完成所有插件的功能迁移，也不要求立刻拆完所有大 view。插件级改造必须按本文的边界逐步推进。

Core architecture work is complete when the repository has an agreed contract
for boundaries, ownership, events, background work, jobs, commands, platform IO,
and GPUI lifecycle. Migrating every existing plugin to the ideal form is a
follow-up execution track, not a prerequisite for this specification to be
accepted.

## Layer Contract

```text
app/
  runtime.rs             bootstrap only
  window_controller.rs   launcher and plugin window lifecycle
  background.rs          app-owned tray, hotkey, theme, and OS event bridges
  events.rs              revisioned notification bus
  launcher.rs            command search and inline/list plugin host
  theme*.rs/ui.rs        app-level theme and shared GPUI primitives

core/
  plugin.rs              PluginRuntime, PluginSession, PluginManager
  command.rs             typed launcher command model
  job.rs                 long-running job contract
  plugin_spec.rs         manifest visual/window/category/status specs
  storage.rs             AppPaths and feature path derivation
  page.rs                pagination DTOs

platform/
  clipboard.rs           GPUI/OS clipboard adapter
  hotkey.rs/tray.rs      macOS status item and global hotkey integration
  shell.rs/apps.rs       OS open, app scanning, file dialogs, process adapters

features/
  registry.rs            builtin runtime registration and preview stubs
  <plugin>/              manifest, runtime wiring, service/store/model/view
```

Rules:

- `app::runtime` must not become a feature registry, window manager, task supervisor, or platform API bag. It only wires startup.
- `core` must not depend on plugin UI or platform implementation details.
- `platform` must not depend on feature UI.
- `features::registry` is the only builtin plugin registration point.
- Feature directories own feature-specific service/store/view logic, but must not bypass core contracts.

## Runtime Lifecycle

Qingqi has three different lifetimes. Do not mix them.

### Application Lifetime

Owned by `app::runtime` and app-level singletons:

- `AppPaths`
- `ThemeStore`
- `PluginManager`
- `WindowController`
- `BackgroundSupervisor`
- `AppEventBus`

Application lifetime objects may be shared with `Rc<RefCell<_>>` only when they stay on the GPUI main thread. Cross-thread services must use `Arc` plus interior synchronization where needed.

### Plugin Runtime Lifetime

Owned by `PluginManager`.

A `PluginRuntime` may own:

- `Arc<Service>`
- cheap command revision source
- background watcher state
- plugin-wide lightweight cache
- static manifest metadata

A `PluginRuntime` must not own:

- GPUI render trees
- window handles
- large per-window buffers
- selected row/input state

### Plugin Session Lifetime

Owned by the launcher inline/list host or a plugin window.

A `PluginSession` may own:

- `Entity<T>` view state
- window-local `Rc<RefCell<Panel>>`
- current query/filter/selection
- subscriptions
- cached render-ready view model

A `PluginSession` must release large buffers, lists, previews, editor state, and subscriptions in `on_close` or when dropped.

## Plugin Manager Contract

`PluginManager` owns only plugin runtime coordination:

- runtime registration
- manifest collection
- command cache
- dynamic command query delegation
- runtime/session panic isolation
- background runtime startup
- idle runtime close hooks

It must not:

- open or activate windows
- store window handles
- call platform APIs directly
- know plugin-specific UI state
- own global refresh policy

Window behavior belongs in `app::window_controller`.

## Window Controller Contract

`app::window_controller` is the only place that opens, activates, remembers, or clears launcher/plugin windows.

Required behavior:

- Reopen should activate an existing window when possible.
- Stale handles should be detected and cleared.
- Plugin window close must call `PluginSession::on_close`.
- Non-background plugin runtimes may receive `close_idle`.
- The launcher may host inline/list plugin sessions, but must close those sessions when leaving plugin mode.

Root migration note:

- Current handle logic relies on `downcast::<Launcher>()` and `downcast::<PluginWindow>()`.
- Any future `gpui_component::Root` migration must replace that downcast path with retained inner entity handles or wrapper methods.
- Root migration is a window lifecycle change, not a drive-by UI refactor.

## Event Bus Contract

`AppEventBus` is a notification surface, not a data store.

Allowed data:

- global revision
- last event
- event subscribers
- optional per-source revision
- optional per-kind revision

Disallowed data:

- task lists
- command lists
- response bodies
- clipboard records
- plugin state snapshots

Event kinds:

- `FeatureChanged`: service/store state changed; relevant plugin UI may refresh from service snapshot.
- `CommandsChanged`: launcher command cache may need invalidation.
- `JobsChanged`: long-running job snapshots/progress changed.

Rules:

- Feature watchers may publish events; they do not call global refresh directly.
- Plugin UI is responsible for subscribing to its own service/event source and
  notifying its own entity when plugin-owned data changes.
- Core must not turn feature/job events into global window refreshes.
- `AppEventBus::subscribe` is for interested consumers; subscribers must filter
  by source/kind and then pull real data from the owning service/store.
- Event publishers must only read cheap revisions or flags.
- Event consumers must fetch real data from the owning service/store.

## Background Work Contract

App-level recurring loops belong in `app::background`.

Examples:

- tray action polling
- hotkey polling
- theme/system appearance observation

Feature-owned recurring loops belong in the feature runtime/service. Clipboard
history capture, download progress watching, FTP transfer watching, and similar
domain work must stay with the owning plugin. Core may provide platform helpers
such as clipboard read/write, but should not own feature data capture loops.

Feature-level background work belongs to the feature runtime/service, but must follow these rules:

- One owner per recurring task.
- Guard against duplicate starts.
- Publish `AppEventBus` events for interested consumers instead of triggering
  app-wide repaint from loop bodies.
- Keep a cheap revision on the service.
- Do not hold locks across slow IO, network, compression, process waits, or database scans.
- Use `JobProvider` for observable/cancellable/pauseable long-running work.
- Detached tasks are a temporary GPUI integration tool; new code must still have an obvious owner and shutdown/cancel path.

Recommended future core shape:

```text
TaskOwner = app | plugin_id | service_id
NamedLoop = owner + name + interval + cheap callback
TrackedJob = JobId + provider + cancel/pause/resume + latest snapshot
```

Do not introduce this abstraction until at least one existing task is migrated behind tests.

## Job Contract

`core::job` is the shared contract for long-running work.

A feature should implement `JobProvider` when work is:

- cancellable
- pauseable/resumable
- progress-bearing
- user-visible beyond one instant action
- durable enough to appear in a job list

Service responsibilities:

- Own active worker flags/handles.
- Persist or expose task state.
- Increment a cheap revision on status/progress changes.
- Publish or trigger `JobsChanged` through runtime watcher.
- Return `JobSnapshot` without requiring GPUI.

UI responsibilities:

- Render snapshots.
- Invoke `cancel_job`, `pause_job`, `resume_job`.
- Never infer worker state from UI-only flags.

## Command Contract

Launcher commands are typed through `CommandTarget`.

Rules:

- Static plugin-open commands come from manifest metadata.
- Dynamic commands must expose a cheap revision via `commands_revision`.
- `commands()` must be reasonably cheap; if it touches a large store, add a service cache or snapshot first.
- `commands_for_query(query, limit)` may consult dynamic providers, but must respect limits and avoid slow full scans.
- Command payloads may be plugin-local strings, but dispatch outside the plugin must remain typed.

Command invalidation:

- `PluginManager` owns command cache.
- Runtimes own command revision.
- Feature watchers publish `CommandsChanged` when command source revisions change.
- Launcher syncs command cache through `PluginManager`, not by reading feature services directly.

## Service/Store/View Contract

Feature internals must keep this flow:

```text
GPUI event
  -> ViewAction
  -> Session/View state update
  -> Service command/query
  -> Store/platform/background worker
  -> Service revision/snapshot
  -> AppEventBus event
  -> View refreshes from service snapshot/page
```

Service:

- Owns domain behavior and worker coordination.
- May use platform adapters.
- Must be testable without launching GPUI whenever possible.
- Should expose snapshots/pages/DTOs rather than raw stores.

Store:

- Owns persistence and migrations.
- Does not know GPUI.
- Does not know window/session state.

View/session:

- Owns GPUI entities, local input, selection, and render-ready cache.
- Does not open SQLite directly.
- Does not run network/process/compression work in render.
- Does not hold service locks while building large element trees.

## Ownership Rules

- Use `Arc<Service>` as the default shared service handle.
- Put `Mutex/RwLock` around specific mutable state inside the service, not around the entire service unless the service is still transitional.
- Use atomics for cheap revisions and worker flags.
- Use channels or service snapshots for background-to-UI communication.
- Use `Rc<RefCell<_>>` only for GPUI-main-thread window-local state.
- Do not store `Rc<RefCell<_>>` in background services.
- Do not hold any lock across slow IO.

## GPUI Rules

- `render` builds elements from current state only.
- Create input/editor/list entities once in session/view state, not inside render.
- Use `Entity<T>` for GPUI state that participates in rendering and notifications.
- Use `cx.notify()` for entity-local changes.
- Do not use app-wide repaint for ordinary UI or plugin data changes. Use
  `cx.notify()` for entity-local changes and `window.refresh()` only when a
  callback already has the current window and the whole current window must
  repaint.
- Large views should be split into `view/` modules before adding new complex sections.
- Large lists must be paginated or virtualized.

## Platform Boundary

All platform-specific behavior should be in `platform/` or a feature-local adapter trait implemented through `platform/`.

Examples:

- Clipboard read/write: `platform::clipboard`.
- File/app open: `platform::shell` or `platform::apps`.
- File picker: `platform::shell`.
- Global hotkeys: `platform::hotkey`.
- Tray/status item: `platform::tray`.
- Process groups/signals: future `platform::process`.
- HTTP execution: feature service adapter, not GPUI view.

Rules:

- Feature views must not scatter `std::process::Command`.
- Stores must not call platform APIs.
- Services may call platform adapters, but keep them mockable when practical.

## File Organization Rules

Core files should stay small and specific:

- `runtime.rs`: startup wiring only.
- `window_controller.rs`: window lifecycle only.
- `background.rs`: app-level recurring loops only.
- `events.rs`: event bus only.
- `plugin.rs`: plugin contracts and manager only.
- `command.rs`: command DTO/scoring only.
- `job.rs`: job DTO/provider only.

Feature directories should prefer:

```text
manifest.rs
model.rs
store.rs
service.rs
plugin.rs
view.rs or view/
```

Use `view/` once a view exceeds one clear responsibility. Recommended split:

```text
view/mod.rs
view/state.rs
view/action.rs
view/vm.rs
view/layout.rs
view/sections/*.rs
view/shared.rs
```

## Testing Gates

Run at least:

```bash
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo fmt
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo check
```

Add focused tests for:

- storage migrations
- parsing/validation
- command matching and revision invalidation
- event bus revision behavior
- job state transitions
- service snapshot logic

Plugin UI-only changes may not need broad tests, but must not weaken service/store tests.

## Core Architecture Completion Criteria

The core architecture is considered aligned when:

- `runtime.rs`, `window_controller.rs`, `background.rs`, and `events.rs` keep the boundaries above.
- `PluginManager` has no window ownership.
- Core does not convert feature watcher events into app-wide refreshes.
- Feature UIs own their data refresh subscriptions and entity notifications.
- Long-running observable work has a path to `JobProvider`.
- New shared service handles default to `Arc<Service>`.
- New platform IO enters through `platform/` or adapter traits.
- New large plugin views are split before additional behavior is piled onto one file.
- `AGENT.md`, `docs/architecture.md`, and this file agree on the same rules.

For the current core-only milestone, completion means:

- The authoritative spec exists and covers all core boundaries above.
- `AGENT.md` points future agents to the spec before core changes.
- `docs/architecture.md` and `README.md` expose the spec and adjustment plan.
- Plugin implementation changes are explicitly deferred to plugin-specific
  batches.
- No plugin source files are modified merely to satisfy this documentation
  milestone.

## Deferred Plugin Work

The following are intentionally deferred from the core-only milestone:

- Rewriting existing plugin services to `Arc<Service>`.
- Moving every slow query out of existing render paths.
- Splitting every large plugin view immediately.
- Root-enabling existing windows.
- Replacing all platform calls inside feature code.
- Implementing every phase in `architecture-adjustment-plan.md` as code.

Those changes should be performed one plugin or one shared primitive at a time,
with tests and manual lifecycle checks appropriate to the touched code.
