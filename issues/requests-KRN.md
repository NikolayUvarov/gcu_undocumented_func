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
