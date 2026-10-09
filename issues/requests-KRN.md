# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (10 requests waiting, 2026-10-09; the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md)) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## The IDL fuzzer in CI's host tests (500-ASR-0001)

**Recorded by:** the assurance track (`ASR`), 2026-10-09, for [500-ASR-0001](500-ASR-0001-idl-decoder-fuzzing.md).

### Problem

`tests/idl_fuzz_host.rs` fuzzes every generated IDL decoder with a fixed seed: 24 receivers and 82 types, 50 000 inputs per target, about 4 s. It is not in CI's host tests yet, and the CI files are the kernel track's.

### Plan (a proposal; the kernel track decides)

- Add `idl_fuzz` to the list of host tests in `.github/workflows/ci.yml` (the step "Host tests") and in `scripts/ci_local.sh` (`host_tests`), built like the others: `rustc --edition=2021 --test tests/idl_fuzz_host.rs`.
- If 175-KRN-0046 changes how the host tests fail, the line follows that.

### Acceptance criteria

CI runs the test on every push, and a finding fails the host-test step.

## The updater's VFS client badged `BADGE_UPDATE`

**Recorded by:** the storage session for the update track, 2026-10-09, for [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md).

### Problem

`vfs_server` now has the update zone (docs/update/slots.md): a client badged `mind::fs::BADGE_UPDATE` (4) may fill the slot that did not boot and write the boot records whole, in place, and nothing else. 351-KRN-0022 (on the kernel branch) lends `updater` an unbadged, read-only VFS client, so no client holds the badge.

### Plan (a proposal)

- In `init`'s `"updater"` arm, give slot `SLOT_VFS` a client of `vfs_server` badged `BADGE_UPDATE` in place of the lent one, as `keystore` and `netpolicy` get theirs: `grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_UPDATE)?, CLIENT)`.
- The `updater` suite's stand-in (`tests/updater_stub`), booted from a slot volume (`scripts/boot_slots.py layout … --both`):
  - writes a file in the inactive slot and a whole record;
  - is refused in the running slot, in `EFI/`, a record of another size and a new file in `MIND/`.

  The update track can write those cases once the grant is in `main`.

### Acceptance criteria

Only `updater` holds a client with `BADGE_UPDATE`; it reads as before and may write only the update zone.

## A memory quota for `blockstore` that fits its disk's index

**Recorded by:** the storage session, 2026-10-09, for [251-STO-0013](251-STO-0013-an-index-that-grows-with-the-medium.md) (the speech models of 251-STO-0010).

### Problem

The block store's index now grows with its medium: 56 bytes a slot, room for a block per 8 sectors (`slots_for` in `blockstore/src/store.rs`). `blockstore` allocates the slots at mount and halves them until its memory quota allows.

Its quota is the default 16 MiB (`HEAP_MAX_BYTES`), so the index stays under about 14 MiB, roughly 230 000 blocks of 16 KiB: 3.5 GiB of objects. A model disk of several such models, or a store disk larger than that, would mount with an index too small to hold every block, and mounting refuses such a store whole.

### Plan (a proposal; the kernel track decides)

- In `init`'s quotas, `"blockstore" => Quota { memory_mib: BLOCKSTORE_MEMORY_MIB, ..Quota::default() }`, next to `windows` and `compositor`, with 64 MiB. That is an index of 2^20 slots (56 MiB) and the rest of what it holds now.
- Or a quota computed from the store disk's size, if `init` knows it when it starts `blockstore`.

### Acceptance criteria

On a store disk of 8 GiB, `[BLOCKSTORE] INDEX:` reports the slots `slots_for` asks for, not a halved number.

## A kernel panic in `awaits_reply` after the task table shrank

**Recorded by:** the storage session, 2026-10-09, from its local gate (`scripts/ci_local.sh --ref claude/relaxed-meitner-5bmhpz`, the branch at 10375c2 merged with `main` at a9ac93f; the branch changes no kernel file).

### Problem

The group "aarch64: GICv2 with GICv2m" failed in the `normal` suite's `applications_until_memory_ends`, right after `kill 165`:

```
KERNEL PANIC: index out of bounds: the len is 1 but the index is 2 at src/scheduler.rs:438:19 CPU=2 PID=3 NAME=rtc
```

Line 438 is `awaits_reply`: `self.tasks[client]`. `Table::index` (`&self.chunks[index / CHUNK][index % CHUNK]`) panics for a slot in a chunk that `shrink` dropped. So when `rtc` replied to a client whose task had ended, the table had shrunk under that client's slot: the second chunk emptied as the suite's applications were killed. The other groups that run the same suite passed, so it depends on timing. GICv2 changes how interrupts reach CPU 2.

### Plan (a proposal; the kernel track decides)

- `Table::get(index) -> Option<&Option<T>>`, `None` past the end, used wherever a slot is held across a point where the table may shrink: `awaits_reply`, `fail_reply`, the reply paths, timeouts. Or `Index` gives a static `None` past the end, as an empty slot reads.
- A host or QEMU case: a client killed while it waits for a reply, in the table's second chunk, with the chunk dropped before the server replies.

### Acceptance criteria

A reply to a client whose slot's chunk was dropped fails with `ERR_PEER` to the server and does not panic.

## The `devicetree` suite misses the kernel's line when CI is slow

**Recorded by:** the storage session, 2026-10-09: its branch's CI run for 29741e7 failed in "aarch64 (programs, shell and four CPUs)" on this suite alone; the next commit, with the same code, passed. The tools branch saw it in 2 of 5 runs (37891849301, 37892537807) and passed it 3 of 3 times on its machine. The suite is [210-KRN-0029](../issues-done/210-KRN-0029-device-tree-in-bootinfo.done)'s.

### Problem

`devicetree_suite` (`tests/qemu_smoke.py`) stops the machine once the bootloader prints `BOOT: DEVICE TREE AT …`. It then runs it on in steps of `cont`, 10 ms, `stop`, and reads the screen for the kernel's `MIND CORE KERNEL: DEVICE TREE AT …`. The failure was `AssertionError: (<re.Match … 'BOOT: DEVICE TREE AT 0x47ef6000, 1052672 BYTES'>, None)` after all 500 steps.

Measured on the storage session's machine (aarch64, the branch's build):
- `vm.hmp()` waits 10 ms per byte of the command before sending it, so each step lets the machine run about 110 ms, not 10 ms.
- The stop after the bootloader's line comes as late: by the first look, the kernel's line is already on the screen.
- The line stays visible for about four such steps (about 450 ms), then init's services scroll it off. A slow runner that lets the machine run longer before a stop misses it, and every later step looks in vain.

### Proposed fix

Stop and continue through QMP itself (`vm.qmp("stop")`, `vm.qmp("cont")`) in that suite, for the first stop and in the loop. Each step then lets the machine run 11–20 ms, and the line stayed at the top for more than 20 steps in the same measurement. The suite asserts the same thing.

### Acceptance criteria

The suite passes on aarch64 as before; its steps no longer go through the monitor's typing pace.

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

## The pinned toolchain installed once before the parallel build (000-KRN-0020)

**Recorded by:** the assurance and drivers session (`ASR`, `DRV`), 2026-10-09, after the maintainer's build of `fast-test` failed.

### Problem

`rust-toolchain.toml` gained `components = ["rust-src"]` (e5a34c2, 250-APP-0020). On a machine with `nightly-2026-10-02` but without that component, `02_build.sh` starts every crate's cargo at once (000-KRN-0020). Each cargo asks rustup to add `rust-src`, and the downloads race on one file:

```
error: component download failed for rust-src: could not rename 'downloaded' file from
'~/.rustup/downloads/7da4d…partial' to '…': No such file or directory (os error 2)
```

All 70 crates fail, and the build reports them as failures of the code. A single `rustup component add rust-src --toolchain nightly-2026-10-02` fixed it.

### Plan (a proposal; the kernel track decides)

- Before the parallel step, `02_build.sh` runs `rustup toolchain install` once in the repository. It reads `rust-toolchain.toml` and installs the channel, the components and the targets. A failure stops the build with that message.
- `01_prepare_env.sh` does the same, so the two cannot disagree.

### Acceptance criteria

On a machine whose rustup lacks a component the toolchain file names, `02_build.sh` installs it once and the build succeeds. A failed install is reported as such, not as 70 failed crates.

## A fixed slot for the shell's command endpoint (211-APP-0040)

**Recorded by:** the tools track (APP), 2026-10-09, for [211-APP-0040](211-APP-0040-the-shells-commands-in-console.md) (the shell's commands in `wm`'s `console`, the kernel track's request for 211).

### Problem

The shell will serve `idl/shell.wit`: a client sends a command line and the shell runs it on its own authority. The endpoint goes from the shell to `wm` and from `wm` to `console` in a launch session, so both need a fixed slot to find it in. The application slots 1–29 are all named, and fixed slots end at `SLOT_DYNAMIC` (30), below which the kernel delivers capabilities into a receive slot.

### Plan (a proposal; the kernel track decides)

- `SLOT_SHELL` in `common/abi.rs` for applications, with `SLOT_DYNAMIC` moved up (the clipboard's `SLOT_CLIPBOARD`, asked for above, can come in the same change).
- Nothing in `init`: the shell makes the endpoint and lends it itself. `REQUEST_SHELL` goes into `libmind::process` with the tools track's change.

### Acceptance criteria

A launcher fills `SLOT_SHELL` in a launch session and the program holds the endpoint there; the ABI version and the kernel's tests follow the move of `SLOT_DYNAMIC`.

## `bcm_wifi` as a boot service (550-DRV-0020)

**Recorded by:** the drivers track (`DRV`), 2026-10-09, for [550-DRV-0020](550-DRV-0020-bcm4331-read-only-probe.md), stage 1 of the MacBook Pro's Wi-Fi ([550-DRV-0006](550-DRV-0006-broadcom-wifi.md)). The maintainer put Wi-Fi first among the network tasks.

### Problem

`bcm_wifi/` is the driver for the MacBook Pro's Broadcom BCM4331 (`14E4:4331`, class `028000`). Its stage 1 is written and builds, and only reads the chip. Three things in the kernel track's files keep it from running:

- it is not in `BOOT_SERVICES`/`BOOT_FILES`;
- `init` does not start it;
- `02_build.sh` does not build it.

`BOOT_IMAGES` is 32 and sizes `BootInfo.programs`, so one more boot image is an ABI change.

### Plan (a proposal; the kernel track decides)

- **`common/abi.rs`:** `bcm_wifi` and `bcm_wifi.elf` in the boot lists, `BOOT_IMAGES` one larger, with the ABI version and its transition as the track does them.
- **`02_build.sh`:** `"bcm_wifi:bcm_wifi:bcm_wifi.elf"` in the crate list (x86 only; there is no such chip on the aarch64 targets).
- **`init`:**
  - start `bcm_wifi` when `DEVICE_FIND` finds vendor `14E4` device `4331`, or class `02:80:00` from vendor `14E4`; otherwise `bcm_wifi NOT STARTED: NO DEVICE`, as for the other drivers;
  - grant BAR0 (16 KiB MMIO) in `SLOT_DEV0`. Stage 1 needs nothing more.
- **Later stages, for the same grant list when they come** (requests then):
  - the MSI (or INTx) line;
  - a DMA region for the transmit and receive rings (about 256 KiB);
  - a read-only `vfs` client for the microcode file (`firmware/` on the boot volume, put there by the maintainer's build);
  - a service endpoint for `NET`'s station.
- **A configuration write limited to the BCMA window registers** (0x80, 0xAC, 0x84) of the driver's own function may be asked for later, if moving BAR0's windows turns out to be needed. Stage 1 does not move them.

### Acceptance criteria

On the MacBook Pro, `init` starts `bcm_wifi` with BAR0, and its stage-1 lines are in the boot log. On QEMU, which has no such chip, it is not started and says so.
