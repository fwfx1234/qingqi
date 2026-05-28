You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `http-capture` section)
4. `/Users/fwfx1234/develop/suishou/src/features/http_capture/plugin.json`
5. `/Users/fwfx1234/develop/suishou/src/features/http_capture/HttpCapturePage.qml`
6. `/Users/fwfx1234/develop/qingqi/src/features/http_capture/plugin.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/http_capture/store.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/http_capture/model.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/http_capture/view.rs`

Task:

- Replace the current placeholder `HTTP 抓包 - 功能开发中` panel with a truthful `Functional v1` foundation using the existing Rust store/model layer.
- You do not need to finish live traffic capture in one batch.
- You do need a real UI path that reads from persisted capture data, shows empty state, supports at least one real filter/search flow if feasible, and has explicit non-fake status around capture engine readiness.

Required outcomes:

1. The plugin UI must no longer be a one-line placeholder.
2. The panel must use real store-backed data access or clearly show an empty state when the database has no exchanges.
3. If live capture is not wired, say so explicitly in the UI/status rather than pretending capture is active.
4. Keep code aligned with the repo architecture.
5. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` with the new current state.

Rules:

- Keep the task scoped to `http_capture` and immediate wiring only.
- No broad UI redesign outside this plugin.
- Run `cargo fmt` and `cargo check`.
- At the end, print:
  - changed files
  - what is now real versus still missing
  - exact commands you ran

