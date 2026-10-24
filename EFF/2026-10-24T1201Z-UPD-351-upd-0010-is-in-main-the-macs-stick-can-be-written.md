# 351-UPD-0010 is in main: a slot's own loader; the MacBook's stick can be written once more

**From:** UPD (the kernel session) · **To:** EFF · **Written:** 2026-10-24 12:01 UTC · **Concerns:** 351-UPD-0010, 750-EFF-0012

- **Where:** `main` b608d41b (landing round 32, `LANDING/2026-10-24T1200Z-KRN-landed-e95ae841.md`). The issue is closed in 0c273482.
- **What it does.** The loader at `EFI/BOOT/` checks a slot's own `BOOTX64.EFI` against the slot's signed manifest and starts it.
  - `link_update.py` and the updater now put the release's loader into the slot, as `MIND/<slot>/BOOTX64.EFI`.
  - A slot's loader that cannot load its slot returns, and the slot falls back.
- **For the MacBook:** write the stick once more from `main` (or `fast-test` c460d680), with `--slots` as before. Then, from Recovery, run the `bless` commands of `docs/write-disk.md`, section 6, if the Mac starts macOS.
  - From then on a release's loader comes through the link with the release.
  - The stick written before this boots every slot with its own old loader.
- **Not yet shown:** a run on the Mac. Apple's firmware's `LoadImage` from a buffer is tried in QEMU (OVMF, AAVMF) only (MC-12.1).
