# 351-DRV-0015 — A TPM 2.0 driver

**Type:** driver · **Owner:** `DRV` track (open) · **Priority:** P2 · **Status:** in progress · **Blocked by:** [requests-KRN.md](requests-KRN.md) (the kernel hands out the TPM's registers, `PLATFORM_TPM`) · **Main task:** [351](351-self-update.md) · **Roadmap:** track A · **Constitution:** MC-11.9, MC-1.5

Recorded by the storage session at the maintainer's request (2026-10-09), for [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md) and [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md). Numbered from 0015 so as not to meet numbers the kernel session gives drivers.

## Problem

No TPM is driven: nothing can seal a key or keep a version counter in hardware.

## Plan

- **A ring-3 service `tpm`** over the TPM's CRB (or TIS) registers, which `init` gives it from the ACPI `TPM2` table:
  - x86: the CRB at the address the table names;
  - aarch64 `virt`: `tpm-tis-device`.

  It sends TPM 2.0 commands and returns responses. Nothing else holds the registers.
- **An interface (`idl/tpm.wit`)** with the operations its clients need, not raw commands:
  - seal and unseal for `keystore` (by badge);
  - an NV counter for the updater's version floor.
- **The PCR policy** is left to the measured-boot step.
- **Tests:** QEMU with `swtpm`, installed in CI; without a TPM the service reports none.

## Acceptance criteria

In QEMU with `swtpm`, `keystore` seals and unseals a secret through the service, and a client without the badge is refused.

## Related

[351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md), [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md).

## Progress (2026-10-09)

- **Done** (the storage session, the drivers track being open):
  - `libmind/src/tpm.rs`: the TPM 2.0 commands sealing takes, marshalled by hand. Startup, GetCapability for the manufacturer, CreatePrimary of an ECC P-256 storage key (the same on every boot), Create of a sealed data object, Load, Unseal, FlushContext; a password session with the empty password. Host tests `tests/tpm_host.rs`: commands byte for byte, responses, bounds.
  - `idl/tpm.wit` 1.0: `info` for any client, `seal` and `unseal` for `mind::tpm::BADGE_SEAL`. No raw commands.
  - `tpm/`, the service: the CRB or the FIFO (by `TPM_INTERFACE_ID`), locality 0, one command at a time. A fresh primary key per request, flushed after it, so a restarted service leaves nothing loaded. Every seal, unseal and refusal is logged with the client's PID.
  - `init` starts it with the registers in slot 2 when the kernel gives them, and gives `keystore` the seal badge ([351-KRN-0043](../issues-done/351-KRN-0043-tpm-service-at-boot.done)). The shell's `tpm` command shows it ([351-APP-0018](../issues-done/351-APP-0018-shell-tpm-command.done)).
  - CI and `scripts/ci_local.sh` install `swtpm`.
- **Waiting:** the kernel's half, the registers' place from the firmware's tables ([requests-KRN.md](requests-KRN.md)). Until then `tpm` logs `NO TPM` everywhere. On the storage session's machine, with a sketch of that half, the service sealed and unsealed through `swtpm` behind QEMU's `tpm-crb` (x86, the CRB) and `tpm-tis-device` (aarch64, the FIFO), and refused the shell. The `tls` suite's `tpm_check` does the same once the half lands; until then it checks only the path without a TPM.
- **Left:** the NV counter for the updater's version floor goes with [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md).

