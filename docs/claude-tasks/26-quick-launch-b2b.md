You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `quick-launch`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/quick_launch/QuickLaunchWindowPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/quick_launch/view_model.py`
6. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/model.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/service.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/store.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/view.rs`

Task:

- Continue from the current Rust `quick-launch` implementation.
- Implement a conservative follow-up batch focused on making run history and result handling more useful and truthful.
- Keep the work tightly scoped to `quick_launch`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Improve run history usefulness:
   - use the existing stored `QuickRun` data to make it easier to distinguish success / failed / timeout / stopped runs at a glance
   - if low-risk, add a truthful rerun affordance from recent history or result detail
2. Improve result handling truthfulness:
   - keep the latest-result / history detail surfaces aligned with stored run data
   - if practical, expose a little more of the existing run metadata (duration, exit code, time) without redesigning the whole page
3. Preserve current create/edit/duplicate/enable/disable/delete, parameter sheet, stop/timeout, and dynamic-command behavior.
4. Add or extend focused tests for helper/service logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::quick_launch`, and `cargo check`.

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
