# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (4 requests waiting, 2026-10-09; the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md)) · **Recorded by:** the tools track (APP), 2026-10-06

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

The tools track then builds the voice engines for `aarch64-unknown-none` (hard float, a tier-2 target with its own core). Until then they build in soft float there: correct, and their checks pass in the aarch64 tools suite, but far too slow for a real model.

### Acceptance criteria

On aarch64 in QEMU, two tasks on one CPU keep their V registers across switches, and `dictate --features` gives kaldi-native-fbank's features.

## The devicetree check races init on CI runners (210-KRN-0029)

**Recorded by:** the tools track (APP), 2026-10-09.

### Problem

The `devicetree` suite catches `MIND CORE KERNEL: DEVICE TREE AT …` on the screen. The machine runs in 10 ms steps until the line shows, but the kernel writes it only to its early console (`PanicSerial`), and init's services soon write over it.

- On GitHub's runners the line was gone between two steps in 2 of 5 runs of the tools branch: runs 37891849301 and 37892537807, job "aarch64 (programs, shell and four CPUs)", `kernel` None after 500 steps.
- Here it passed 3 of 3 times.

### Plan (a proposal; the kernel track decides)

The kernel also keeps the line where a test can read it after boot (its log ring, which `dmesg` shows), and the check reads it there.

### Acceptance criteria

The check no longer depends on how much guest work a 10 ms step covers.

## A slot and a request flag for the system clipboard (000-APP-0032)

**Recorded by:** the tools track (APP), 2026-10-09, for [000-APP-0032](000-APP-0032-system-clipboard.md) (the tools plan's phase T4: the system clipboard).

### Problem

`edit` (Ctrl+C/X/V), `fm`'s command line and the shell's line each keep their own text, so nothing can be copied from one program to another. The tools track writes the clipboard: `idl/clipboard.wit` and a `clipboard` service that holds the text. It needs a way to reach the programs that ask for it.

### Plan (a proposal; the kernel track decides)

The same way as the parser service (109-KRN-0042):

- `init` starts `clipboard` and gives the shell a client.
- `SLOT_CLIPBOARD` and `REQUEST_CLIPBOARD` are added in `common/abi.rs` and `libmind::process`.
- A launcher may fill the slot in a launch session.

The shell lends its client only to a program that asks for it, and `msh` gets a `clipboard` word. What the service answers, and to whom, is in 000-APP-0032.

### Acceptance criteria

A program that asks for `REQUEST_CLIPBOARD` holds an endpoint of `clipboard` in `SLOT_CLIPBOARD`; one that does not ask holds nothing there.

## A fixed slot for the shell's command endpoint (211-APP-0040)

**Recorded by:** the tools track (APP), 2026-10-09, for [211-APP-0040](211-APP-0040-the-shells-commands-in-console.md) (the shell's commands in `wm`'s `console`, the kernel track's request for 211).

### Problem

The shell will serve `idl/shell.wit`: a client sends a command line and the shell runs it on its own authority. The endpoint goes from the shell to `wm` and from `wm` to `console` in a launch session, so both need a fixed slot to find it in. The application slots 1–29 are all named, and fixed slots end at `SLOT_DYNAMIC` (30), below which the kernel delivers capabilities into a receive slot.

### Plan (a proposal; the kernel track decides)

- `SLOT_SHELL` in `common/abi.rs` for applications, with `SLOT_DYNAMIC` moved up (the clipboard's `SLOT_CLIPBOARD`, asked for above, can come in the same change).
- Nothing in `init`: the shell makes the endpoint and lends it itself. `REQUEST_SHELL` goes into `libmind::process` with the tools track's change.

### Acceptance criteria

A launcher fills `SLOT_SHELL` in a launch session and the program holds the endpoint there; the ABI version and the kernel's tests follow the move of `SLOT_DYNAMIC`.
