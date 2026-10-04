# 100 — `virtio_net`: network card driver in ring 3

**Type:** driver · **Owner:** kernel track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track A (stage III), first step of track D · **Constitution:** MC-1.3, MC-6.3, Appendix B.6 (NIC driver)

## Problem

MIND Core has no network at all (profile: "One node; no network"). Track D (network stack, policy broker, TLS) needs a NIC driver first. Stage III names VirtIO block/net/input as the driver vertical slice.

## Plan

- `virtio_net` service for the VirtIO network device in its legacy (transitional) PCI interface: I/O BAR0, split virtqueues, legacy INTx interrupt. Authority per B.6: the I/O ports of its own device, its IRQ line and a bounded DMA region; nothing else.
- `DEVICE_FIND` learns to match PCI vendor and device IDs (a QEMU `pc` machine has an e1000 by default, of the same class).
- Interface `idl/net.wit`: device info (MAC, MTU, link), send one Ethernet frame, receive one frame, wait for a frame (deferred reply from the interrupt), counters. Frames travel in MIND IDL buffers (copied into the driver's private memory, the client's lease revoked before the reply is read).
- Supervision: `init` stops the device (no decoding, no DMA) and clears the DMA region before a restart, as for AHCI and xHCI.
- Diagnostics: the shell's `net` command (info, `net arp <ip>`) through a client `init` grants the shell. The only other client will be the network stack (issue 101).

## Acceptance criteria

- In QEMU with `-device virtio-net-pci` and user networking, `net` shows the MAC and link state and `net arp 10.0.2.2` gets the gateway's answer (frames sent and received through the driver).
- After the driver is killed, `init` quiesces the device, restarts the driver, and `net arp` works again.
- Without the device the driver reports none; a QEMU e1000 is not taken for a VirtIO device.
- A QEMU suite covers it and runs in CI; the profile and docs list the driver and its authority.

## Related

[ROADMAP](../ROADMAP.md) tracks A and D; [101](101-network-stack.md), [102](102-network-policy-broker.md), [103](103-tls-service.md).
