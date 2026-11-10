# KRN ready: the block store's own partition as a window of its driver (307-KRN-0177), with STO's windows (307-DRV-0054)

**Ready:** `claude/307-KRN-0177-store-partition` 1c1493d41d2f

**From:** the kernel session (KRN, PRT, NET)

**What it lands** (on `main` bd03ca93):
- **STO's windows, 307-DRV-0054**, as STO's branch had them at c3938027: `block.wit` 1.2 `window`, and `Windows` in `block_protocol`. No request of STO's for them is waiting. A later one for the same commits lands nothing more.
- **307-KRN-0177:**
  - `libmind/src/partition.rs`: MBR and GPT (both CRCs), and the store partition's rule: its type and superblock both, or the one `store.txt` names;
  - **`block::serve` keeps a buffer per client badge.** Before, two clients of one disk read into each other's buffer;
  - **`init`:** when no VirtIO disk is the store's, it reads and logs every `nvme`/`ahci`/`usb_storage`/`virtio_blk` disk's table and takes one candidate (none of two or more). It sets window 1 on its driver and gives `blockstore` that window's client. After the driver restarts, it checks the partition again before setting the window again;
  - `tests/usb_image_smoke.py` checks all this;
  - `docs/profile`'s grant and TCB rows.
- **Issue text:** 307-KRN-0229 (new, `vfs_server` bounded to its own partitions), and 000-KRN-0085's step 5 (the windows after a return).

**Tested, at bfd38a3e** (since then only `main`'s merge, which brought `land.py`'s pool rule, and issue text):
- **acc-1, through Effector:**
  - "x86: USB image": the main boot with the store on the stick's partition 3 beside `vfs_server`, and four more boots: a blank NVMe disk, two candidates, a blank partition, and the same named in `store.txt`. Passed, 98 s.
  - `usb_image_smoke.py --arch aarch64`: passed.
- **acc-2:** "x86: files and block writes", "x86: services, storage, audio" and "x86: NVMe boot disk". Passed.
- **PCU:** "aarch64: files and the block store" and "aarch64: NVMe boot disk". Passed.
- **The kernel session's container:** the host tests passed, `tests/manifest_test.py` aside (this container's Python, on `main` too).

**What changes for other suites:** only a boot of a real image (`usb=True` or `raw=True`), as the USB image suite's, now puts the store on partition 3. Suites on `fat:` disks keep the store where it was.
