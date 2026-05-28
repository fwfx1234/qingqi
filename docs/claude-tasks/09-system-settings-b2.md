You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `system-settings` section)
4. `/Users/fwfx1234/develop/suishou/src/features/system_settings/plugin.json`
5. `/Users/fwfx1234/develop/suishou/src/features/system_settings/SystemSettingsPage.qml`
6. `/Users/fwfx1234/develop/suishou/src/features/system_settings/view_model.py`
7. `/Users/fwfx1234/develop/suishou/src/app/platform/macos/permissions.py`
8. `/Users/fwfx1234/develop/suishou/src/app/plugins/importer.py`
9. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/mod.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/plugin.rs`
11. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/view.rs`
12. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/settings_store.rs`
13. `/Users/fwfx1234/develop/qingqi/src/platform/shell.rs`

Task:

- Audit the current Rust `system-settings` plugin against the suishou reference.
- Implement a conservative `Functional v1` batch focused on truthful macOS permissions and useful diagnostics actions.
- Keep the work tightly scoped to `system_settings`, small shared shell/platform helpers if needed, and the migration guide.
- Do not broaden this into a full plugin-management rewrite.

Required outcomes:

1. Make macOS accessibility state truthful:
   - show a real accessibility authorization status if the current platform can read it cheaply
   - if some permission types still cannot be checked truthfully in this batch, keep them explicitly marked as unknown / not yet implemented instead of pretending they are available
2. Add a real “open system settings” path for accessibility:
   - wire a user action that opens the relevant macOS settings page
   - refresh or re-read the permission status honestly after the action when practical
3. Make diagnostics actions useful:
   - allow opening the real data/config/log directories shown in the diagnostics area
   - use truthful success/failure notices instead of placeholder text
   - if some maintenance actions such as icon-cache cleanup are still not cheaply available in this batch, leave them explicitly disabled
4. Preserve what is already real:
   - keep theme mode switching through `ThemeStore`
   - keep persisted plugin-window retention working through `SettingsStore`
   - keep shared app-index status / rescan wiring intact
5. Plugin import remains conservative:
   - only add a directory/zip import flow if it can be done with low risk and without redesigning plugin loading
   - otherwise keep import surfaced as intentionally incomplete and document the remaining gap honestly
6. Add or extend focused tests for any helper/state logic you introduce.
7. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `system-settings` row and notes reflect the new truthful state.

Rules:

- Prefer the existing `SettingsPanel` / `SettingsStore` / `view` boundaries over inventing a new subsystem.
- Keep edits closely scoped.
- Do not rewrite unrelated plugins.
- Do not claim other permissions, plugin import, or maintenance actions are complete unless they are truly wired.
- Run `cargo fmt`, `cargo test --bin qingqi`, and `cargo check`.

At the end, print:

- exact files changed
- which `system-settings` behaviors are now real
- which `system-settings` behaviors are still missing
- exact commands you ran
