# 213-KRN-0126 — The kernel's DMA limit holds the Realtek radio's region: `rtw89_wifi` no longer fails with `NoMemory`

**Type:** kernel (`kernel/src/scheduler.rs`), profile · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; the Ryzen's boot left) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-1.7, MC-12.1

## Problem

The drivers track reported on 2026-10-20 (`internal-dev-messages`, `ALL/2026-10-20T0652Z-DRV-rtw89-wifi-fails-to-start-no-memory.md`) that both of the Ryzen's boots of fast-test d0c2dea log:
- `[INIT] rtw89_wifi FAILED: NoMemory`;
- then `wlan NOT STARTED: NO DEVICE`.

**The cause.** `PLATFORM_DMA` hands out at most `DMA_LIMIT`, 8 MiB, in all, from the kernel arena.
- The Ryzen's other drivers already hold 2.8 MiB: `free` shows `DMA=2961408/8388608`. That is xHCI, NVMe, AHCI, `audio_gw` and `rtl8168`'s 2 MiB.
- `rtw89_wifi` asks for 8 MiB of its own ([213-KRN-0114](213-KRN-0114-realtek-driver-startup.md)).

QEMU has no RTL8852BE, so no suite reached the limit.

## Plan

1. `DMA_LIMIT` becomes 16 MiB.
   - On the Ryzen, that holds the 10.8 MiB its drivers ask for.
   - It still takes a quarter of the 64 MiB arena at most. The Ryzen's arena held 3 MiB in use, with 61 MiB in one piece.
2. docs/profile/kernel-objects.md and bootstrap.md name the new limit.

## Acceptance criteria

- [ ] The QEMU suites pass as before.
- [ ] On the Ryzen:
  - `rtw89_wifi` starts;
  - `free` shows the DMA in use under 16 MiB;
  - no driver fails with `NoMemory`.

  Recorded for that machine and build.

## Related

[213-KRN-0114](213-KRN-0114-realtek-driver-startup.md), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md), docs/profile/kernel-objects.md.
