# 213-DRV-0031 — A wired RTL8168 path on the Ryzen for ordinary use and Effector

**Type:** driver · **Owner:** ASTRA auditor-fixer, for `DRV`, with `KRN` and `NET` integration · **Priority:** P1 · **Status:** in progress (variant identified; descriptor foundation; hardware bring-up pending) · **Blocked by:** reviewed variant initialization/firmware, boot grants and hardware validation · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H, A, D · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3, Appendix B.6

## Problem

Ryzen build e6730d7a boots the network stack without a driver for its installed `10ec:8168` Ethernet controller. Ping and an Effector agent cannot work over a NIC the operating system does not drive. The separate RTL8852BE Wi-Fi device is not covered by this task. The timeout/fairness changes in [550-NET-0031](550-NET-0031-bounded-driver-calls-and-ingress.md) do not add hardware support.

The maintainer requested both Wi-Fi and Ethernet, with Wi-Fi first (2026-10-15). This wired task can proceed in parallel; it is not a replacement for the Wi-Fi task.

## Plan

1. **Review the identified variant.** Task 213-DRV-0030 observed XID `0x4c0`, RTL8168g/8111g, on the actual Ryzen; the pinned source mapping is in [hwdocs/rtl8168.md](../hwdocs/rtl8168.md#ryzen-observation-2026-10-15). Match only reviewed variants; report an unknown XID and leave the device stopped. Derive this variant's reset, PHY negotiation and power-management sequence from primary specifications or reviewed, licence-compatible sources; record those sources and any reused code in `THIRD_PARTY.md`. No speculative PHY writes.
2. **Provide a ring-3 driver.** A proposed `rtl8168` service receives its own MMIO BAR, DMA memory, interrupt and service endpoint. Read and validate the MAC; bound every reset, PHY and descriptor wait. Set up receive/transmit descriptor rings with ownership, length and wrap checks; use memory barriers at ownership changes; reclaim a descriptor only after completion. Start with 1500-byte MTU, software checksums and no segmentation offload. Count receive errors, queue overflow, transmit failures and interrupt activity. Keep the service responsive when link negotiation or transmission fails.
3. **Provide boot grants as a kernel-track task.** `init` finds class `02:00:00` and exact PCI ID `0x8168_10ec`, takes the observed device's current BAR2 through the platform capability path, lends bounded DMA memory below 4 GiB, and grants MSI through the existing `PLATFORM_DEVICE_MSI`. It records the device for normal quiesce/restart handling. The reported legacy line `255` is not a usable fallback. Do not grant arbitrary PCI configuration access or hard-code the observed BAR address. If the verified variant requires an operation current capabilities cannot express, request an explicit KRN interface task.
4. **Use the existing card protocol.** Serve `idl/net.wit` 1.3: info/link, frame send/receive, deferred wait, counters, read-only badges, and zero offloads. Implement the existing shared frame ring or explicitly reject attach until it is supported. Return bounded, documented errors on absent/down/busy hardware. Add the service to the image and startup registry through KRN's task.
5. **Connect it to the stack.** NET/KRN agree which wired card slot carries RTL8168, preserving existing VirtIO cards and WLAN. Ensure default-route selection and DHCP/link transitions use the same code as other wired cards. A missing or disconnected NIC must not stall other cards or socket requests. No service ABI change is needed solely to introduce this driver.
6. **Handle firmware explicitly.** Determine from the identified variant whether an external firmware file is needed. If its terms forbid redistribution, use the project's named-source fetch, SHA-256 validation and post-image-write `data/firmware/` installation. Never commit proprietary payloads or put them in an image; a missing required file must be reported.
7. **Validate before claiming support.** Host tests cover descriptor serialization, boundaries, ownership/reclaim, malformed receive lengths and bounded wait/recovery. QEMU regression checks protect existing network services; an RTL8139 emulation is not evidence for RTL8168. Then test the exact Ryzen, recording the build, XID, interface settings, cable/switch and results.

## Acceptance criteria

- The installed variant is explicitly identified and supported; unsupported variants fail clearly.
- On this Ryzen, a connected Ethernet link obtains an address and route through DHCP; bidirectional ping and an authenticated HTTPS request through the existing TLS/policy services work.
- Record at least 30 minutes of ping alongside sustained TCP transfer and the maintainer's previously slow tests: packet counts/loss, latency, throughput, NIC counters and kernel/service failures. Any loss or pause is explained and bounded rather than hidden in an aggregate success message.
- Unplug/replug, link negotiation failure and driver restart leave the stack responsive; a restored link reacquires its lease and new flows work. No old DMA buffers are reused while the device still owns them.
- No unresolved transmit/receive timeout, DMA error or service hang remains in the validation run. Measurements and known limits are recorded per machine/build in `docs/profile`.
- Effector's connectivity task may depend on this result, but completing the driver does not itself establish a working Effector agent or server session.

## Related

[213-DRV-0030](../issues-done/213-DRV-0030-rtl8168-hardware-identification.done), [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md), [550-DRV-0005](550-DRV-0005-usb-ethernet.md), [550-NET-0031](550-NET-0031-bounded-driver-calls-and-ingress.md), [750](750-effector-agent.md), [requests-DRV.md](requests-DRV.md), `idl/net.wit`, `init/src/main.rs`.

## Progress (2026-10-15)

The independent `rtl8168` no_std library now implements a read-only TxConfig/XID decoder and coherent x86-64 descriptor rings: checked DMA layout below 4 GiB, bounded single-frame send/receive/reclaim, TX padding and full-ring handling, volatile ownership with compiler/hardware fences, RX validation/FCS removal and wrap handling. Its unsafe contract requires exclusive stable coherent memory and a quiesced device before reuse. It contains no executable, driver startup registration, PHY/reset writes or claim of a working Ethernet path.

Eight host regressions in `tests/rtl8168_host.rs` pass, including independent raw descriptor values, full/wrapped queues, ownership retention on short output, malformed input and unknown/inaccessible identity. The crate checks for `x86_64-unknown-none`. Primary sources, exact layout, safety conditions and remaining hardware-dependent work are recorded in [hwdocs/rtl8168.md](../hwdocs/rtl8168.md). No Linux or BSD driver implementation is copied.

Task 0030 now supplies actual Linux hardware evidence from `2026-10-15T18:29:02Z`: TxConfig `0x4f000f80`, XID `0x4c0`, with verified `10ec:8168` revision `0c`, PHY ID `0x001cc800` and the driver's reported `rtl8168g-2_0.0.1 02/06/13` firmware version. The source-selected initialization and firmware filename are documented in the hardware notes; this identifies the implementation target without claiming support. Carrier was absent during collection. Device initialization, firmware byte/licence review, boot grants, service integration and the connected-link/load/recovery acceptance tests remain open. Identification no longer blocks implementation review.
