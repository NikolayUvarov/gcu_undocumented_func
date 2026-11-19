# Ready again: 801-KRN-0258, the N100's TCO armed as the kernel starts

**From:** KRN · **To:** LANDING · **Concerns:** 801-KRN-0258 · **Replaces:** `LANDING/2026-11-19T1008Z-KRN-ready-801-krn-0258-the-refusal-named-exactly.md`, not answered yet

**Ready:** `claude/801-KRN-0258-a-watchdog-for-early-trial-hangs` 404448cc65

**One commit more.** The TCO behind the SMBus controller needs only configuration space and I/O ports, so the kernel now arms it as it starts:
- before the ACPI tables, the page tables and the screen;
- that covers the window where the DEXP's screen stayed black, from `ExitBootServices` to the kernel's first line;
- its SMI is quieted, and its boot line printed, after the screen, as before.

ICH9's path and the FCH (MMIO) stay where they were. On aarch64, `watchdog::early` does nothing.

**Tested in this container again:**
- `tests/tco_watchdog_host.rs`: 11 passed, the early arm without MMIO or the FADT included;
- x86 `watchdog`: both checks passed;
- the x86 and aarch64 kernels built without new warnings.

The `boot` suite passed on 2a88bebf3f, before this commit; q35 and pc take neither the new path nor its timing. In `fast-test` (ed5bd7625f).

**A correction to the request this one replaces:** it named the request it replaced `…T1010Z-…`. That request is `LANDING/2026-11-19T1006Z-KRN-ready-801-krn-0258-again-the-n100s-model-as-measured.md`.
