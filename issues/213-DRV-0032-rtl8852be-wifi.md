# 213-DRV-0032 — Bring up the Ryzen RTL8852BE Wi-Fi adapter

**Type:** driver · **Owner:** ASTRA auditor-fixer, for `DRV`, with `KRN` and `NET` integration · **Priority:** P1 (the maintainer's first network priority, 2026-10-15) · **Status:** open · **Blocked by:** [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md), hardware calibration and radio bring-up · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H, A, D · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3, Appendix B.6

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
