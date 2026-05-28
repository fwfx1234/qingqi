You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `clipboard` section)
4. `/Users/fwfx1234/develop/suishou/src/features/clipboard/ClipboardWindowPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/clipboard/view_model.py`
6. `/Users/fwfx1234/develop/suishou/src/features/clipboard/runtime.py`
7. `/Users/fwfx1234/develop/suishou/src/app/services/clipboard/service.py`
8. `/Users/fwfx1234/develop/suishou/src/app/services/clipboard/backends/macos_backend.py`
9. `/Users/fwfx1234/develop/qingqi/src/platform/clipboard.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/plugin.rs`
11. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/service.rs`
12. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/history_store.rs`
13. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/view/mod.rs`
14. `/Users/fwfx1234/develop/qingqi/src/features/clipboard/view/history.rs`
15. `/Users/fwfx1234/develop/qingqi/src/app/launcher.rs`

Task:

- Audit the current Rust `clipboard` plugin against the suishou reference, focusing on file-list clipboard handling on macOS.
- Implement a conservative parity batch that makes file clipboard records real end-to-end without broad UI redesign.
- Keep the work scoped to `clipboard`, small `platform` support if needed, and the migration guide.

Required outcomes:

1. Make file clipboard capture truthful:
   - when the macOS clipboard contains file URLs / a file list, capture them as real `ClipboardItemKind::Files` records through an actual platform path
   - do not rely on plain text fallback as the only file path
   - preserve current text/image capture behavior
2. Make file record storage and detail rendering honest and useful:
   - store file records in a format that is machine-parseable and stable for later reuse
   - render file detail/preview as file-oriented content (file count, names, paths, or similarly truthful info), not as a misleading generic text blob
   - keep list subtitles / previews aligned with the suishou intent
3. Make copy-back truthful:
   - if the selected record is a file list and the current platform layer can really write files back to the system clipboard, do that
   - if full file write-back is not feasible, use an honest fallback or message path; do not pretend a plain text write is equivalent to restoring a file clipboard payload
4. Keep integration paths working:
   - `ClipboardFilter::Files` should continue to work
   - launcher clipboard context detection for file records should still detect useful file/image-file context kinds
   - background polling should continue to dedupe sensibly instead of spamming duplicate file records
5. Add or extend focused tests for the parsing/storage/platform-helper logic you introduce.
6. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `clipboard` row and notes reflect the new truthful state.

Rules:

- Prefer conservative local edits.
- Do not rewrite unrelated plugins.
- Keep architecture boundaries intact.
- If direct macOS file clipboard read/write needs a small platform helper, keep it minimal and honest.
- Do not add fake background claims or fake enabled actions.
- Run `cargo fmt`, `cargo test --bin qingqi -- features::clipboard`, and `cargo check`.

At the end, print:

- exact files changed
- which `clipboard` file-record behaviors are now real
- which `clipboard` behaviors are still missing
- exact commands you ran
