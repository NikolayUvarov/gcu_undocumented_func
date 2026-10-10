# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (5 requests waiting, 2026-10-10; the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md), FP/SIMD for programs on aarch64 [250-KRN-0056](../issues-done/250-KRN-0056-fp-simd-for-programs-on-aarch64.done), the devicetree check [210-KRN-0055](../issues-done/210-KRN-0055-the-devicetree-suite-steps-through-qmp.done)) · **Recorded by:** the tools track (APP), 2026-10-06

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
