# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (4 requests waiting, 2026-10-09) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## The TPM's registers from the firmware's tables (`PLATFORM_TPM`)

**Recorded by:** the network and drivers tracks (the storage session), 2026-10-09, for [351-DRV-0015](351-DRV-0015-tpm-driver.md) and [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md).

### Problem

The TPM service `tpm` drives a TPM 2.0 and `keystore` seals the device key with it. Both are built and host-tested, and so is `init`'s part ([351-KRN-0043](../issues-done/351-KRN-0043-tpm-service-at-boot.done)): it starts `tpm` with `PLATFORM_MMIO, PLATFORM_TPM` (`common/abi.rs`, 0x30) in its slot 2.

But `platform::mmio(PLATFORM_TPM)` answers `NOT_FOUND` on both architectures, because the kernel does not read the TPM's place from the firmware. So `tpm` reports no TPM, and the key stays unencrypted (the interim).

### Plan (a proposal; the kernel and porting tracks decide)

- **x86** (`kernel/src/arch/x86_64/acpi.rs`, `platform.rs`): read the ACPI `TPM2` table: the control area's address at 40, the start method at 48.
  - A CRB (7) has locality 0's registers in the control area's page. QEMU's `tpm-crb` puts the area at 0xFED40040.
  - A FIFO (6) is at the PC Client profile's 0xFED40000.
  - Hand out 4 KiB. Other start methods are not driven.
- **aarch64** (`acpi.rs`, `aml.rs`, `board.rs`, `platform.rs`): QEMU's `tpm-tis-device` is a DSDT device `MSFT0101` whose static `_CRS` holds a `Memory32Fixed` window (0x5000). The pin controllers' scan in `aml.rs` finds it once it takes the ID; hand out the window's first 4 KiB. A `TPM2` table with a CRB (7) as on x86.
- **Tests:**
  - `tests/aml_host.rs`: the `MSFT0101` device beside a PL061.
  - `tpm_check` in the `tls` suite (both architectures, `swtpm`) checks the sealing once this lands. Until then it skips that part and says why; remove the skip with this task.

A sketch of all of it, about 70 lines, was run by the storage session on its machine: the whole chain (seal, a reboot, unseal, another TPM refused) passed on x86 with `tpm-crb` and on aarch64 with `tpm-tis-device`. It is not committed, because `kernel/src/arch/` is the porting track's. The storage session can send it on request.

### Acceptance criteria

- With `-device tpm-crb` (x86) or `-device tpm-tis-device` (aarch64) and `swtpm`, `init` hands `tpm` the registers and `tpm` logs `READY`.
- Without a TPM, `PLATFORM_TPM` answers `NOT_FOUND` as now.

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
