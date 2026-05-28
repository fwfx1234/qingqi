You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on the `ftp-sftp-ssh-client` section)
4. `/Users/fwfx1234/develop/suishou/src/features/ftp_sftp_ssh_client/plugin.json`
5. `/Users/fwfx1234/develop/suishou/src/features/ftp_sftp_ssh_client/FtpSftpSshClientPage.qml`
6. `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/plugin.rs`
7. `/Users/fwfx1234/develop/qingqi/src/features/registry.rs`

Task:

- Audit the current Rust implementation of `ftp-sftp-ssh-client`.
- If the implementation is already stable enough to expose as a builtin plugin, wire it into `src/features/registry.rs` and fix whatever small issues block that registration.
- If you discover a hard blocker that prevents safe registration, do not force it live. Instead, implement the smallest honest fix or status surfacing needed, then update the migration guide notes so the blocker is explicit.

Required outcomes:

1. No silent stub state. Either the plugin is registered and buildable, or the repo clearly records why it is still withheld.
2. Keep scope close to runtime wiring, immediate compile fixes, and honest status handling.
3. Update `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` with the new state.

Rules:

- Prefer conservative changes.
- Do not rewrite the whole plugin.
- Run `cargo fmt` and `cargo check`.
- At the end, print:
  - whether you registered the plugin
  - exact files changed
  - any remaining blocker
  - exact commands you ran

