# KRN ready: a returning drive checked before it is written again, with DRV's `usb_storage` (000-KRN-0085 steps 2 and 3, 000-DRV-0058)

**Ready:** `claude/000-KRN-0085-returns-checked` 391c7d9c44

**Supersedes** DRV's `LANDING/2026-11-12T1908Z-DRV-ready-usb-storage-reports-a-device-that-came-back.md`: this branch carries `claude/000-DRV-0058-a-returned-usb-disk` (f7090144) whole, as DRV asked (`KRN/2026-11-13T1214Z-DRV-let-your-0085-branch-land-both.md`).

**Advances** 000-KRN-0085 (steps 2 and 3) and DRV's 000-DRV-0058, which DRV closes on the swapped-unasked boot once this is in.

**What it changes:**
- **`block::Device`** (`libmind/src/block.rs`): `check_returns()`, `returned()` and `reattach()`. A client that asks is told of a return and re-attaches; one that does not repeats, as before.
- **`vfs_server`** asks for it on every disk. On a return, or a drive asked again, it compares sector 0, the identity and the cached sectors (000-KRN-0079): the same medium goes on, another closes its volumes with nothing cached written, neither within 10 s is `DOES NOT ANSWER`.
- **DRV's `usb_storage`** reports a device that came back (`usb_storage/src/recovery.rs`, `tests/usb_storage_host.rs`).
- **`tests/usb_image_smoke.py`:** the unplug, the stick again, the longer copy and 000-KRN-0079's step.
- `main` 23f1f33e merged in; `scripts/host_tests.sh` as `main` has it.

**Tested** at 38cb3ed4 on the maintainer's machines, QEMU under TCG:
- acc-1: "x86: USB image" (166 s), "x86: files and block writes" (989 s);
- acc-2: "aarch64: files and the block store" (919 s);
- x32-128: "host tests" (1849 s).

Before the merge, "x86: USB image" passed twice on x32-128 at ae04efa1.
