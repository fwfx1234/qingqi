You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `api-debugger`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/ApiDebuggerPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/EnvManagerDialog.qml`
6. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/tabs_controller.py`
7. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/model.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/store.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/service.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/view.rs`

Task:

- Audit the current Rust `api-debugger` implementation against the suishou reference.
- Implement a conservative `Functional v1` batch focused on real environment management actions and more truthful request-tab persistence.
- Keep the work tightly scoped to `api_debugger`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make environment management more real:
   - if the store/service already support create/update/delete/list for environments, wire a conservative subset of those actions into the existing env manager UI
   - keep the actions truthful; if import/export is still not implemented, leave it visibly inactive or as plain text
2. Improve request-tab truthfulness:
   - use the existing `http_tabs` store more meaningfully than a synthetic `tab-{index}` placeholder when practical
   - persist and restore a conservative subset of open tab state so the current UI is less misleading
   - if close/delete of persisted tabs is low-risk, wire it; otherwise keep tab controls honest
3. Preserve the current real send/history/assertion path.
4. Add or extend focused tests for any service/store logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::api_debugger`, and `cargo check`.

Rules:

- Prefer small Rust-native improvements over broad redesign.
- Do not claim full multi-tab parity or env import/export unless it is truly implemented.
- Keep UI/editor/store boundaries understandable.

At the end, print:

- exact files changed
- which `api-debugger` environment/tab behaviors are now real
- which `api-debugger` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
