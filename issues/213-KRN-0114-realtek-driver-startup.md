# 213-KRN-0114 — Start the Ryzen Realtek drivers with DMA disabled until ready

**Type:** kernel / driver integration · **Owner:** ASTRA auditor-fixer (maintainer's network assignment, 2026-10-15) · **Priority:** P1 · **Status:** in progress · **Blocked by:** remaining integrated x86 acceptance and actual MacBook/Ryzen boot evidence; ARM deferred · **Roadmap:** H / 213, D / 550 · **Constitution:** MC-1.5, MC-3.3, MC-6.3, MC-12.1, MC-12.3, MC-12.4

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
  interrupt and net endpoint; give Wi-Fi its BAR, retained 8 MiB DMA and
  firmware-reader client for one bounded firmware/MAC/RX diagnostic, without
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
build 0.22 s, init build 3.64 s). Implementation `5f1d940de69e7ab03843ddb620698044bcc6afb1`
was published in `fast-test` `649fea37`; all ten startup/activation cases
then passed on the own branch (0.00 s, zero failed/ignored, x86_64 Linux
6.6.87.2 WSL2, rustc 1.101.0-nightly c36f14571). Command:
`rustc --edition=2021 --test tests/rtw89_startup_host.rs -o /tmp/mind-rtw-startup-tests-20261016`
then `/tmp/mind-rtw-startup-tests-20261016`. Nothing was built or tested on
`fast-test`. Actual board reads, DMA/WCPU startup and cleanup remain
untested on Ryzen. The driver owner's independent review found no blocking
integration/ownership defect; it does not establish hardware operation.

### Cumulative MMIO integration fixture

`tests/rtw89_pipeline_device.rs` supplies one register/OTP/XTAL/MDIO/DBI
model throughout the real board-read, second power-on, MAC preparation,
PCIe preparation, command-ring installation and activation helpers. It does
not reseed guards with a hand-written ready state between stages. Two
standard host cases cover GEN1/GEN2 compatibility and refusal of an
unsupported link after the actual preceding helpers.

The separate `tests/rtw89_pipeline_private.rs` binary requires an explicit
local pinned firmware path. It uses the production `Verified::verify`,
download and stop helpers through the same cumulative model, compares all
transferred sections with the verified file, checks status 7 and retained
payloads, and includes a readiness-timeout cleanup case. The board's OTP and
PCI transitions are modeled, not observations from Ryzen. Payload bytes stay
in memory and are never written to a report or committed.

After own-branch compilation and raw publication `8461fd20`, commit
`09d1ceaa` passed both standard host cases (zero failed/ignored, 0.00 s)
and all three explicit private-file scenarios on 2026-10-16. GEN1 and GEN2
each transferred 164 packets, including all 326,472 section bytes, reached
status 7/version 0.29.29.15 and completed modeled stop/power-off. Withheld
H2C readiness reached the expected bounded timeout with no submitted packet
and successful independent cleanup. Model elapsed times are not hardware
timing evidence. The configuration was x86-64 Linux 6.6.87.2 WSL2 with
rustc 1.101.0-nightly c36f14571 and the separately fetched hash-pinned file.
The private binary is an explicit local check, not a skipped or conditional
passing case in the normal host gate. No production guard or API was changed.

## Related

[213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md),
[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md),
[550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done),
[213-KRN-0113](213-KRN-0113-realtek-boot-identification.md),
[hardware audit](../issues-audit/2026-10-15-hardware-audit.md).

### Native MAC/RX diagnostic continuation (2026-10-16)

The allocation is now a retained 8 MiB coherent region below 4 GiB. The
first 128 KiB is reserved for the command queue; the page-aligned remainder
is exclusive to RXQ/RPQ. Each constructor receives only its disjoint region,
while init and the driver retain the whole allocation after every outcome.
RX descriptors are installed after PCIe BDRAM preparation and before
mastering. The successful firmware token then admits runtime MAC preparation
and installed RX-only activation; ordinary packet TX and host IRQs remain
disabled. No WLAN endpoint, PHY calibration or association is advertised.

Cleanup independently attempts receive stop, chip radio disable and command
DMA/WCPU stop, preserving their errors. After verified MEM-only PCI state,
MAC power-off first waits for any outstanding XTAL command under the source
50 ms bound and its fresh overall one-second budget. An uncertain command
is never replaced: a timeout skips power-off writes but still requests final
kernel quiescence. DMA is never freed or reused. Independent review found
and corrected the indirect-command cleanup gap before publication.

The full own-branch x86 image builds (one second incremental), and twelve
updated startup/cleanup fixtures compile. Execution awaits raw publication;
these checks and the existing library models do not establish actual Ryzen
DMA, RF calibration or connectivity. The frozen `ac1854f2` gate does not
contain this continuation.

Own-branch `b0987183` was published in `fast-test` `7e280d0c` before
executing its compiled startup harness. All twelve cases passed on 2026-10-16
(0.00 seconds, zero failed/ignored, x86-64 WSL2, rustc 1.101.0-nightly
c36f14571), including pending-XTAL refusal, disjoint retained DMA bounds,
all thirteen startup-stage failure positions and independent cleanup.
The later own-branch integration `f314d00c` also builds the complete x86
image with the queue-session and typed PHY/RX guards. These checks do not
establish cumulative runtime device behavior or physical connectivity.

### Native firmware PHY capability continuation

The native sequence now calls the real bounded GET_FEATURE mailbox query after
download and before runtime MAC setup. It retains the response token from that
exact queue/Ready session and logs all four reply words, effective TX/RX NSS and
whether successful zero NSS fields invoked the source chip-default rule. A
missing, malformed or late reply fails this stage; it cannot substitute OTP or
assumed two-stream capabilities. The existing independent cleanup remains in
force, with no resend or ordinary data TX. The startup fixture covers failure
at all fourteen stages; execution is recorded only after raw publication.
This remains a diagnostic, not calibrated RF or a WLAN endpoint.

Own native continuation `e4cb5ea8` and consolidated `dd91a0a1` build the
complete x86 image. After raw publication `f566311f`, all twelve startup
fixtures passed (0.00 s, zero failed/ignored, x86-64 WSL2) including failure
at every one of the fourteen stages. This verifies orchestration behavior,
not actual firmware capabilities or a physical network link.
