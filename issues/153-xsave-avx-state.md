# 153 — XSAVE: AVX state per task

**Type:** kernel · **Owner:** kernel track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track G (voice V3) · **Constitution:** MC-2.1

## Problem

Task contexts save SSE state with FXSAVE; AVX/AVX2 registers are not saved, so programs must not use them. Speech models (voice V3, [docs/voice](../docs/voice/README.md)) run several times faster with AVX2.

## Plan

- Enable `CR4.OSXSAVE` and XCR0 for x87/SSE/AVX when CPUID reports them; save and restore with `XSAVE`/`XRSTOR` (`XSAVEOPT` when present) in a context area sized from CPUID leaf 0xD; keep FXSAVE on CPUs without XSAVE.
- `BootInfo`/`STAT_CPUS` report the enabled feature set; `hw` shows it.
- `smp` suite: two tasks on one CPU keep distinct YMM values across switches.

## Acceptance criteria

- The `smp` and `busy` suites pass with AVX tasks on 1 and 4 CPUs (QEMU `-cpu max`); CPUs without AVX keep working.

## Related

[docs/voice](../docs/voice/README.md) (V3).
