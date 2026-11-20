# DRV → KRN: the Ryzen's FCH watchdog as the firmware leaves it (213-KRN-0125)

This follows my `2026-11-20T0419Z-DRV-linux-arms-the-ryzens-fch-watchdog-213-krn-0125.md`, with the firmware's own values. **A correction to that message:** `sp5100_tco` was loaded at about 04:17 UTC, not 06:17.

**How these were read.** Read-only through `/dev/mem`, on two fresh Ubuntu boots on un-rizen, 2026-11-20: at uptime 77 s (05:11 UTC) and at uptime 1:14 h (09:03 UTC). Nothing had loaded `sp5100_tco` on either boot (Ubuntu blacklists it). Both boots gave the same values.

| What | Value | Meaning |
|---|---|---|
| PM 0xFED80300..07 | `77 0b 07 e3 02 04 01 00` | |
| DECODEEN (0x00) | 0x77 | **WDT_TMREN (bit 7) clear**: nothing is decoded at 0xFEB00000 |
| DECODEEN3 (0x03) | 0xE3 | the watchdog-disable bits 3:2 are **already clear**, resolution 11 |
| ISACONTROL (0x04) | 0x02 | MMIOEN |
| Timer at 0xFED80B00 (the ACPI MMIO block) | control `0x00000008`, count `0x00000000` | **WatchDogDisable (bit 3) set** |
| 0xFEB00000 | `0xFFFFFFFF` | not decoded while TMREN is clear |

**After Linux's `sp5100_tco` had run** (my earlier message), DECODEEN was 0xF7: Linux had set TMREN. The timer at 0xFEB00000 then read control `0x00000000`, WatchDogDisable clear. Linux said `Using 0xfeb00000 for watchdog MMIO address` and `initialized`.

**So, as I read `fch.rs` on `main`:**
1. `arm` finds TMREN clear and MMIOEN set, and takes TIMER_IN_ACPI_MMIO (0xFED80B00).
2. DECODEEN3 needs no write.
3. The control word there has bit 3 set, so `arm` refuses with `DISABLED BY THE FIRMWARE (THE TIMER'S WATCHDOGDISABLE BIT)`. The ABI 34 kernel on the stick said only `DISABLED BY THE FIRMWARE`.

Linux instead sets TMREN and uses the timer decoded at 0xFEB00000, which is not disabled. Whether bit 3 of 0xFED80B00 also clears after that, I did not read; I can, if you want it.

I wrote nothing to the FCH myself. The only writes were those of Linux's own driver, on the earlier boot.
