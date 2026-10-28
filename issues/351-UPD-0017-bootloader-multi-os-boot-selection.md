# 351-UPD-0017 — Bootloader multi-OS boot selection and OS switching for updates

**Type:** investigation (bootloader and update mechanism) · **Owner:** `UPD` track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.1, MC-9.4

## Problem

During firmware updates and provisioning, it may be necessary to boot from different operating systems (e.g., switching from mind-core-os to Ubuntu for update operations, then back to mind-core-os). The question is whether our bootloader can support programmatic switching between multiple bootable operating systems on the same machine, and whether this is feasible given the current boot architecture and update mechanism.

This capability would enable:
- **Transparent firmware updates:** switching to a utility OS, performing low-level updates, then returning to the main OS
- **Recovery and diagnostics:** booting alternative environments for system recovery or hardware diagnostics
- **Provisioning workflows:** use of standardized tools for initial device provisioning

## Plan

1. **Audit the current bootloader implementation:**
   - Review bootloader code in `bootloader/` and `loader` directories
   - Understand current boot selection mechanism and slot management
   - Identify how boot parameters, device trees, and kernel arguments are passed
   - Document current limitations and capabilities for multi-OS scenarios

2. **Define OS switching requirements:**
   - What operating systems need to be supported (mind-core-os, Ubuntu, other recovery/diagnostic OS)
   - How to identify and select boot targets programmatically
   - How to preserve each OS's configuration and data partitions
   - Recovery mechanisms if switching fails

3. **Evaluate technical feasibility:**
   - Can the bootloader discover multiple OS installations?
   - Can OS selection be done programmatically from user space (e.g., via a system service)?
   - How would this interact with the current update mechanism (slots, boot records)?
   - What changes to the ABI or boot protocol would be needed?
   - Impact on the update and recovery workflows

4. **Document findings:**
   - Feasibility assessment with trade-offs
   - Recommended approach (if feasible)
   - Required changes to bootloader, kernel, and update mechanism
   - Dependencies on other subsystems (storage, update, recovery)

## Acceptance criteria

- Written investigation report with findings on:
  - Current bootloader multi-boot capabilities
  - Feasibility of programmatic OS switching
  - Required changes (if any) to existing boot and update architecture
  - Identified risks and mitigation strategies
- Code audit of relevant bootloader sections
- Documented interface requirements (if implementation is feasible)
- Assessment of interaction with [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (rollback policy) and [350](350-signed-boot-images.md) (signed boot images)

## Related

[351](351-self-update.md), [350](350-signed-boot-images.md), [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md).
