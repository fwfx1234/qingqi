You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `system-settings`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/system_settings/SystemSettingsPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/system_settings/view_model.py`
6. `/Users/fwfx1234/develop/qingqi/src/core/storage.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/plugin.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/system_settings/view.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/app_launcher/service.rs`
10. `/Users/fwfx1234/develop/qingqi/src/platform/apps.rs`

Task:

- Audit the current Rust `system-settings` implementation against the suishou reference.
- Implement a conservative follow-up batch focused on replacing obviously disabled maintenance actions with truthful low-risk behavior.
- Keep the work scoped to `system_settings` plus minimal `app_launcher` / `platform` helpers if required.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make plugin-directory maintenance more truthful:
   - add a stable imported-plugin root path if a low-risk location is already practical in Qingqi
   - wire a real “open plugin directory” style action if possible
   - if actual plugin import is still too broad for this batch, keep import itself visibly unimplemented and document that clearly
2. Make icon-cache maintenance more truthful:
   - replace the disabled “清理图标缓存” behavior with a real cache cleanup action
   - keep the action conservative: remove cached app icon files, report a truthful result, and avoid broad unrelated state resets
3. Preserve current theme, retention, app-index rescan, permissions, and diagnostics behavior.
4. Add or extend focused tests for helper/state logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::system_settings`, `cargo test --bin qingqi -- features::app_launcher`, and `cargo check`.

Rules:

- Prefer small truthful maintenance actions over a broad plugin-management redesign.
- Do not claim full plugin import support unless it is truly implemented.
- Keep UI affordances honest when a capability is still missing.

At the end, print:

- exact files changed
- which `system-settings` maintenance behaviors are now real
- which `system-settings` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
