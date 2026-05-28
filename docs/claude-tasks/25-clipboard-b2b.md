You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `clipboard`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/clipboard/ClipboardWindowPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/clipboard/view_model.py`
6. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/service.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/history_store.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/view/mod.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/view/history.rs`
10. `/Users/fwfx1234/develop/qingqi/src/platform/shell.rs`

Task:

- Continue from the current Rust `clipboard` implementation.
- Implement a conservative follow-up batch focused on making the detail pane more truthful and useful for file records.
- Keep the work tightly scoped to `clipboard` and, if needed, `platform/shell.rs`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Improve file-record detail usefulness:
   - if a selected record is a file list, make it easier to inspect and act on the stored paths than the current generic copy/pin/delete strip
   - if low-risk, add truthful actions like opening the parent directory or revealing an item when the path exists
   - keep missing-path cases honest instead of pretending the action worked
2. Keep existing real behavior intact:
   - do not regress background capture, search/filter, pin/delete/clear, image preview, or copy-back behavior
3. Add or extend focused tests for helper/state logic you introduce.
4. Run `cargo fmt`, `cargo test --bin qingqi -- features::clipboard`, and `cargo check`.

Hard boundaries:

- Only edit these Rust files in this batch:
  - `/Users/fwfx1234/develop/qingqi/src/features/clipboard/history_store.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/clipboard/view/mod.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/clipboard/view/history.rs`
  - `/Users/fwfx1234/develop/qingqi/src/platform/shell.rs`
- Do not edit any other file under `src/features` or `src/platform`.
- If you conclude another file must change, stop and explain that in the final summary instead of editing it.

At the end, print:

- exact files changed
- which `clipboard` file-detail/path behaviors are now real
- which `clipboard` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
