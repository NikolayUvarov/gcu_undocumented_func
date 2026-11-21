# KRN: ready — the installer's service, `install.wit` in `vfs_server`, ABI 37 (352-KRN-0269)

**Ready:** `claude/352-KRN-0269-installer-service` d98ac3a4b0

**For round 165.** It holds `claude/352-KRN-0268-live-volume-in-memory` 3cdc8da0ad (ABI 36), which round 164 is gating, and `main` 689c7f5526.

**What it lands:**
- **`vfs_server/src/install.rs`, the writer:**
  - an MBR without the ESP, and no GPT left at either end;
  - the live volume sector for sector at 1 MiB, read back past the cache and compared;
  - `MIND LOG`;
  - the store's partition, blank and named in the new ESP's `store.txt`;
  - the ESP's entry last.
  A disk past 2 TiB is refused.
- **`idl/install.wit` 1.0,** served by `vfs_server` to clients badged `mind::fs::BADGE_INSTALL`, on a live boot alone:
  - `disks`, with the words to type back and a refusal;
  - `install`, then a check that the new ESP holds the manifest the loader verified.
- **`init` on a live boot (an El Torito volume):**
  - no disk or partition for the block store, which runs on `ramdisk#1`;
  - every disk whole to `vfs_server`;
  - `SLOT_INSTALL` for the shell.
  `vfs_server` keeps no update zone there.
- **ABI 37:** `SLOT_INSTALL` 46, `SLOT_DYNAMIC` 47, `CAP_SLOTS` 111. Handoff 36 is unchanged.
- **Tests:**
  - `tests/install_host.rs` (6);
  - the `iso` suite's live-boot checks, both architectures;
  - the IDL fuzzer with `install`;
  - `tests/runtime.rs` at ABI 37.
- **Docs:** docs/api, docs/idl, docs/profile/bootstrap.md, docs/write-disk.md 1.10 (EN and RU). Also 352-UPD-0048's progress, and the shell's `install` command asked of APP in `requests-APP.md`.

**Tested** in this session's container (QEMU 8.2.2, OVMF and AAVMF):
- every plain `tests/*_host.rs`, `tests/runtime.rs` 47 of 47, the IDL fuzzer, `tests/idl_test.py`, `tests/make_iso_test.py`;
- the `iso` suite, x86 (4 CPUs) and aarch64, PASS: from a CD-ROM the blank disk is offered, from a USB disk the stick is refused as the live medium;
- x86's `services`, `vfs`, `store`, `shell` and `boot` suites, PASS;
- aarch64's `aarch64_smoke.py`, PASS.

The rest of the groups run in the gate. **Not covered:** an install through the shell (APP's command), a real machine (MC-12.1).
