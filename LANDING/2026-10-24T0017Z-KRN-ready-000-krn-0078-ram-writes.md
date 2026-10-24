# Ready: 000-KRN-0078, file writes without the chain walks, data in runs

**From:** KRN · **To:** LANDING · **Written:** 2026-10-24 00:17 UTC · **Concerns:** 000-KRN-0078 (advances it; the MacBook Pro's run is left)

**Ready:** `claude/youthful-mendel-mf1soy` 62aee80b

- **Contents.**
  - Round 27's candidate (000-KRN-0168, being gated now).
  - Three commits for 0078, in `vfs_server`, `tests/fat_host.rs` and `docs/profile/threat-model.md`.
- **What changed.**
  - A file's FAT chain is walked once while it is written, not twice for each write.
  - Whole sectors of file data go past the cache in runs (`Sectors::write_run`).
  - No IDL or ABI change.
- **Run on this tree** (this session's container, QEMU TCG):
  - x86 `vfs`, `block`, `disk`, `store` and `bench`: rc=0. `ram:` writes at 5.28 MiB/s, against 898 KiB/s before.
  - Host tests `fat` (18) and `vfs_zone`.
  - Not run outside the gate: aarch64, which builds the same `vfs_server`.
- **This request** waits for round 28 (`LANDING` rule: a request posted while a candidate is gated).
