# 174-APP-0058 — `cpus` names AVX-512 and AMX

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (written; a machine with AVX-512 left) · **Blocked by:** — · **Main task:** [174](174-full-use-of-pc-hardware.md) · **Roadmap:** track H · **Constitution:** MC-10.2

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it for [174-KRN-0037](174-KRN-0037-every-vector-state-component.md).

## Problem

The kernel saves AVX-512's and AMX's state components where the processor has them. `STAT_CPUS.xsave` carries them (XCR0: `0xE0` AVX-512, `0x60000` AMX). `shell/src/observe.rs` printed `FPU=XSAVE+AVX` for any of them, so a user could not tell from `cpus` that the wider units are usable.

## Plan

`cpus` prints `FPU=XSAVE+AVX`, then `+AVX512` and `+AMX` for each group whose bits are all set. The `XSAVE+AVX` prefix stays, because the `busy` and `smp` suites match it.

## Acceptance criteria

- The `smp` suite with `--cpu-model max` still sees `FPU=XSAVE+AVX` on every CPU.
- A machine with AVX-512 shows `+AVX512` (the profile records it).

## Progress

- `cpus` adds `+AVX512` when XCR0's bits `0xE0` are all set, and `+AMX` for `0x60000`, after `XSAVE+AVX`.
- The `smp` suite with `--cpu-model max` is the check here: QEMU's TCG has no AVX-512, so the line stays `XSAVE+AVX`.
- `+AVX512` waits for a run on a machine that has it.

## Related

[174-KRN-0037](174-KRN-0037-every-vector-state-component.md), [requests-APP.md](requests-APP.md) (where it was recorded).
