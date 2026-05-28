You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `api-debugger`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/ApiDebuggerPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/tabs_controller.py`
6. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/model.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/store.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/service.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/view.rs`

Task:

- Continue from the current Rust `api-debugger` implementation after the environment CRUD + tab persistence batch.
- Implement a conservative follow-up batch focused on more truthful persisted request-tab draft state.
- Keep the work tightly scoped to `api_debugger`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make persisted tab drafts more truthful:
   - when practical, persist and restore a conservative subset of current request state beyond just method + url
   - prioritize existing fields that are already represented in Rust model/store code, such as active request panel/body mode, auth draft, query/path/header/cookie text, and body text
   - keep the implementation honest; if some editor state is still not wired, leave it unchanged and document the gap
2. Preserve the current real send/history/assertion path and the environment CRUD behavior already landed.
3. Keep tab IDs/store usage truthful; do not regress the current UUID-based tab persistence.
4. Add or extend focused tests for any service/store logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::api_debugger`, and `cargo check`.

Rules:

- Prefer a small Rust-native persistence improvement over a broad multi-tab redesign.
- Do not claim full suishou tab parity unless it is truly implemented.
- Keep store/service/view boundaries understandable.

At the end, print:

- exact files changed
- which `api-debugger` tab-draft persistence behaviors are now real
- which `api-debugger` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
