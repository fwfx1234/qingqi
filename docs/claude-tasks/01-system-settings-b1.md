You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `system-settings` section)
4. `/Users/fwfx1234/develop/suishou/src/features/system_settings/plugin.json`
5. `/Users/fwfx1234/develop/suishou/src/features/system_settings/SystemSettingsPage.qml`
6. `/Users/fwfx1234/develop/suishou/src/features/system_settings/view_model.py`
7. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/plugin.rs`

Task:

- Continue from the current in-progress `system-settings` changes and finish them.
- Keep edits tightly scoped to:
  - `/Users/fwfx1234/develop/qingqi/src/features/system_settings/`
  - `/Users/fwfx1234/develop/qingqi/src/features/registry.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/app_launcher/plugin.rs` only if required to preserve the shared `AppIndexService` wiring that already exists.
- Do not edit any other feature files.
- Do not restart from scratch or redesign the feature.
- Focus on making the current implementation compile and satisfy the listed outcomes.

Required outcomes:

1. The plugin must stop showing fake hard-coded operational state where real state can be sourced cheaply.
2. Keep real theme mode switching working through `ThemeStore`.
3. Add a real, persisted plugin-window retention setting if the current Rust app has a stable place to store it.
4. Show truthful diagnostics paths based on `AppPaths`, not placeholder strings.
5. Where a capability is not yet implemented (for example plugin import or permissions), surface an explicit disabled/error status instead of pretending it works.
6. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `system-settings` notes reflect the new current state.

Rules:

- Follow the repo’s Rust-native architecture. No GPUI work in services; no fake success paths.
- No drive-by cleanup.
- Do not invoke background task planning tools or expand to unrelated files.
- There is already partial work in:
  - `src/features/system_settings/plugin.rs`
  - `src/features/system_settings/view.rs`
  - `src/features/system_settings/settings_store.rs`
  - `src/features/system_settings/mod.rs`
  - `src/features/registry.rs`
  - `src/features/app_launcher/plugin.rs`
- Preserve those changes where useful and only repair what is needed.
- The current local `cargo check` failures are in `system_settings/view.rs` and `system_settings/plugin.rs`; fix those first.
- Run `cargo fmt` and `cargo check`.
- At the end, print:
  - changed files
  - what real functionality was added
  - what remains intentionally incomplete
  - exact commands you ran
