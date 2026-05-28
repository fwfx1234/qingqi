You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/service.rs`
4. `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/view.rs`

Task:

- Continue from the current Rust `quick_launch` implementation.
- Fix the current compile break in `quick_launch/view.rs` caused by an incomplete transition from `latest_run_statuses: HashMap<i64, RunStatus>` to `latest_run_summaries: HashMap<i64, RunSummary>`.
- Keep the work tightly scoped to `quick_launch/view.rs`.

Known current compile failures:

- `self.latest_run_statuses.clear()` should be reconciled with the current `latest_run_summaries` field
- `self.latest_run_statuses.insert(run.action_id, run.status)` should be reconciled with `RunSummary`
- `let latest_statuses = self.latest_run_statuses.clone()` should be reconciled with the current summary map
- `action_row(...)` and `latest_run_status_chip(...)` still use `HashMap<i64, RunStatus>` even though the view now stores `HashMap<i64, RunSummary>`

Likely current target lines in `view.rs`:

- around line `188`
- around line `708`
- around line `1070`
- around line `1372`
- around line `2962`

Required outcomes:

1. Make `QuickLaunchView` internally consistent around the current `latest_run_summaries` field.
2. Keep the current conservative history/result/status-chip improvements truthful:
   - use `RunSummary` data for row chips if that is the current direction
   - do not revert back to a weaker `RunStatus`-only map unless absolutely necessary
   - if the existing UI already wants a compact "latest run" chip, prefer `RunSummary::chip_label()` plus the current summary status tone
3. Do not broaden scope beyond this compile recovery.
4. Run `cargo fmt`, `cargo test --bin qingqi -- features::quick_launch`, and `cargo check`.

Hard boundaries:

- Only edit this file:
  - `/Users/fwfx1234/develop/qingqi/src/features/quick_launch/view.rs`
- Do not edit any other file under `src/features` or `src/platform`.
- If you conclude another file must change, stop and explain that in the final summary instead of editing it.

At the end, print:

- exact files changed
- which `quick-launch` history/result/status behaviors are now real
- which `quick-launch` behaviors are still missing
- exact commands you ran
