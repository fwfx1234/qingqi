You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `download-manager` section)
4. `/Users/fwfx1234/develop/suishou/src/features/download_manager/plugin.json`
5. `/Users/fwfx1234/develop/suishou/src/features/download_manager/DownloadManagerPage.qml`
6. `/Users/fwfx1234/develop/suishou/src/features/download_manager/view_model.py`
7. `/Users/fwfx1234/develop/suishou/src/features/download_manager/service.py`
8. `/Users/fwfx1234/develop/suishou/src/features/download_manager/repository.py`
9. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/manifest.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/model.rs`
11. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/store.rs`
12. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/service.rs`
13. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/view.rs`

Task:

- Audit the current Rust `download-manager` plugin against the suishou reference.
- Implement a conservative `Functional v1` parity batch that closes the biggest real user-facing gaps without rewriting the whole plugin.
- Keep the work scoped to `download_manager` plus the migration guide.

Required outcomes:

1. The plugin must support real task-management flows beyond the current minimal list:
   - multi-URL ingestion from pasted text if feasible
   - `pause all` plus `resume all`
   - `clear failed` in addition to clearing completed
   - retrying a failed or cancelled task
   - revealing/opening the download directory or downloaded file when appropriate
2. Persist real plugin settings instead of hardcoding everything in memory:
   - save root
   - timeout
   - retry limit
   - proxy URL
   - user agent / referer / cookie / custom headers
   - if `max concurrent` or `speed limit` is touched, it must either be truly enforced or be surfaced honestly as not wired yet
3. Wire the settings that are feasible into the actual downloader behavior. Do not fake support.
4. Keep render paths free of direct slow IO. If you touch task loading, prefer service APIs/snapshots over view code reading the store directly.
5. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `download-manager` row and notes describe the new truthful state.

Rules:

- Prefer conservative, local changes.
- Do not rewrite unrelated plugins.
- Do not silently pretend unsupported download/network features work.
- Add or update focused tests for service/store behavior when you add persistence, retry, or queue/control logic.
- Run `cargo fmt`, `cargo test --bin qingqi -- features::download_manager`, and `cargo check`.

At the end, print:

- exact files changed
- which suishou behaviors are now real in Qingqi
- which download-manager behaviors are still missing
- exact commands you ran
