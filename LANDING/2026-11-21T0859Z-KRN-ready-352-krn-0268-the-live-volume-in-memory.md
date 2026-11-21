# KRN: ready — the live system's boot volume in memory, ABI 36 (352-KRN-0268, with 352-UPD-0047's next steps)

**Ready:** `claude/352-KRN-0268-live-volume-in-memory` 51574f2bb9

**For round 164.** It holds `claude/352-UPD-0047-installer-iso` 8237936bb2, which round 163 is gating; `main` cb5dc7f6e0 merged in.

**What it lands:**
- **352-KRN-0268 (ABI 36, handoff 36):**
  - the loader names an El Torito entry (`VOLUME_ELTORITO`) and reads its FAT volume into memory, at most 512 MiB: `BOOT: THE VOLUME READ INTO MEMORY: n KB`;
  - `BootInfo.live_volume` and `live_volume_len`; the kernel lends those pages read-only through `PLATFORM_BOOT_VOLUME`, and answers `ERR_NOT_FOUND` on any other boot;
  - `init` starts `ramdisk#2` with it on a live boot alone, otherwise `ramdisk#2 NOT STARTED: NO BOOT VOLUME IN MEMORY: NOT A LIVE BOOT`;
  - `ramdisk` serves a lent object read-only; `vfs_server` mounts it first, matched by the manifest the loader verified.
- **352-UPD-0047 steps 2 and 3, in part:**
  - `make_iso.py --slots`: the ISO's volume in slots, sized with room for slot B;
  - on a live boot the updater fills no slot and says `A LIVE SYSTEM`.
- **`tests/qemu_smoke.py`:** the `iso` suite on both architectures (in `x86: keys, shell, tools` and `aarch64: tools`), from a CD-ROM (IDE on x86, SCSI on aarch64) and a USB disk: the volume in memory, programs run from it, a write refused, the ISO unchanged.
- **Docs:** docs/api, docs/profile/bootstrap.md, docs/write-disk.md and docs/update/updater.md (EN and RU), and the issues' progress.

**Tested** in this session's container (QEMU 8.2.2, OVMF and AAVMF):
- the `iso` suite, x86 (4 CPUs) and aarch64, PASS;
- x86's `services`, `shell`, `vfs`, `tools`, `boot` and `memory` suites, PASS, `ramdisk#2` not started there;
- aarch64's `normal` and `devicetree` suites, PASS;
- `tests/make_iso_test.py` 5 of 5.

The rest of the groups run in the gate. Not covered: a real machine (MC-12.1).
