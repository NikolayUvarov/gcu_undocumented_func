# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (one request waiting, 2026-10-09) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## FP/SIMD for programs on aarch64 (250)

**Recorded by:** the tools track (APP), 2026-10-09, for main task [250](250-voice-dictation.md) (and [252](252-neural-speech-synthesis.md)).

### Problem

The dictation and speech-synthesis engines compute in f32 with SIMD. On x86_64 the kernel saves SSE and AVX state per task ([153](../issues-done/153-xsave-avx-state.done)), and `dictate` is built for a hard-float x86 target (`targets/x86_64-mind-float.json`): its features match kaldi-native-fbank's in the system.

On aarch64 the kernel leaves FP/SIMD disabled at EL0 (`CPACR_EL1` = 0, `kernel/src/arch/aarch64/cpu.rs`), and programs are built for `aarch64-unknown-none-softfloat`. A program that runs one NEON or FP instruction traps, so the engines cannot run there.

### Plan (a proposal; the kernel track decides)

- Enable FP/SIMD at EL0 (`CPACR_EL1.FPEN` = 0b11) on every CPU.
- Save and restore V0–V31, FPCR and FPSR with each task's context: 528 bytes, in the context record as x86 keeps its XSAVE area. Lazily, or always: the kernel track chooses.
- Leave the kernel itself without FP (its own code stays soft-float).
- `STAT_CPUS` or `cpus` reports it, as `FPU=XSAVE+AVX` does on x86.
- Tests: the `smp` and `busy` suites on aarch64 with two tasks on one CPU keeping distinct V registers, as the busy fixture does with `ymm0` on x86.

The tools track then builds the voice engines for `aarch64-unknown-none` (hard float, a tier-2 target with its own core) and runs `dictate`'s check in the aarch64 tools suite. That check is skipped there until then.

### Acceptance criteria

On aarch64 in QEMU, two tasks on one CPU keep their V registers across switches, and `dictate --features` gives kaldi-native-fbank's features.
