You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `api-debugger` section)
4. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/ApiDebuggerPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/service.py`
6. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/request_sender.py`
7. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/script_service.py`
8. `/Users/fwfx1234/develop/suishou/src/features/api_debugger/variable_service.py`
9. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/model.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/store.rs`
11. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/service.rs`
12. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/script_service.rs`
13. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/variable_service.rs`
14. `/Users/fwfx1234/develop/qingqi/src/features/api_debugger/view.rs`

Task:

- Audit the current Rust `api-debugger` request execution path against the suishou reference.
- Implement a conservative `Functional v1` transport/history batch focused on truthful HTTP request execution and response persistence.
- Keep the work scoped to `api_debugger` plus the migration guide. Do not try to finish WebSocket, OpenAPI import, or the full tabbed workspace in one go.

Required outcomes:

1. Make HTTP execution truthful and explicit:
   - if the current Rust request path is already real, keep it and fix the missing integration pieces around it
   - if some request modes or auth/body combinations are not really supported in this batch, surface that honestly in logs/notices instead of pretending they are complete
2. Persist request/response history through the existing SQLite store:
   - a real send should write an `http_history` entry using the existing schema or a minimal migration if needed
   - the stored history should include meaningful method/url/status/title/response content rather than placeholder data
3. Use the existing variable/script helpers where feasible:
   - request URL / headers / body should continue to resolve environment variables truthfully
   - if the current `script_service` can already run basic assertions or extractions, wire the conservative subset into the send flow or response logs
   - if some script behavior is still missing, keep it honest and document it
4. Keep UI behavior honest:
   - UI send must stay non-blocking
   - response panel should reflect the real latest response
   - status / notice text should distinguish success, failure, and unsupported subfeatures clearly
5. Keep tabs/workspace scope conservative:
   - do not build full tab management UI in this batch
   - if there is an obvious low-risk way to persist the current editor state into the existing `http_tabs` schema when sending, do it; otherwise leave tabs for a later batch
6. Add or extend focused tests for the request/history/script integration you introduce.
7. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `api-debugger` row and notes reflect the new truthful state.

Rules:

- Prefer the existing store/model/service boundaries over inventing a new subsystem.
- Prefer conservative fixes over broad redesign.
- Do not rewrite unrelated plugins.
- Do not claim unsupported HTTP/WS/OpenAPI capability as done.
- Run `cargo fmt`, `cargo test --bin qingqi -- features::api_debugger`, and `cargo check`.

At the end, print:

- exact files changed
- which `api-debugger` HTTP/response/history behaviors are now real
- which `api-debugger` behaviors are still missing
- exact commands you ran
