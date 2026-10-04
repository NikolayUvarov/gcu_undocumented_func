# 204 — aarch64 profile and CI

**Type:** documents + CI · **Owner:** porting track · **Priority:** P2 · **Status:** open · **Blocked by:** 202, 203 · **Roadmap:** track H · **Constitution:** MC-12.1, Appendix A (profiles)

## Plan

- **Build:** `02_build.sh` builds for both architectures (`ARCH=aarch64`); a launcher `03_run_qemu_aarch64.sh`.
- **Profile:** `docs/profile/aarch64/` for `aarch64/QEMU-virt-0`:
  - platform, TCB (GIC, ITS, firmware, PSCI), threat model, evidence;
  - what differs from x86, for example no IOMMU (SMMU later) and no legacy devices.
- **CI:** an aarch64 build job and QEMU jobs running the suites from 201–203.
- **Then:** boards with UEFI (Raspberry Pi 4/5 with the EDK2 port, servers with ACPI) as further profiles.

## Acceptance criteria

The CI matrix is green for both architectures, and the README says how to build and run either.

## Related

[201](201-aarch64-boot.md), [202](202-aarch64-devices.md), [203](203-aarch64-smp-and-power.md).
