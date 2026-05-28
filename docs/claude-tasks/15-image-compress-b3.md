You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `image-compress`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/image_compress/ImageCompressPage.qml`
5. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/manifest.rs`
6. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/service.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/view.rs`

Task:

- Continue from the current Rust `image-compress` implementation.
- Implement a conservative follow-up batch focused on keeping the UI updated during background work and, if low-risk, adding a truthful cancel path.
- Keep the work tightly scoped to `image_compress`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make background progress more visible/truthful:
   - while a batch is running, the UI should refresh without requiring unrelated user interaction if there is a low-risk way to poll/drain results
   - keep the state coarse-grained and honest; do not invent fake per-file percentages
2. If a low-risk cancel path is practical, add it:
   - cancel should stop processing remaining items and keep completed vs not-yet-processed items truthful
   - if cancel is too risky for this batch, leave it explicitly unimplemented and document why
3. Preserve current import/paste/reveal/overwrite/save-as/retry/remove behavior.
4. Add or extend focused tests for helper/state logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::image_compress`, and `cargo check`.

Rules:

- Prefer conservative background-state coordination over a broad redesign.
- Do not claim JobProvider integration unless it is truly implemented.
- Keep thread-safety obvious.

At the end, print:

- exact files changed
- which `image-compress` running/cancel behaviors are now real
- which `image-compress` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
