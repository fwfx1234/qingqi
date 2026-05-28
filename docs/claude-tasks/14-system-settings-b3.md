You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `system-settings`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/system_settings/SystemSettingsPage.qml`
5. `/Users/fwfx1234/develop/qingqi/src/app/theme_store.rs`
6. `/Users/fwfx1234/develop/qingqi/src/app/background.rs`
7. `/Users/fwfx1234/develop/qingqi/src/app/runtime.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/plugin.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/view.rs`

Task:

- Audit the current Rust `system-settings` plugin and theme runtime against the suishou reference.
- Implement a conservative `Functional v1` batch focused on truthful “follow system” behavior while the app is running.
- Keep the work scoped to `system-settings`, `theme_store`, and minimal app/runtime/background wiring if required.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make `ThemeMode::System` more truthful at runtime:
   - when the app is left running and the macOS appearance changes, the effective dark/light mode should update without requiring a restart if there is already a low-risk polling path available
   - if polling is used, keep it cheap and clearly scoped
2. Keep current theme persistence behavior intact.
3. Keep the existing real permissions/diagnostics actions intact.
4. Add or extend focused tests for any helper/state logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi`, and `cargo check`.

Rules:

- Prefer a conservative runtime poll or refresh hook over a redesign.
- Do not claim full OS-notification integration unless it is truly implemented.
- Do not broaden into plugin import or icon-cache cleanup in this batch.

At the end, print:

- exact files changed
- which `system-settings` runtime-follow behaviors are now real
- which `system-settings` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
