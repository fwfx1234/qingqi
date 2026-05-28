You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `download-manager` section, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/download_manager/DownloadManagerPage.qml`
5. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/model.rs`
6. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/store.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/service.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/download_manager/view.rs`

Task:

- Audit the current Rust `download-manager` implementation against the suishou reference.
- Implement a conservative `Functional v1` batch focused on truthful settings controls and richer task filtering.
- Keep the work tightly scoped to `download_manager`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make settings controls real where the Rust service already has support:
   - expose existing save-root / concurrency / timeout / retry / proxy / request-header style settings through the plugin UI if they are already service-backed
   - keep unsupported settings honestly disabled or absent instead of pretending they work
2. Improve truthful filtering:
   - extend the current task filter UI beyond only all/active/completed/failed when low-risk categories or states already exist in the data model
   - keep filters driven by real `DownloadTask` status/category data, not placeholder counters
3. Preserve the current real download path:
   - do not regress add/start/pause/resume/cancel/retry/open behavior
   - keep view/service boundaries intact
4. Add or extend focused tests for any model/store/service logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi`, and `cargo check`.

Rules:

- Prefer conservative local edits over redesign.
- Keep long-running work out of render paths.
- Do not rewrite unrelated plugins or shared infrastructure.
- If a UI affordance is still missing a real backend path, surface it honestly.

At the end, print:

- exact files changed
- which `download-manager` settings/filter behaviors are now real
- which `download-manager` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
