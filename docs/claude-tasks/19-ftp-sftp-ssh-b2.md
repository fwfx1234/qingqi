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
9. `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/view.rs`
10. `/Users/fwfx1234/develop/qingqi/src/features/ftp_sftp_ssh_client/transfer.rs`

Task:

- Continue from the current Rust `ftp-sftp-ssh-client` implementation.
- Implement a conservative follow-up batch focused on making transfer queue control more truthful and usable.
- Keep the work tightly scoped to `ftp_sftp_ssh_client`.
- Do not edit `/Users/fwfx1234/develop/qingqi/docs/migration-guide.md` in this batch because other Claude tasks may be running in parallel on the same workspace. Report the suggested migration-guide note in your final summary instead.

Required outcomes:

1. Improve transfer queue truthfulness:
   - if cancel support already exists in `TransferService` / `FtpSftpSshService`, wire a conservative cancel action into the visible transfer strip or nearby UI
   - keep completed / failed / cancelled state honest and do not fake progress
2. Improve transfer queue usability:
   - make it easier to distinguish active vs terminal transfers in the current UI
   - if practical, surface slightly richer status text from existing model state rather than only a generic progress pill
3. Preserve the current real profile CRUD, connect/disconnect, remote listing, upload/download, and clear-finished behaviors.
4. Add or extend focused tests for helper/state logic you introduce.
5. Run `cargo fmt`, `cargo test --bin qingqi -- features::ftp_sftp_ssh_client`, and `cargo check`.

Rules:

- Prefer a small transfer-control improvement over a broad SSH terminal redesign.
- Do not claim suishou terminal bridge parity unless it is truly implemented.
- Keep service/view boundaries understandable and thread-safety obvious.

At the end, print:

- exact files changed
- which `ftp-sftp-ssh-client` transfer behaviors are now real
- which `ftp-sftp-ssh-client` behaviors are still missing
- a short suggested note for `docs/migration-guide.md` (do not edit the file)
- exact commands you ran
