# Landing request: 307-DRV-0055, what a flush reaches, per block driver

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-28

**Ready:** `claude/307-DRV-0055-flush-per-driver` 3dedceeef

**What it changes:** `ahci`, `ata` (`ahci/src/cache.rs`, shared), `nvme`, `usb_storage` (`mode.rs`), `virtio_blk` and `ramdisk`; `tests/flush_host.rs` (new), `tests/nvme_host.rs`, `tests/ata_ahci_waits_host.rs`, `tests/qemu_smoke.py` (`flush_check` at the end of the `block` suite); README, docs/tools (EN and RU), docs/profile/evidence.md. No IDL, ABI, init or libmind change.
- Each driver sends its device's flush:
  - FLUSH CACHE EXT, or FLUSH CACHE without 48-bit addressing;
  - NVMe FLUSH;
  - SYNCHRONIZE CACHE(10);
  - `VIRTIO_BLK_T_FLUSH`.
- A device that says it has no write cache, or has it off, gets no command, and its flush is answered done.
- A refused or unanswered flush is answered failed, never done.
- Each outcome is logged once.
- A USB disk's cache is not asked (`ASK_CACHING_PAGE` off), so its flush is always sent. The Ryzen's JMicron bridge lost the next command after the caching page's ask.

**It advances:** 307-DRV-0055. QEMU's criterion is met, and the Ryzen's boot stick is shown. The MacBook's and the DEXP's sticks are still to be read.

**Tested** (LEG9, x86):
- by the agent at bd5474f26:
  - all of `scripts/host_tests.sh` exits 0, 283 `test result: ok`;
  - "x86: services, storage, audio", "x86: files and block writes", "x86: NVMe boot disk" and "x86: USB image" PASS;
  - `ARCH=aarch64 ./02_build.sh` builds;
- by me, after the `usb_storage` changes:
  - `tests/flush_host.rs` 7 of 7, `usb_storage_host` 14 of 14;
  - on bench-54c (`main` c2318c10b + this branch at 21b02376f): "x86: keys, shell, tools" PASS 590 s, "x86: files and block writes" PASS 1077 s with `flush_check`, "x86: USB image" PASS 192 s.

  "x86: USB image" failed once on bench-54b: the swap of the same stick lost the race between the recovery's tries and usb_host's enumeration (noted in the issue, for 000-DRV-0058). It passed when run alone.

**On the Ryzen** (bench-54c, release 202611281303, ABI 40):
- `[USB] STORAGE: FLUSH: SYNCHRONIZE CACHE(10) SENT (ITS CACHE NOT ASKED), THE DEVICE SAID DONE`;
- `BLOCKSTORE … DURABILITY=FLUSHED`;
- no lost READ after the probe.

In `fast-test` (9488074c4).
