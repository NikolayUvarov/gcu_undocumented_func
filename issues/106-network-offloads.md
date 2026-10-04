# 106 — Checksum and segmentation offloads, after measurement

**Type:** performance · **Owner:** network track · **Priority:** P3 · **Status:** open · **Blocked by:** 101, a throughput benchmark · **Roadmap:** track D · **Constitution:** Appendix B.2 (performance decided by measurement on a profile), MC-1.5, MC-12.1

## Problem

VirtIO devices can compute and verify checksums (`VIRTIO_NET_F_CSUM`, `GUEST_CSUM`) and split or merge large segments (TSO, GSO, mergeable receive buffers). They save CPU time, but receive-side checksum checking means the stack trusts the device for data integrity.

## Plan

1. Benchmark first: throughput and CPU use of `netstack` over `virtio_net` (TCP bulk transfer to a host-forwarded port, small-packet rate), recorded in `docs/profile`.
2. Only if the benchmark shows checksum or segmentation work as a bottleneck: negotiate the features in `virtio_net`, carry the per-frame flags in `idl/net.wit` (a minor version), use them in the stack.
3. Profile: state that with receive checksum offload the device is trusted for integrity of received data (it already is in the TCB without an IOMMU, MC-1.5); keep a switch that turns offloads off.

## Acceptance criteria

- A recorded benchmark before and after; offloads enabled only with a measured gain; the profile names the trust consequence; the `net` suites pass with offloads on and off.

## Related

[101](../issues-done/101-network-stack.done), [104](../issues-done/104-virtio-modern-msix.done).
