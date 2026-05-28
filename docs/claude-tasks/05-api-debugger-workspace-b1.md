You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `api-debugger` section)
4. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/ApiDebuggerPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/service.py`
6. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/repositories/collection_repo.py`
7. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/repositories/environment_repo.py`
8. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/repositories/tab_repo.py`
9. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/model.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/store.rs`
11. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/service.rs`
12. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/view.rs`

Task:

- Audit the current Rust `api-debugger` workspace path.
- Replace the primary sample-data workspace flow with a real SQLite-backed workspace foundation using the existing store schema where possible.
- Keep this batch scoped to real workspace persistence and honest state handling. Do not try to finish the entire api-debugger plugin in one go.

Required outcomes:

1. The main collection/request workspace must no longer depend on `sample_groups()` as the normal data source.
2. The plugin should load a truthful initial workspace from the SQLite store:
   - if stored collection/request data exists, load it
   - if no collection data exists yet, create or surface a minimal honest default workspace rather than pretending a static demo project is the user's real data
3. Core workspace edits should persist across reload/reopen where feasible in this batch:
   - collection tree node create/rename/delete/move/reorder/expanded state
   - endpoint request basics from the current editor state, using existing snapshot/storage paths if available
   - environment changes must remain real
4. If some advanced parts still cannot be persisted cleanly in this batch, surface that honestly in notices/status instead of silently falling back to demo data.
5. Keep the scope conservative:
   - do not rewrite the whole request transport stack
   - do not attempt full OpenAPI / Mock / WS parity
   - do not broaden into unrelated UI redesign
6. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `api-debugger` row and notes reflect the new truthful state.

Rules:

- Prefer the existing store/model/service boundaries over inventing a new subsystem.
- Add focused tests for any new store/service persistence logic you introduce.
- Run `cargo fmt`, `cargo test --bin qingqi -- features::api_debugger`, and `cargo check`.

At the end, print:

- exact files changed
- what is now persisted for api-debugger workspace state
- what still falls back or remains missing
- exact commands you ran
