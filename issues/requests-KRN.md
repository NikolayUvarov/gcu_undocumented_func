# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (2 requests waiting, 2026-10-09) · **Recorded by:** the tools track (APP), 2026-10-06

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

## The `devicetree` suite misses the kernel's line when CI is slow

**Recorded by:** the storage session, 2026-10-09: its branch's CI run for 29741e7 failed in "aarch64 (programs, shell and four CPUs)" on this suite alone; the next commit, with the same code, passed. The suite is [210-KRN-0029](../issues-done/210-KRN-0029-device-tree-in-bootinfo.done)'s.

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
