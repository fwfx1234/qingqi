# GPUI Component Usage Guide

This guide is for low-level models and handoff engineers working on Qingqi UI
migration. Qingqi now depends on `gpui-component` as an optional higher-level UI
toolkit on top of GPUI. Use it deliberately: it can reduce duplicated control
code, but Qingqi still owns the application architecture, plugin lifecycle, and
suishou visual parity.

## Current Integration

- Dependency: `gpui-component = "0.5.1"`.
- GPUI compatibility: Qingqi pins `gpui = "=0.2.2"`, and
  `gpui-component 0.5.1` also targets GPUI `0.2.2`.
- Initialization: `gpui_component::init(cx)` is called once in
  `src/app/runtime.rs` before Qingqi registers local `TextInput` bindings.
  After the workspace split, this initialization belongs in `qingqi-app`.
- The current integration does **not** wrap Qingqi windows in
  `gpui_component::Root` yet.

Measured release startup overhead for dependency + `init` only:

```text
baseline release RSS median:         75,176 KB
with gpui-component init RSS median: 75,632 KB
delta:                                  456 KB
```

This means the library initialization cost is acceptable. Memory risk comes
from specific heavy widgets and data ownership, not from the dependency itself.

## What To Use It For

Prefer `gpui-component` for controls where Qingqi currently repeats a lot of
hand-written GPUI style code:

- Buttons, icon buttons, toggles, checkboxes, switches.
- Tabs, segmented controls, badges, tags, simple menu/dropdown controls.
- Dialogs, sheets, popovers, notifications, once a window is Root-enabled.
- Form rows and settings controls.
- Resizable split panels for API debugger, FTP/SFTP/SSH, quick launch editor
  sheets, and other dense tools.
- Virtual list/table patterns for large or variable-height data.
- Code editor / rope-backed input only where Qingqi truly needs editor
  behavior: scripts, JSON bodies, API bodies, logs, markdown/text previews.

Keep Qingqi's existing lightweight controls where they are cheaper and already
well-tested:

- Launcher query input.
- Small search fields.
- Simple settings text fields.
- Existing single-line fields that do not need syntax highlighting, line
  numbers, LSP, search panel, or editor-style selection behavior.

## Root Requirement

`gpui_component::Root` is required for window-scoped overlay features:

- `WindowExt::open_sheet`
- `WindowExt::open_dialog`
- Notifications
- Focus restoration for component-managed overlays
- `focused_input` tracking for `gpui_component::input::InputState`

Do not call these APIs in a window whose root view is still `Launcher` or
`PluginWindow`. `Root::read` and `Root::update` expect the first window layer to
be `gpui_component::Root` and will panic otherwise.

Until a window is Root-enabled, safe component families are:

- `button`
- `tab`
- `badge`
- `tag`
- `checkbox`
- `switch`
- `slider`
- `progress`
- plain layout/style helpers
- virtual list/table if their state is local and they do not depend on Root
  overlay APIs

When converting a window to Root, preserve Qingqi's lifecycle semantics:

1. Keep the real app view as a GPUI `Entity<T>`.
2. Create `Root::new(view, window, cx)` as the actual window root.
3. Audit all places that call `window_handle.downcast::<Launcher>()` or
   `window_handle.downcast::<PluginWindow>()`. They will no longer work if the
   stored handle points at `Root`.
4. Replace downcast-based reopen/activate paths with either a retained inner
   entity handle or a small root wrapper that exposes the required operation.
5. Verify window close still calls `PluginSession::on_close` and
   `PluginManager::close_idle`.

Root migration is allowed during development, but do it per window, not as a
drive-by change while migrating an unrelated plugin.

## Memory Rules

The global dependency is cheap enough. Widget state is where memory can grow.

- Do not create editor/input entities in render. Create them once in session or
  view state, reuse them, and release large buffers on close.
- Prefer lightweight Qingqi `TextInput` for ordinary text fields.
- Use `gpui_component::input::InputState` only for editor-like fields or when a
  component API requires it.
- Do not enable editor features by default unless the plugin needs them:
  line numbers, syntax highlighting, LSP hooks, diagnostics, completion,
  markdown parsing, and large undo history all add state.
- Do not clone large lists into closures for every render. Use `Arc`, indices,
  paging, or virtualized delegates.
- For tables and lists, prefer virtualized rendering when row count can exceed a
  few hundred or rows are expensive to paint.
- Explicitly clear large editor buffers, previews, image bytes, and row caches
  in `PluginSession::on_close` or the view's close path.

If a plugin migration changes memory-sensitive behavior, measure it:

```bash
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo build --release
target/release/qingqi &
ps -o rss= -p <pid>
kill <pid>
```

Take several samples after the window has settled. Report the before/after RSS
and which window/plugin was open.

## Architecture Rules Still Apply

Using `gpui-component` does not change Qingqi's Rust-native boundaries:

- `service.rs` and `store.rs` must not depend on `gpui` or `gpui-component`.
- Business logic must remain testable without starting GPUI.
- UI state belongs in `Session`, `Entity<T: Render>`, or window-scoped panel
  state.
- Background work still goes through services, stores, `cx.spawn`, channels, or
  short locks.
- Render functions consume view models and build elements. They must not read
  databases, scan directories, run commands, or make network requests.

Recommended flow remains:

```text
User input
  -> ViewAction
  -> Session/View state update
  -> Service/store call
  -> DTO/Page<RowVm>/StatusVm
  -> Render GPUI/gpui-component elements
```

## Component Selection Guide

Use this table before replacing hand-written UI:

| Need | Preferred Approach |
| --- | --- |
| Simple styled button | `gpui_component::button::Button` if nearby UI already uses component styling; otherwise the project UI helper is fine (`app::ui` before split, `qingqi-ui` after split) |
| Segmented tabs | `gpui_component::tab::TabBar` / `Tab` for new work |
| Boolean settings | `switch`, `checkbox`, or `radio` |
| Settings page rows | `setting` or `form` modules once visual parity is checked |
| One-line search input | Existing `app::text_input::TextInput` unless migrating that whole window |
| Multi-line plain text | Existing `TextInput` if simple; component `InputState` if editor interactions matter |
| Code/script/JSON editor | Component `InputState` in code editor mode, with highlighting only as needed |
| Long list | GPUI `uniform_list` or component virtual list; choose the one with less state churn |
| Table with columns/resizing | `gpui_component::table` |
| Sheet/dialog overlay | Only after Root-enabling the window |

## Styling Policy

Qingqi's visual target is still suishou plugin-page parity. Do not blindly use
the default shadcn-like appearance if it drifts from the target screen.

- Prefer Qingqi theme tokens (`src/app/theme.rs` before split, `qingqi-ui::theme` after split) where exact color parity
  matters.
- Component defaults are acceptable for internal tooling, `gpui-demo`, and
  features that do not yet target pixel parity.
- If a component's default color/radius/spacing conflicts with suishou parity,
  override it locally or wrap it in an adapter helper.
- Avoid adding a broad wrapper abstraction until at least two plugins use the
  same adapted component.

## Recommended Migration Order

1. Keep the global `init` in place and verify `cargo check`.
2. Update `gpui-demo` first with real component examples:
   button variants, segmented tabs, switch/checkbox, table/list, and a Root
   overlay example in a window that has been audited.
3. Migrate simple repeated controls in one small plugin.
4. Evaluate `table` or virtual list for large datasets.
5. Evaluate component editor only for JSON/API/Quick Launch use cases.
6. Consider Root-enabling plugin windows after a single plugin proves overlay
   value.
7. Update this document and `docs/workspace-split-guide.md` when a component pattern
   becomes the preferred Qingqi pattern.

## Testing Requirements

For any change that adds or uses `gpui-component`:

```bash
cd F:/develop/qingqi
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo fmt
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test
PATH=/Users/fwfx1234/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo check
```

If a change touches Root, overlays, focus, input, or plugin window lifecycle,
also manually verify:

- App starts.
- Launcher opens and closes.
- Existing plugin window can open, activate, reopen, and close.
- Closing plugin windows clears handles and does not leak stale sessions.
- Keyboard shortcuts still work for launcher and text input.

## Do Not Do These

- Do not migrate every plugin to `gpui-component` in one pass.
- Do not use component editor for every text field.
- Do not store `InputState`, `Button`, `Tab`, or other UI entities in services
  or stores.
- Do not use sheet/dialog APIs before Root-enabling that window.
- Do not keep both old and new heavy editor buffers alive for the same field.
- Do not add `webview`, `inspector`, or `tree-sitter-languages` features unless
  a specific task requires them and the memory/build impact is measured.
