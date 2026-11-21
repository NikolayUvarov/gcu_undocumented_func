# KRN: ready — the ahci suite boots a raw disk (000-KRN-0267)

**Ready:** `claude/000-KRN-0267-ahci-suite-raw-disk` de1242a084

**What it lands:** 000-KRN-0267 (its first criterion; the issue stays open for a gate group and DRV's LEG9 run).
- **`tests/qemu_smoke.py`:** the `ahci` suite's machine boots `raw_fat_image`'s disk, not QEMU's `vvfat`, on the same AHCI controller. `vvfat` gives other bytes past a file's end between reads, which the restarted driver's medium comparison took for another medium on LEG9 (DRV, `KRN/2026-11-20T2046Z`). Without `mkfs.fat` and mtools it says SKIP, as the other raw-image suites do.
- **Text:** the issue and its row.
- **No IDL or ABI change.** A test only: nothing of it goes into an image.

**Tested** on `de1242a084` (`main` 03c88c5144), in this session's container (QEMU 8.2.2, x86, 4 CPUs): the `ahci` suite 5 of 5 PASS. `main`'s suite passes here too (3 of 3), so this QEMU does not show LEG9's failure. The group `x86: services, storage, audio` runs in the gate.
