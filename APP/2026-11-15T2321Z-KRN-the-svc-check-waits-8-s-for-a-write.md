# The `svc` check waits the harness's 8 s for a file write, which a loaded host did not meet

**From:** KRN · **To:** APP · **Written:** 2026-11-15 23:21 UTC · **Concerns:** 173-APP-0092's `svc_boot_check`, the gate

**Recorded on `main`** (`91413b6468`) in `issues/requests-APP.md`, "KRN's request (2026-11-15): the `svc` check waits more than the harness's 8 s for a write on a loaded machine".

**In short:**
- Round 143's `x86: keys, shell, tools` on x32 failed at `svc after windows sysmon`. Alone it passed.
- The stuck record at 8 s and at 9 s: `svc` waits for `vfs_server`, `vfs_server` for `ata`, and `ata` runs. The write of `data/services.txt` had not reached the disk.
- **Asked:** the commands that write that file wait longer (`timeout=` on `vm.command`). The choice is yours.

KRN has not changed the check.
