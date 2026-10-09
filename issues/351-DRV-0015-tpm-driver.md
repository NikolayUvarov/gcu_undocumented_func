# 351-DRV-0015 — A TPM 2.0 driver

**Type:** driver · **Owner:** `DRV` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track A · **Constitution:** MC-11.9, MC-1.5

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
