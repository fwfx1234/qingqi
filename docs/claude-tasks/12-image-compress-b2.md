You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `image-compress` section, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/image_compress/ImageCompressPage.qml`
5. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/plugin.rs`
6. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/service.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/view.rs`

Task:

- Audit the current Rust `image-compress` plugin against the suishou reference.
- Implement a conservative `Functional v1` batch focused on moving batch compression off the UI thread and keeping result state honest while work is running.
- Keep the work tightly scoped to `image_compress` plus tiny shared helpers only if absolutely required.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make batch compression non-blocking:
   - the main “run compression” action should no longer do the full batch loop synchronously in the render/UI path
   - background execution may be conservative, but it must keep the UI responsive
2. Keep result state truthful while work is in progress:
   - represent entries that are waiting/running/succeeded/failed honestly
   - do not show success before a file is actually written
   - if precise per-file progress is not cheap in this batch, use truthful coarse-grained running state instead of fake percentages
3. Preserve current real capabilities:
   - keep import/paste/reveal/overwrite/save-as/retry/remove behavior working
   - do not regress clipboard image import or path normalization
4. Add or extend focused tests for any helper/state logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi`, and `cargo check`.

Rules:

- Prefer a small Rust-native background execution path over a broad redesign.
- Keep shared state boundaries understandable and thread-safe.
- Do not rewrite unrelated plugins.
- If some operations must remain main-thread only, keep that split explicit and honest.

At the end, print:

- exact files changed
- which `image-compress` batch behaviors are now real
- which `image-compress` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
