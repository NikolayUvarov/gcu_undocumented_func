# 550 — Network on real hardware: the MacBook Pro first, then PCs

**Type:** drivers (main task) · **Owner:** `DRV` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** tracks A and D · **Constitution:** MC-12.1, MC-12.9, MC-1.5, MC-2.3, Appendix B.6

Asked by the maintainer (2026-10-08): the MacBook Pro that boots to the shell ([211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md)) should reach the network, and PCs after it. Opened by the kernel session; the `DRV` track is open. Main task [501](501-effector.md) (tests on real hardware without the maintainer typing them) needs it.

## Problem

The only network driver is `virtio_net`, which drives QEMU's VirtIO card. `netstack`, `netpolicy`, `tls` and `keystore` run above it in QEMU only; no physical machine has reached a network.

The MacBook Pro Retina 15" (A1398, 2012–13, Intel 7-series chipset) lists in `devices`, by class only (the shell prints no vendor or device IDs):

- `03:00.0`, class `020000` (Ethernet), with `03:00.1`, class `080501` (SD host). On this model that is a Broadcom BCM57765-family combination chip. This Mac has no Ethernet socket, so wired network here means a USB or Thunderbolt Ethernet adapter.
- `04:00.0`, class `028000` (network): the Wi-Fi. Apple's specifications give a Broadcom BCM4331 for this model; its PCI ID has not been read yet.
- `05:00.0`, `06:xx` and `07:00.0`: Thunderbolt.

No driver serves any of them. What exists to build on:

- `usb_host` serves `idl/usb.wit` to class drivers and works on this Mac: a USB-SATA adapter at SuperSpeed, a receiver at full speed, hot plug.
- `netstack` has one interface per card ([105](../issues-done/105-multiple-network-cards.done)) and shares a frame ring with each driver ([107](../issues-done/107-batched-frame-path.done)), but its cards are `virtio_net` instances only, two of them.

## Plan: tasks by track

| Task | Track | What |
|---|---|---|
| [550-DRV-0005](550-DRV-0005-usb-ethernet.md) | `DRV` (open) | A USB Ethernet class driver for CDC-ECM and CDC-NCM over `usb_host`, with the choice of configuration and alternate setting in `usb_host`. The first and quickest path: many USB Ethernet adapters offer one of the two, and QEMU has a CDC-ECM device to test with |
| [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) | `DRV` (open) | The Mac's Broadcom Wi-Fi (BCM4331 class, expected). A SoftMAC chip: it needs a host 802.11 MAC layer (`NET`) and Broadcom firmware, whose licence is checked, through the maintainer, before anything is shipped |
| [550-DRV-0007](550-DRV-0007-broadcom-ethernet.md) | `DRV` (open) | Broadcom tg3-family Ethernet: Apple's Thunderbolt Gigabit Ethernet adapter and many PCs; no QEMU model |
| [requests-NET.md](requests-NET.md): several network interfaces | `NET` | Card drivers other than `virtio_net`, more than two, cards that come and go; the default route chosen by a stated rule |
| [requests-NET.md](requests-NET.md): an 802.11 station and WPA2-PSK | `NET` | Scan, authentication and association, the WPA2-PSK 4-way handshake and CCMP; a Wi-Fi configuration interface |
| [550-APP-0033](550-APP-0033-wifi-setup-program.md): a Wi-Fi setup program | `APP` | Lists the networks found with signal and security, connects with a passphrase and stores it through `keystore` |
| `init` (not numbered yet) | `KRN` | Starts the new drivers with their `usb_host` badge or PCI device and hands their endpoints to `netstack` as cards; numbered by the kernel track when 550-DRV-0005 needs it |

Order: 550-DRV-0005 and the `NET` request for several interfaces first; Wi-Fi after 550-DRV-0006 and the station; 550-DRV-0007 when an adapter or a PC with such a chip is at hand.

Every new PCI driver that does DMA joins the TCB of memory isolation, as `virtio_net` does today: no IOMMU is used (MC-1.5, [docs/profile/tcb.md](../docs/profile/tcb.md)). The profile names each one when it lands.

Later, as their own issues when needed:

- vendor USB protocols: Realtek RTL8152/8153 in vendor mode, ASIX AX88179, and the ASIX AX88772 that Apple's own USB Ethernet adapter is expected to use;
- Thunderbolt hot plug (an adapter plugged in after power-on);
- the Ethernet of the maintainer's Intel PC ([211](211-intel-pc-from-a-sata-ssd.md)), often an Intel I21x or a Realtek RTL8111. QEMU models Intel's `e1000e`, so an Intel driver could be tested in CI;
- Intel Wi-Fi in PCs, whose firmware licence is a question of its own.

## Acceptance criteria

- **First:** the MacBook Pro gets an address by DHCP over a USB Ethernet adapter (CDC-ECM or CDC-NCM), and the shell's `https` fetches a page. The run is recorded as this machine's configuration, with the adapter's IDs, not carried over from QEMU (MC-12.1, MC-12.9).
- **Later:** the same over Wi-Fi with WPA2-PSK.
- The main task closes when both pass, or when the Wi-Fi part is split off with its reason.

## Related

[211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), [211](211-intel-pc-from-a-sata-ssd.md), [501](501-effector.md), [351](351-self-update.md) (updates fetched over the network), [105](../issues-done/105-multiple-network-cards.done), [107](../issues-done/107-batched-frame-path.done), `idl/net.wit`, `idl/usb.wit`, [docs/profile/network.md](../docs/profile/network.md).
