# 213-KRN-0114 — Start the Ryzen Realtek drivers with DMA disabled until ready

**Type:** kernel / driver integration · **Owner:** ASTRA auditor-fixer (maintainer's network assignment, 2026-10-15) · **Priority:** P1 · **Status:** in progress · **Blocked by:** 213-DRV-0031/0032 native binaries; own-branch build and tests; hardware · **Roadmap:** H / 213, D / 550 · **Constitution:** MC-1.5, MC-3.3, MC-6.3, MC-12.1, MC-12.3, MC-12.4

## Problem

Neither Realtek driver can run from a boot image today. Ordinary PCI BAR
grants enable bus mastering before a new driver has reset its engine or
installed DMA rings. Restart also enables DMA before the new driver starts.
The Wi-Fi board-access stage must have memory decoding without bus mastering;
the Ethernet driver must enable mastering only after it prepares its rings.
The maintainer requests a complete hardware staging path for the original
MacBook and Ryzen, with Wi-Fi first and Ethernet in parallel.

## Plan

- Merge the kernel owner's original ABI 10/WLAN startup branch into ASTRA,
  preserving MacBook BCM4331 and QEMU radio behavior; never import fast-test.
- Add the two native boot images as ABI 11 and rebuild all boot components.
  Freeze the ABI 10 prefix and append extra images, checking version before pointer use. Update the boot image together; test actual legacy layout rejection.
- Add a platform-only MMIO BAR grant that enables memory decoding with I/O
  decoding and bus mastering off, checks readback, and keeps Status W1C bits.
- Restrict init selection to the exact Realtek IDs and classes. Pass the
  selected device index as a decimal argument so a BAR-owning driver can use
  the existing checked DEVICE_STATE API. Give Ethernet its retained DMA,
  interrupt and net endpoint; give the initial Wi-Fi stage only its BAR and
  firmware-reader client, without advertising a working radio to WLAN.
- On restart restore only memory decoding through DEVICE_REGISTERS and an owned BAR, after Platform is dropped. MSI preserves Command and MSI-X requires existing MEM; neither starts DMA. Verify the old process is dead and device
  stop succeeds before clearing DMA; a failed stop quarantines the driver and retains
  its DMA contents. Preserve the existing VirtIO and MacBook routes.

## Acceptance criteria

- Production PCI-control tests reject inaccessible/refusing devices and
  establish the requested Command bits without writing Status flags.
- Both native binaries build in the complete x86 image; absence of the exact
  device remains an explicit no-device result, not a boot failure.
- Runtime/IDL/host tests and required x86 QEMU groups pass on ASTRA after the
  commit is published in fast-test. ARM is deferred by the maintainer and
  remains an explicit incomplete full-gate requirement.
- The original MacBook still starts its radio/station, and Ryzen's next boot
  records driver identity, grant/start state and traffic outcomes separately.
  A Wi-Fi board probe is not association or a usable network interface.

## Related

[213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md),
[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md),
[550-KRN-0097](550-KRN-0097-the-wifi-station-in-the-boot-services.md),
[213-KRN-0113](213-KRN-0113-realtek-boot-identification.md),
[hardware audit](../issues-audit/2026-10-15-hardware-audit.md).
