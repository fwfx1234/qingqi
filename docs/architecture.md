# Qingqi Architecture

Qingqi is a Rust + GPUI reimplementation of the local sibling project
`../suishou`. The Rust version should preserve suishou's plugin workflows and
plugin-page visual language, while using Rust-native boundaries for lifecycle,
state, background work, and testability.

Qingqi is not a compatibility runtime. It does not load QML, does not embed
PySide6, does not preserve QObject-style APIs, and does not keep suishou's
Python plugin entrypoints or database schemas. Suishou is the functional and
visual reference; Qingqi owns the Rust architecture.

The suishou `qml-demo` plugin is intentionally not ported. Qingqi replaces it
with `gpui-demo`, a Rust + GPUI learning and component-pattern plugin used to
document how Qingqi builds controls, layouts, state, and background work.
Qingqi also includes `gpui-component` as a higher-level GPUI control toolkit;
see [gpui-component-guide.md](gpui-component-guide.md) before using it in a
plugin or Root-enabling a window.

For day-to-day architecture rules, read
[core-architecture-spec.md](core-architecture-spec.md). For the expert review
and staged adjustment roadmap, read
[architecture-adjustment-plan.md](architecture-adjustment-plan.md). This file
summarizes the current architecture; the spec is the normative source for new
core changes.

## Layers

```text
app/       GPUI application runtime, windows, launcher, theme, shared controls
core/      plugin traits, command model, job model, window specs, storage paths
platform/  OS-specific wrappers such as clipboard, app opening, file dialogs
features/  one directory per plugin: manifest, service/store, view/session
docs/      migration guide and status matrix for long-running port work
```

## Runtime Model

- `app::runtime` is bootstrap only. It wires tracing, paths, shared stores,
  plugin registration, app actions, menus, and app-level background supervisors.
- `app::window_controller` owns launcher and plugin window lifecycle. It is the
  only place that should open, activate, remember, or clean up plugin windows.
- `PluginManager` owns plugin runtimes and exposes a merged command list. It
  caches command lists behind each runtime's cheap command revision, while still
  allowing dynamic query commands for launcher search.
- Plugin runtime/session calls are panic-isolated at the manager/window
  boundary so one plugin can fail into an error surface without taking down the
  launcher or other plugin windows.
- A runtime is long-lived and cheap. It owns shared services, caches, and
  background handles.
- A plugin session is window-scoped. It owns UI state and must release large
  lists, previews, and editor buffers on close.
- Commands are typed. A command either opens a plugin or invokes a plugin
  action with an optional payload.
- Heavy work must not run in render. Use services, stores, and GPUI background
  tasks, then return immutable view models to the UI.

## Event Flow

Feature watchers may publish app events, but plugin-owned data changes should
be consumed by the plugin's own session/view subscriptions instead of causing a
global window refresh.

```text
feature service revision changes
        |
        v
PluginRuntime/service watcher updates plugin-owned state
        |
        v
open plugin session/view observes its service or event source
        |
        v
that plugin UI syncs from the service snapshot and calls cx.notify()
```

Use event kinds intentionally:

- `FeatureChanged`: feature-local state changed; interested plugin UI may sync.
- `CommandsChanged`: launcher command cache may need to be refreshed.
- `JobsChanged`: long-running job snapshots or progress changed.

The app event bus is intentionally lightweight. It is a revisioned publish /
subscribe notification surface, not a data store and not a repaint pump.
Subscribers filter by source/kind, then fetch real data from the owning feature
service. Feature services remain the source of truth.

## Long-Running Jobs

`core::job` defines `JobId`, `JobStatus`, `JobSnapshot`, and `JobProvider`.
Any feature that runs a cancellable or observable long task should expose that
contract from its service layer.

The current reference implementation is `download-manager`:

- `DownloadService` owns active worker flags, persisted task state, and a cheap
  service revision.
- `DownloadService` implements `JobProvider` for snapshots, pause, resume, and
  cancel.
- `DownloadManagerRuntime` watches the service and publishes `JobsChanged` for
  interested consumers.
- `DownloadManagerPanel` should refresh from its service revision through its
  own UI subscription path, rather than relying on app-wide window refresh or
  doing network/database work in GPUI render construction.

Future candidates for this model include image compression batches, QR export
batches, FTP/SFTP transfer queues, and quick-launch runs that need durable
progress.

## Implemented Adjustment Plan

1. Split startup/window/background responsibilities:
   `runtime.rs` handles bootstrap, `window_controller.rs` handles launcher and
   plugin windows, `background.rs` owns app-level recurring loops, and
   `features::registry` owns builtin registration.
2. Keep plugin state UI-light:
   `PluginManager` no longer stores window handles and now focuses on runtimes,
   manifests, command cache, and panic isolation.
3. Keep refresh ownership local:
   plugin watcher loops may publish to `AppEventBus`, but plugin UI should
   subscribe to its own service/event source and notify its own entity.
4. Make launcher commands cheaper:
   runtimes expose command revisions, the manager caches static command lists,
   and dynamic query commands are merged only for non-empty queries.
5. Move slow list loading off the GPUI path:
   clipboard history list refresh and pagination now use background tasks with
   generation checks.
6. Establish a shared job contract:
   `core::job` is the reusable progress/cancel/pause/resume interface and the
   download manager is the first real implementation.
7. Keep large feature views splittable:
   clipboard view is split into `view/mod.rs`, `view/history.rs`,
   `view/settings.rs`, and `view/shared.rs`; apply the same pattern to other
   large GPUI views when they exceed one clear responsibility.

## Migration Rules

- No compatibility layer. Translate suishou behavior into Rust-native
  service/store/view boundaries.
- Do not port QML line-by-line. Port the view model, state transitions, service
  behavior, and visual layout.
- Put business logic in `service.rs` or `store.rs` and add tests before wiring
  GPUI views.
- Use `Rc<RefCell<_>>` only inside window UI state. Use `Arc`, channels, and
  short locks for shared/background services.
- Use `AppPaths` for every persistent file. Do not hard-code user data paths.
- Keep plugin pages visually close to suishou. The launcher can remain cleaner
  and more modern than the reference.
- Replace QML-specific learning content with Qingqi-native GPUI examples in
  `src/features/gpui_demo`.
- Use `gpui-component` for repeated controls, tables, virtualized lists, and
  editor-like inputs when it reduces code and memory churn. Do not use
  Root-dependent APIs until the target window has been explicitly Root-enabled.
- Long-running work must expose service snapshots and event revisions. Avoid
  app-wide repaint loops in feature plugins; notify the owning entity/window
  instead.
- Large plugin pages should split by domain view section before they become
  difficult to test or reason about.

See [migration-guide.md](migration-guide.md) for the execution template and
plugin status matrix.

Core architecture changes should be checked against
[core-architecture-spec.md](core-architecture-spec.md) before plugin-specific
work begins.
