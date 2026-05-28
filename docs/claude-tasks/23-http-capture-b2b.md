You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `http-capture`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/http_capture/HttpCapturePage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/http_capture/view_model.py`
6. `/Users/fwfx1234/develop/qingqi/src/features/http_capture/model.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/http_capture/store.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/http_capture/view.rs`

Task:

- Continue from the current Rust `http-capture` foundation.
- Implement a conservative follow-up batch focused on the right-side detail inspector and one low-risk extra filter.
- Keep the work tightly scoped to `http_capture`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make the selected-exchange detail inspector more truthful and useful:
   - use existing request/response headers/body/timing fields more meaningfully than a flat summary
   - if practical, add a conservative local detail switcher using the existing `DetailTab` enum
   - render truthful empty states for missing sections
2. If practical, expose one additional low-risk filter already supported by the Rust model, such as `hide_static`.
3. Preserve the explicit “capture engine not wired” status and store-backed list/filter/pagination behavior.
4. Add or extend focused tests for helper/model logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::http_capture`, and `cargo check`.

Hard boundaries:

- Only edit these Rust files in this batch:
  - `/Users/fwfx1234/develop/qingqi/src/features/http_capture/model.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/http_capture/view.rs`
- Do not edit any other file under `src/features` or `src/platform`.
- If you conclude another file must change, stop and explain that in the final summary instead of editing it.

At the end, print:

- exact files changed
- which `http-capture` inspector/filter behaviors are now real
- which `http-capture` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
