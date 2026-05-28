You are Claude Code working inside `/Users/fwfx1234/develop/qingqi`.

Read these files before editing:

1. `/Users/fwfx1234/develop/qingqi/AGENT.md`
2. `/Users/fwfx1234/develop/qingqi/docs/core-architecture-spec.md`
3. `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` (focus on `ftp-sftp-ssh-client`, but do not edit the migration guide in this batch)
4. `/Users/fwfx1234/develop/suishou/src/features/ftp_sftp_ssh_client/FtpSftpSshClientPage.qml`
5. `/Users/fwfx1234/develop/suishou/src/features/ftp_sftp_ssh_client/view_model.py`
6. `/Users/fwfx1234/develop/suishou/src/features/ftp_sftp_ssh_client/service.py`
7. `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/model.rs`
8. `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/service.rs`
9. `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/transfer.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/view.rs`

Task:

- Continue from the current Rust `ftp-sftp-ssh-client` implementation.
- Fix the current failing transfer-state follow-up conservatively, then add a small UI truthfulness improvement only if it fits within the hard boundaries below.
- Keep the work tightly scoped to `ftp_sftp_ssh_client`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Restore transfer-state correctness:
   - make the current `cancel` and `clear_finished` behavior pass the existing Rust tests again
   - keep queued/running/completed/failed/cancelled state honest and do not fake progress
2. If practical within the hard boundaries, make the visible transfer strip slightly more truthful:
   - distinguish active vs terminal transfers more clearly
   - surface richer existing status text when it already exists in current state
3. Preserve the current real profile CRUD, connect/disconnect, remote listing, upload/download, and clear-finished affordances.
4. Add or extend focused tests for helper/state logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::ftp_sftp_ssh_client`, and `cargo check`.

Hard boundaries:

- Only edit these Rust files in this batch:
  - `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/transfer.rs`
  - `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/view.rs`
- Do not edit any other file under `src/features` or `src/platform`.
- If you conclude another file must change, stop and explain that in the final summary instead of editing it.

At the end, print:

- exact files changed
- which `ftp-sftp-ssh-client` transfer behaviors are now real
- which `ftp-sftp-ssh-client` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
