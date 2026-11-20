# KRN: ready — the AMD FCH watchdog sets WDT_TMREN as Linux does (213-KRN-0125)

**Ready:** `claude/213-KRN-0125-fch-tmren` 0ea1893c9a

**What it lands:** 213-KRN-0125 (advances).
- `fch::arm` sets DECODEEN's WDT_TMREN when it is clear, and refuses if the bit stays clear.
- It takes the timer at 0xFEB00000, and at the ACPI MMIO block's window only where that one does not answer.
- It puts DECODEEN back, as well as DECODEEN3, on a later refusal.

DRV found that Linux's `sp5100_tco` arms the Ryzen's watchdog where MIND Core refused (`KRN/2026-11-20T0419Z-DRV-…`). The fix is in `fast-test` at `856adee79b` for DRV's boot.

**Tested:**
- `tests/fch_watchdog_host.rs`: 9 tests pass here, with four new cases.
- The kernel builds.
- QEMU has no AMD FCH, so no suite sees the change; the gate runs them all.
- Nothing about the real timer is claimed until the Ryzen's boot line.
