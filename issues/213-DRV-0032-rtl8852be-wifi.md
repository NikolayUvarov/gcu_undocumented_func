# 213-DRV-0032 — Bring up the Ryzen RTL8852BE Wi-Fi adapter

**Type:** driver · **Owner:** ASTRA auditor-fixer, for `DRV`, with `KRN` and `NET` integration · **Priority:** P1 (the maintainer's first network priority, 2026-10-15) · **Status:** open · **Blocked by:** [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md), [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md), hardware calibration and radio bring-up · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H, A, D · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3, Appendix B.6

## Problem

The Ryzen report `hw0005.txt`, build e6730d7a, identifies `03:00.0`, `10ec:b852`, revision `00`, subsystem `10ec:b852`: RTL8852BE. There is no MIND driver for it. Existing VirtIO/Broadcom paths cannot make this adapter work. A firmware parser or a PCI identification line is not Wi-Fi support.

The maintainer requests Wi-Fi first, with RTL8168 Ethernet work in parallel, then tests/configuration through Effector. The existing WLAN service can supply association and WPA2 policy only after this adapter supplies working 802.11 management/data frames and channel control.

## Plan

1. Establish precise source/licence provenance, select a compatible official firmware, and collect the digital chip cut without RF/DMA initialization: [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md).
2. Implement a bounded MAC power lifecycle for RTL8852B, including rollback and explicit power-state diagnostics. Add the documented indirect XTAL/EFUSE reads only after that sequence is reviewed. Read the real MAC address, RFE type, crystal trim, country and board calibration; never invent defaults from PCI identity alone.
3. Port the supported board's BB/RF/NCTL tables and calibration sequence from licence-compatible, pinned sources. Record every imported table and source in `THIRD_PARTY.md`. Respect country/regulatory power restrictions. Missing or unsupported calibration leaves transmission off.
4. Implement firmware download and H2C/C2H transport with absolute deadlines, bounded retries, secure-boot policy and explicit status. Validate a pinned, hash-verified image before any device write. Structural parsing alone is not authentication, approval of download addresses or secure-boot support.
5. Implement PCIe RX/TX descriptors, buffer ownership, DMA address validation and interrupts in a separate user-space driver. KRN reviews capability/IRQ/DMA integration; no ABI change is made in this DRV task. Link-down, firmware failure and malformed descriptors must not stall other network cards or tests.
6. Connect the driver to the existing WLAN service, initially the narrowest measured configuration: one supported band/channel width and WPA2-PSK, then scan/associate, DHCP, DNS and ping during test load. Extend capabilities only with evidence for each configuration.
7. Run the applicable host/QEMU gates on ASTRA, then record the actual Ryzen build, AP/band/channel, firmware hash, association/IP/link events and latency/loss during the test suites. Include reconnection after AP loss and bounded failure when firmware is absent. Effector registration/poll/report is a separate end-to-end check after stable IP/TLS exists.

## Acceptance criteria

- The actual Ryzen RTL8852BE associates with the configured AP, obtains an address and keeps ping plus ordinary traffic responsive while MIND tests run; report measurements, not only a successful initial ping.
- Firmware/board/calibration provenance and supported chip cuts are explicit. Unknown cuts, unavailable firmware and invalid calibration fail with useful diagnostics and without transmission.
- RX/TX and firmware-command paths have bounded waits and resource limits; interruption, link loss and restart do not corrupt ownership or block unrelated services.
- Firmware remains outside the repository and built images, in the maintainer's `proprietary/` store and, after explicit copying, `data/firmware/` on the written boot volume.
- Every implemented slice has focused tests; required project gates and hardware evidence identify their configuration. Documentation states limitations, including unsupported security modes/bands.

## Related

[213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md), [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [550](550-network-on-real-hardware.md), [750](750-effector-agent.md), [RTL8852BE source/format notes](../hwdocs/rtl8852be.md), `wlan/`, `wpa/`, `init/src/main.rs`.

## Progress (2026-10-15)

The first slice provides source pins, a firmware fetch/check helper and bounded, host-tested format/register helpers. It performs no radio, firmware-download, association or packet-transfer operation. The task remains open.

On 2026-10-15 at 18:34:35 UTC, the Ryzen's Ubuntu `7.0.0-38-generic` driver boot log established digital cut 1 (CBV), analog cut 1 and RFE 1 for `0000:03:00.0`, through Effector operation `20261015183435.268000692_000025`. No debugfs/MMIO access was needed. The compatible pinned normal-CE image is cut 1, version 0.29.29.15, offset 96, size 326616. This removes the unknown-cut/RFE selection question; it does not provide the remaining EFUSE/PHY calibration values or a MIND MAC power, firmware-download, DMA or radio implementation. The raw MIND `SYS_CFG1` observation remains in [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md).

### Implemented slice: bounded native board access

The maintainer reaffirmed Wi-Fi as the first network priority after the x86
validation stage. Work proceeds on `codex/213-DRV-0032-rtl8852be`, from ASTRA
`1aea9100` merged with main `bd7b5493`. The slice is an executable
`rtw89_wifi` service with exact PCI/cut checks, the pinned RTL8852B MAC power
lifecycle, bounded indirect XTAL operations, board EFUSE reads and shutdown.
Tests exercise the same register sequence through a simulated I/O seam;
they cannot establish operation on the actual adapter.

The service must begin with memory decoding enabled and PCI bus mastering
disabled. The current generic BAR grant enables both, and no RTL8852BE boot
service exists. KRN-0114 supplies the required boot image and MEM-only BAR grant;
this task does not change the boot ABI or borrow the Broadcom service identity.
Firmware download, DMA rings, BB/RF calibration, WLAN registration and traffic
remain later work. No Linux driver unbinding, reset or radio experiment is part
of this slice.

Own-branch commit `a6e69a07` builds for `x86_64-unknown-none`. After its
publication in `origin/fast-test` merge `0b5bd1bf`, all 16 focused host tests
passed on 2026-10-15 (0.05 seconds). No tests ran on `fast-test`. Independent
review found and corrected the native 10 ms sleep-tick mismatch and acceptance
of transitional MAC power states. Short hardware waits now require the
calibrated clock; initial/final MAC power state must be zero. These results do
not establish hardware operation or a network link. KRN-0114 integration must
rebuild the executable with its final boot ABI; actual Ryzen board reads,
power-off and all radio acceptance criteria remain pending.

### Implemented slice: exact RFE-1/cut-1 PHY tables

The separate branch `codex/213-DRV-0032-rtl8852be-phy` provides the pinned
BSD BB/gain/RF/NCTL data and bounded production stages in `phy.rs`.
The RF stage includes both physical paths and mandatory firmware mirrors
under one deadline. Partial failures poison the attempt and independently
attempt BB/RF disable; guard refusal never changes an unknown active device.
The library requires actual firmware-init-ready, full single-band MAC,
BB access, stopped data paths and a complete BAR2 mapping. The native service
still does not call it. The chip TX-power unit, gain decoding and BB reset
now follow the BB table under the same deadline; failed interleaves cannot
publish gains or admit RF programming. The additional RCK stage follows NCTL and source DPK backoff, with bounded
analog/direct RF access and separate RF5-restore/chip-disable errors.
BTC/pre-NCTL DM interleaves, remaining RFK calibration,
board trims and regulatory authorization remain integration
work, as specified in [the PHY contract](../hwdocs/rtl8852be-phy.md).

The initial `59a77177` run passed 18/19 Rust cases; the RF-prefix case
incorrectly expected register 0 before the pinned table's first register 5.
After correction and publication of `f4766604`, all 20 Rust fixtures and four
extractor fixtures passed, as did pinned-source regeneration. Published
`78cf274a` passed all 24 Rust cases including the TX-power/BB-reset interleave.
Published `0d6c66bf` passed all 34 RCK/RF/PHY fixtures. The separate DACK
engine follows with mandatory typed BTC START/STOP, pinned BSD calibration
tables and preserved per-stage/cleanup failures. After publication of
`7c6d5ecc` and `a8418545`, all 44 Rust and three DACK extractor fixtures
passed, as did pinned table regeneration. The next initial RXDCK slice uses
the shared zero-only scheduler mailbox, post-DACK phase guard and actual BTC
contract. Native build passes and 56 fixtures compile; execution awaits its
publication. It never enables packet transmission.
Actual BTC arbitration and the pre-NCTL DM prefix still block native activation. No hardware or radio success is claimed.

Independent review corrected the generic-MAC BB/RF guard/cleanup to the actual RTL8852B common-chip dispatch before native activation. Cleanup now clears AFC/BB reset and both XTAL RF-enable fields without replacing a pending indirect command.
