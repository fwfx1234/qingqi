# Qingqi Agent Notes

This project is a Rust + GPUI desktop toolbox. Keep changes aligned with the
current architecture instead of growing `app::runtime` into a catch-all entry
point.

Authoritative core rules live in `docs/core-architecture-spec.md`. Read that
file before changing `src/app/`, `src/core/`, `src/platform/`, plugin runtime
wiring, background loops, event dispatch, command caching, or job handling.
`docs/architecture-adjustment-plan.md` records the expert review and staged
roadmap; the spec is the day-to-day rulebook.

## Architecture Boundaries

- `src/app/runtime.rs` is the application bootstrap only: tracing, paths,
  shared stores, plugin manager creation, action/menu registration, and startup
  wiring.
- `src/app/window_controller.rs` owns launcher and plugin window lifecycle:
  open, activate, close cleanup, and command dispatch into windows.
- `src/app/background.rs` owns app-level polling and platform integration loops
  such as clipboard capture, tray actions, global hotkeys, and app event
  refresh.
- `src/app/events.rs` is the app-level event bus. Feature watchers publish
  lightweight events here; they should not directly own global refresh policy.
- `src/features/registry.rs` owns builtin plugin registration and preview stub
  registration.
- `src/core/` should stay UI-light: plugin traits, command types, storage paths,
  specs, and cross-feature contracts.
- `src/core/job.rs` defines the shared long-running job contract. Feature
  services with progress/cancel/pause/resume should implement `JobProvider`
  instead of inventing a separate UI-only progress API.
- `src/platform/` wraps OS APIs and should not depend on feature UI.
- Each `src/features/<feature>/` should keep domain logic in `service.rs` or
  `store.rs`, persistent models in `model.rs`, and GPUI rendering/state in
  `view.rs` or a `view/` module.
- Core architecture changes must not turn into broad plugin rewrites. Implement
  shared contracts first, then migrate one plugin at a time in a separate,
  testable pass.

## GPUI Rules

- Do not perform network, database, or filesystem work inside `render`.
- Avoid blocking work in input handlers and click handlers. Start a background
  task, then update the relevant GPUI entity on completion.
- Prefer `Entity<T>` for GPUI state that participates in rendering and
  notifications.
- Use `Rc<RefCell<_>>` only for window-local state that cannot reasonably be an
  entity yet.
- Use `Arc`, channels, atomics, or short-lived locks for shared/background
  services. Do not hold a lock across slow IO.
- Plugin sessions are window-scoped and should release large lists, buffers, and
  previews when closed.

## Plugin Rules

- Register builtin plugins through `features::registry::register_builtin_plugins`.
- Keep `PluginManager` focused on plugin runtimes, commands, manifests, and
  panic isolation. New window behavior belongs in `app::window_controller`.
- `PluginManager` owns command caching. Dynamic command providers must expose a
  cheap revision and keep `commands()` reasonably cheap.
- Commands should be typed `CommandTarget`s. Avoid stringly command dispatch
  outside plugin-local payloads.
- If a plugin exposes dynamic launcher commands, provide a cheap revision value
  and avoid rebuilding command lists from slow stores on every keystroke.
- `PluginRuntime::open_session` and `start_background` receive `AppEventBus`.
  Use it to publish `FeatureChanged`, `CommandsChanged`, or `JobsChanged`
  for interested plugin/session/UI code. Core does not convert plugin events
  into global window refreshes.
- Runtime/session/service ownership follows the core spec:
  `PluginRuntime` owns `Arc<Service>` and cheap runtime-wide state; sessions own
  GPUI entities, subscriptions, selection, and render-ready caches.
- Prefer `Arc<Service>` for new shared services. Use `Rc<RefCell<_>>` only for
  GPUI-main-thread window-local state, not background services.

## Background Work

- Add app-level recurring loops in `app::background`.
- Prefer one owner per recurring task and guard against duplicate starts.
- Long-running feature jobs should expose cancellation/progress through their
  service layer, implement `JobProvider`, and publish `JobsChanged` when state
  changes.
- Feature-owned recurring work belongs to the feature runtime/service. For
  example, clipboard history capture is owned by the clipboard plugin; core
  only exposes clipboard platform read/write helpers.
- Detached tasks are acceptable for current GPUI integration, but new work
  should keep a clear ownership path so the task can later be supervised or
  canceled.
- `BackgroundSupervisor` is only for app-level loops such as tray, global
  hotkey, and theme observation. Do not add plugin data refresh loops there.
- Every recurring task needs one owner, a stable name, duplicate-start
  protection, and a clear event publication strategy.
- Do not hold locks across network, filesystem, process, compression, database
  scans, or other slow work.

## Core Architecture Work

- Treat `docs/core-architecture-spec.md` as the acceptance criteria for core
  design changes.
- Keep `PluginManager` free of window handles and platform IO.
- Keep `WindowController` as the sole launcher/plugin window lifecycle owner.
- Keep `AppEventBus` as revisioned notification only; do not store feature data
  in it or use it as a global repaint trigger. Consumers may subscribe, filter
  by source/kind, and pull data from the owning service/store.
- Add capability abstractions only after at least one concrete existing
  workflow proves the need.
- Do not Root-enable GPUI windows as part of unrelated UI work. Root migration
  changes window handle/reopen/close semantics and must be planned per window.

## Feature Notes

- Clipboard history rendering is split under `src/features/clipboard/view/`.
  Keep history UI, settings UI, and shared helpers separated; list loading
  should stay asynchronous through `ClipboardPanel::refresh_async` and
  `load_more_async`.
- Download manager is a real builtin plugin, not a preview stub. Its service is
  the first `JobProvider` implementation and should remain the reference for
  future long-running feature jobs.
- App launcher and quick launch publish `CommandsChanged` when their backing
  revisions change so launcher command cache invalidation stays cheap.

## Testing

- Keep store/service logic testable without GPUI.
- Add focused unit tests when changing parsing, persistence, command matching,
  migrations, or background job state transitions.
- Run `cargo fmt` and `cargo check` before handing off changes. Existing unused
  warnings are tolerated, but new compile errors are not.
