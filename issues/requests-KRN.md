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


## Owners for the findings of the 2026-10-09 audit

**Recorded by:** the maintainer's assessing session, 2026-10-09. The maintainer decided that the kernel track assigns the owners.

### Problem

The audit of `2cbda21` ([issues-audit/2026-10-09-repository-audit.md](../issues-audit/2026-10-09-repository-audit.md)) has eight findings, all confirmed against the code by [the assessment](../issues-audit/2026-10-09-repository-assessment.md). No issue covers them. Three are in `vfs_server`, which has no owner in [TRACKS.md](../TRACKS.md).

### Plan (a proposal; the kernel track decides)

| Finding | Priority (assessment) | Code | Proposed owner |
|---|---|---|---|
| A01: the packager lists programs before it builds | P1 | `scripts/make_usb_image.py` | `PRT` |
| A02: a failed FAT write loses the free space | P1 | `vfs_server/src/fat.rs`, `main.rs` | the owner of `vfs_server`, to be named (`STO` is nearest) |
| A03: a failed case-only rename deletes the file | P2 | `vfs_server/src/fat.rs` | the same |
| A04: the volume reads clean after unflushed or failed writes | P2; the profile row on the dirty state now | `vfs_server/src/fat.rs`, `disk.rs`, `docs/profile/evidence.md` | the same |
| A05: the block store acknowledges a block it erased | P2; P1 before a durable medium | `blockstore/src/store.rs` | `STO` |
| A06: CI passes a host test that did not compile; local fixtures can be stale | P1 | `.github/workflows/ci.yml`, `scripts/ci_local.sh` | `KRN`, which keeps CI, with `ASR` |
| A07: the editors overwrite an existing `<name>.tmp` | P2 | `edit`, `fm` | `APP` |
| A08: `fm` removes a move's source before the destination is flushed | P1 | `fm` and its `Disk::flush` | `APP` |

- Name an owner for `vfs_server` in TRACKS.md.
- Each owner numbers a task for its findings, and the number is recorded against the finding in the assessment.
- Each fix turns its audit probe's assertion around into a regression test (`issues-audit/repro/`).

### Acceptance criteria

Every finding has an owner and an issue number, recorded in the assessment, and TRACKS.md names an owner for `vfs_server`.
