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

- Continue from the current Rust `api-debugger` implementation.
- Implement a conservative follow-up batch focused on persisted request-tab draft state.
- Keep the work tightly scoped to `api_debugger`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make persisted tab drafts more truthful:
   - persist and restore a conservative subset of request-tab draft state beyond only method + url
   - prefer fields that are already present in the Rust model/store path, such as active panel/body mode, auth draft, query/path/header/cookie text, and body text
2. Preserve the current send/history/assertion path and current UUID-based tab persistence.
3. Add or extend focused tests for service/store logic you introduce.
4. Run `cargo fmt`, `cargo test --bin qingqi -- features::api_debugger`, and `cargo check`.

Hard boundaries:

- Only edit these Rust files in this batch:
  - `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/service.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/view.rs`
- Do not edit any other file under `src/features` or `src/platform`.
- If you conclude another file must change, stop and explain that in the final summary instead of editing it.

At the end, print:

- exact files changed
- which `api-debugger` tab-draft persistence behaviors are now real
- which `api-debugger` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
