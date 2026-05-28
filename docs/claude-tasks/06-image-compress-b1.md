You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `image-compress` section)
4. `/Users/fwfx1234/develop/suishou/src/features/image_compress/ImageCompressPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/image_compress/view_model.py`
6. `/Users/fwfx1234/develop/suishou/src/features/image_compress/service.py`
7. `/Users/fwfx1234/develop/qingqi/src/platform/clipboard.rs`
8. `/Users/fwfx1234/develop/qingqi/src/platform/shell.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/plugin.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/service.rs`
11. `/Users/fwfx1234/develop/qingqi/src/features/image_compress/view.rs`

Task:

- Audit the current Rust `image-compress` plugin against the suishou reference.
- Implement a conservative `Functional v1` parity batch focused on real input/output workflows rather than broad visual redesign.
- Keep the work scoped to `image_compress` plus the migration guide.

Required outcomes:

1. Make clipboard import more truthful and useful:
   - if the clipboard contains an actual image payload, support importing it into the queue through a real file/materialization path
   - if only text/file paths are available, keep that path working
   - clearly distinguish any cases where overwrite-original should be disallowed for clipboard-only images
2. Add real result actions for compressed entries, aligned with what the repo/platform can honestly support:
   - reveal/open result in Finder
   - save-as to an explicit target path if feasible
   - overwrite original when the source is a real file and the operation is safe
   - retry failed entries
   - if copying a compressed result back to the clipboard is feasible via `platform::clipboard`, implement it; otherwise surface an honest disabled/message path
3. Keep compression behavior truthful:
   - do not fake background jobs or async work if the plugin is still synchronous in this batch
   - do not show action buttons that do nothing
4. Add or extend focused tests for the new parsing/materialization/path-handling logic you introduce.
5. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `image-compress` row and notes reflect the new truthful state.

Rules:

- Prefer conservative local edits.
- Do not rewrite unrelated plugins.
- Keep the existing architecture boundaries intact.
- Run `cargo fmt`, `cargo test --bin qingqi -- features::image_compress`, and `cargo check`.

At the end, print:

- exact files changed
- which `image-compress` behaviors are now real
- which behaviors are still missing
- exact commands you ran
