You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `app-launcher` section)
4. `/Users/fwfx1234/develop/suishou/src/features/app_launcher/plugin.json`
5. `/Users/fwfx1234/develop/suishou/src/features/app_launcher/runtime.py`
6. `/Users/fwfx1234/develop/qingqi/src/platform/apps.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/app_launcher/manifest.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/app_launcher/plugin.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/app_launcher/service.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/app_launcher/store.rs`

Task:

- Audit the current Rust `app-launcher` implementation with emphasis on icon-cache failure handling and alias coverage.
- Implement a conservative `Functional v1` hardening batch focused on truthful icon behavior, not a UI redesign.
- Keep the work scoped to `app_launcher`, `platform/apps.rs`, and the migration guide.

Required outcomes:

1. Harden icon-cache truthfulness:
   - treat missing, zero-byte, or obviously broken cached icon files as cache misses instead of successful cache hits
   - do not hand broken icon paths to the UI when the file is no longer usable
2. Make refresh behavior honest:
   - if icon extraction fails, fall back cleanly to the existing letter tile instead of leaving a misleading cached path behind
   - avoid repeated noisy failures in the same scan when a cached icon is already known bad
3. Improve alias coverage conservatively:
   - add low-risk normalized aliases or search text improvements for common macOS app-name / bundle-id variants
   - keep existing search behavior intact for name, path, bundle id, and current aliases
4. Keep the existing metadata-first scan / background refresh flow intact.
5. Add or extend focused tests for cache validation and alias generation/search behavior.
6. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` so the `app-launcher` row and notes reflect the hardened current state.

Rules:

- Prefer small local changes over redesign.
- Do not rewrite unrelated features.
- Do not claim the cache is perfect if some corruption cases still intentionally fall back.
- Run `cargo fmt`, `cargo test --bin qingqi`, and `cargo check`.

At the end, print:

- exact files changed
- which `app-launcher` icon/search behaviors are now real
- which `app-launcher` behaviors are still missing
- exact commands you ran
