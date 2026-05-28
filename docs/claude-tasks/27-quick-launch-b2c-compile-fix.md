You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `quick-launch`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/model.rs`
5. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/service.rs`
6. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/store.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/view.rs`

Task:

- Continue from the current Rust `quick-launch` implementation.
- The workspace currently has a compile break in `quick_launch` after an interrupted follow-up batch.
- Fix the current `quick_launch` implementation conservatively so the repo compiles again, while preserving any truthful history/result improvements already landed.
- Keep the work tightly scoped to `quick_launch`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Known current compile failure:

- `src/features/quick_launch/view.rs` still references `latest_run_statuses` in some places, but the view now stores `latest_run_summaries`.

Required outcomes:

1. Restore compile correctness for `quick_launch`:
   - reconcile the current history/result UI changes with the actual fields/types now present in `QuickLaunchView`
   - do not leave partial renames or dead references behind
2. Preserve the intended conservative improvement scope:
   - latest-result / history detail should stay aligned with current stored run data
   - if rerun/history/status chips were partially added, keep only the truthful pieces that fit the current service/model data
3. Add or adjust focused tests only if needed for any helper/state logic you touch.
4. Run `cargo fmt`, `cargo test --bin qingqi -- features::quick_launch`, and `cargo check`.

Hard boundaries:

- Only edit these Rust files in this batch:
  - `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/service.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/store.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/view.rs`
- Do not edit any other file under `src/features` or `src/platform`.
- If you conclude another file must change, stop and explain that in the final summary instead of editing it.

At the end, print:

- exact files changed
- which `quick-launch` history/result behaviors are now real
- which `quick-launch` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
