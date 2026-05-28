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
- Implement a conservative follow-up batch focused on making the right-side detail inspector more truthful and useful.
- Keep the work tightly scoped to `http_capture`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Make the selected-exchange detail area more real:
   - use the existing `request_headers_json`, `response_headers_json`, `request_body`, `response_body`, `duration_ms`, and related model fields more meaningfully than a flat key/value summary
   - if practical, add a conservative local tab/segmented detail switcher using the existing `DetailTab` enum (`请求头`, `请求体`, `响应头`, `响应体`, `计时`)
   - render truthful empty states when a selected section has no data
2. If practical, expose at least one additional filter already supported by the Rust model, such as `hide_static`, but keep it low-risk.
3. Preserve the current explicit “capture engine not wired” status and the real store-backed list/filter/pagination path.
4. Add or extend focused tests for helper/model logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::http_capture`, and `cargo check`.

Rules:

- Prefer a small inspector improvement over a broad capture-engine redesign.
- Do not claim live proxy/certificate/system-proxy support unless it is truly implemented.
- Keep UI behavior honest and low-risk.

At the end, print:

- exact files changed
- which `http-capture` inspector/filter behaviors are now real
- which `http-capture` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
