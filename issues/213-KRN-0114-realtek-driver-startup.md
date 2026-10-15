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
  interrupt and net endpoint; give Wi-Fi its BAR, retained 128 KiB DMA and
  firmware-reader client for one bounded firmware diagnostic, without
  advertising a working radio to WLAN.
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

## Firmware diagnostic continuation (2026-10-16)

The separate integration branch starts from `0c4205e7` and merges the driver
owner's `27033a43` and `cf768d22`. The existing ABI, service identity and
MEM-only BAR grant remain unchanged. `init` adds a retained 128 KiB DMA grant
in `SLOT_MEM` and keeps the decimal device index and read-only VFS client.
No RTL8852BE WLAN endpoint is minted.

The native service first loads exactly the pinned private firmware file and
checks its digest before mapping/accessing the board. After the board-read
cycle has powered MAC off, `Verified::verify` admits the actual cut/RFE/OTP
and image metadata. Only then can it map/initialize the exclusive DMA queue.
The native adapter checks DMA capability kind, virtual/physical 4 KiB
alignment, full allocation bounds below 4 GiB and the 128 KiB minimum.
Kernel DMA maps are WB and BAR maps UC on x86; no CPU alias accesses the
queue after construction. The DMA mapping is retained with `ManuallyDrop`
on every success/error path and the kernel's original region stays in init.

Production orchestration then powers MAC, prepares download memory and
PCIe, installs the command ring, finishes BDRAM setup and requests kernel
mastering. Only PCIe I/O and channel 12 are unstopped, TXHCI is enabled and
RXHCI/ordinary packet channels stay stopped. After the single bounded
download attempt, independent DMA/WCPU stop runs even on failure. Kernel
`registers_only` and explicit Command readback must confirm MEM-only before
the fresh-budget MAC power-off attempt. A failed confirmation permits no
further MMIO. A final kernel stop follows power-off; every cleanup outcome
is retained separately. No path retries or releases DMA.

The focused startup fixture checks the production stage ordering and a
failure at every stage, independent cleanup, unavailable mastering, missing
ordinary-channel stops, activation readback failure and DMA bounds. Own-branch
`rtw89_wifi` and `init` x86-64 release builds passed (final incremental native
build 0.22 s, init build 3.64 s); ten startup/activation cases compiled with
`rustc --test` but have not run. Execution follows raw `fast-test` publication
on the own branch. Actual board reads, DMA/WCPU startup and cleanup remain
untested on Ryzen. The driver owner's independent review found no blocking
integration/ownership defect; it does not establish hardware operation.

## Related

[213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md),
[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md),
[550-KRN-0097](550-KRN-0097-the-wifi-station-in-the-boot-services.md),
[213-KRN-0113](213-KRN-0113-realtek-boot-identification.md),
[hardware audit](../issues-audit/2026-10-15-hardware-audit.md).
